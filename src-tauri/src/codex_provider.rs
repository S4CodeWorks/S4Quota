use crate::domain::{ProviderId, QuotaSnapshot, QuotaWindow, QuotaWindowKind};
use crate::json_rpc::{JsonRpcClient, JsonRpcConfig, RpcError, RpcNotification};
use crate::provider_manager::{
    ProviderDriver, ProviderFailure, ProviderFailureKind, ProviderSession, SessionSignal,
};
use crate::sanitize::sanitize_text;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::process::Command;
use tokio::sync::{broadcast, watch};
use tokio::time;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const ACCOUNT_READ: &str = "account/read";
const RATE_LIMITS_READ: &str = "account/rateLimits/read";
const RATE_LIMITS_UPDATED: &str = "account/rateLimits/updated";
const ACCOUNT_UPDATED: &str = "account/updated";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexExecutableKind {
    Native,
    TrustedCmdShim,
}

#[derive(Debug, Clone)]
pub struct CodexInstallation {
    path: PathBuf,
    kind: CodexExecutableKind,
    version: Option<String>,
}

impl CodexInstallation {
    #[cfg(test)]
    pub fn kind(&self) -> CodexExecutableKind {
        self.kind
    }

    #[cfg(test)]
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }
}

#[derive(Debug, Clone)]
pub struct CodexProvider {
    rpc_config: JsonRpcConfig,
}

impl Default for CodexProvider {
    fn default() -> Self {
        Self {
            rpc_config: JsonRpcConfig::default(),
        }
    }
}

impl CodexProvider {
    async fn discover_installation(&self) -> Result<CodexInstallation, ProviderFailure> {
        let output = time::timeout(Duration::from_secs(5), async {
            let mut command = Command::new(locator_command());
            command.arg("codex");
            hide_window(&mut command);
            command.kill_on_drop(true).output().await
        })
        .await;

        let mut candidates = match output {
            Ok(Ok(output)) if output.status.success() => String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(PathBuf::from)
                .collect::<Vec<_>>(),
            Ok(Ok(_)) | Ok(Err(_)) | Err(_) => Vec::new(),
        };
        #[cfg(windows)]
        candidates.extend(desktop_codex_candidates());
        deduplicate_candidates(&mut candidates);
        if candidates.is_empty() {
            return Err(unavailable(
                "Codex executable was not found in PATH or the Codex Desktop installation",
            ));
        }
        let (path, kind) = select_candidate(&candidates)?;
        let version = collect_version(&path, kind).await;
        Ok(CodexInstallation {
            path,
            kind,
            version,
        })
    }

    async fn spawn_session(
        &self,
        installation: &CodexInstallation,
    ) -> Result<CodexSession, ProviderFailure> {
        let command = codex_command(installation, app_server_arguments())?;
        let client = JsonRpcClient::spawn(command, self.rpc_config.clone())
            .await
            .map_err(|error| classify_rpc_error(error, "spawn", installation.version.clone()))?;
        let notifications = client.subscribe_notifications();
        let exit = client.subscribe_exit();
        Ok(CodexSession {
            client,
            notifications,
            exit,
            version: installation.version.clone(),
        })
    }
}

fn app_server_arguments() -> &'static [&'static str] {
    &["app-server"]
}

pub struct CodexSession {
    client: JsonRpcClient,
    notifications: broadcast::Receiver<RpcNotification>,
    exit: watch::Receiver<Option<crate::json_rpc::ProcessExit>>,
    version: Option<String>,
}

impl CodexSession {
    #[cfg(test)]
    pub fn job_object_active(&self) -> bool {
        self.client.job_object_active()
    }

    #[cfg(test)]
    pub fn diagnostic_counts(&self) -> (usize, usize) {
        (
            self.client.protocol_error_count(),
            self.client.stderr_diagnostics().len(),
        )
    }
}

#[async_trait]
impl ProviderSession for CodexSession {
    fn process_id(&self) -> Option<u32> {
        Some(self.client.process_id())
    }

    async fn next_signal(&mut self) -> SessionSignal {
        loop {
            tokio::select! {
                notification = self.notifications.recv() => match notification {
                    Ok(notification) if notification.method == RATE_LIMITS_UPDATED
                        || notification.method == ACCOUNT_UPDATED => {
                            return SessionSignal::RefreshSuggested;
                        }
                    Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => {
                        return SessionSignal::ProcessExited("notification stream closed".into());
                    }
                },
                changed = self.exit.changed() => {
                    if changed.is_err() {
                        return SessionSignal::ProcessExited("process supervisor stopped".into());
                    }
                    if let Some(exit) = self.exit.borrow().clone() {
                        return SessionSignal::ProcessExited(exit.summary());
                    }
                }
            }
        }
    }

    async fn shutdown(self) -> Result<(), ProviderFailure> {
        self.client
            .shutdown()
            .await
            .map(|_| ())
            .map_err(|error| classify_rpc_error(error, "shutdown", self.version))
    }
}

#[async_trait]
impl ProviderDriver for CodexProvider {
    type Installation = CodexInstallation;
    type Session = CodexSession;

    async fn discover(&self) -> Result<Self::Installation, ProviderFailure> {
        self.discover_installation().await
    }

    fn executable_label(&self, installation: &Self::Installation) -> String {
        match installation.kind {
            CodexExecutableKind::Native => "codex.exe (native)".into(),
            CodexExecutableKind::TrustedCmdShim => "codex.cmd (trusted npm shim)".into(),
        }
    }

    fn version(&self, installation: &Self::Installation) -> Option<String> {
        installation.version.clone()
    }

    async fn spawn(
        &self,
        installation: &Self::Installation,
    ) -> Result<Self::Session, ProviderFailure> {
        self.spawn_session(installation).await
    }

    async fn initialize(&self, session: &mut Self::Session) -> Result<(), ProviderFailure> {
        let response = session
            .client
            .request(
                "initialize",
                Some(json!({
                    "clientInfo": {
                        "name": "s4quota",
                        "title": "S4Quota",
                        "version": env!("CARGO_PKG_VERSION")
                    },
                    "capabilities": {
                        "experimentalApi": false
                    }
                })),
            )
            .await
            .map_err(|error| classify_rpc_error(error, "initialize", session.version.clone()))?;
        serde_json::from_value::<InitializeResponse>(response).map_err(|_| ProviderFailure {
            kind: ProviderFailureKind::Transient,
            message: "initialize returned an incompatible response".into(),
            version: session.version.clone(),
        })?;
        session
            .client
            .notify("initialized", Some(json!({})))
            .await
            .map_err(|error| classify_rpc_error(error, "initialized", session.version.clone()))
    }

    async fn refresh(&self, session: &mut Self::Session) -> Result<QuotaSnapshot, ProviderFailure> {
        let account = session
            .client
            .request(ACCOUNT_READ, Some(json!({ "refreshToken": false })))
            .await
            .map_err(|error| classify_rpc_error(error, ACCOUNT_READ, session.version.clone()))?;
        let account: GetAccountResponse =
            serde_json::from_value(account).map_err(|_| ProviderFailure {
                kind: ProviderFailureKind::Transient,
                message: "account/read returned an incompatible response".into(),
                version: session.version.clone(),
            })?;
        if account.requires_openai_auth && account.account.is_none() {
            return Err(ProviderFailure {
                kind: ProviderFailureKind::Authentication,
                message: "Codex authentication is required".into(),
                version: session.version.clone(),
            });
        }

        let limits = session
            .client
            .request(RATE_LIMITS_READ, None)
            .await
            .map_err(|error| {
                classify_rpc_error(error, RATE_LIMITS_READ, session.version.clone())
            })?;
        let limits: GetAccountRateLimitsResponse =
            serde_json::from_value(limits).map_err(|_| ProviderFailure {
                kind: ProviderFailureKind::Transient,
                message: "account/rateLimits/read returned an incompatible response".into(),
                version: session.version.clone(),
            })?;
        Ok(normalize_rate_limits(limits))
    }
}

// Wire DTOs are scoped to this adapter and match the schemas emitted by the
// installed App Server during Stage 3. Runtime compatibility is still proven
// by successful method calls, never by this diagnostic version alone.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InitializeResponse {
    #[allow(dead_code)]
    user_agent: String,
    #[allow(dead_code)]
    platform_family: String,
    #[allow(dead_code)]
    platform_os: String,
    #[allow(dead_code)]
    codex_home: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetAccountResponse {
    #[serde(default)]
    account: Option<AccountDescriptor>,
    requires_openai_auth: bool,
}

#[derive(Debug, Deserialize)]
struct AccountDescriptor {
    #[serde(rename = "type")]
    #[allow(dead_code)]
    account_type: String,
    #[serde(default, rename = "planType")]
    #[allow(dead_code)]
    plan_type: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetAccountRateLimitsResponse {
    rate_limits: RateLimitSnapshot,
    #[serde(default)]
    rate_limits_by_limit_id: Option<BTreeMap<String, RateLimitSnapshot>>,
    #[serde(default)]
    ordinary_usage_allowed: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RateLimitSnapshot {
    #[serde(default)]
    limit_id: Option<String>,
    #[serde(default)]
    limit_name: Option<String>,
    #[serde(default)]
    primary: Option<RateLimitWindow>,
    #[serde(default)]
    secondary: Option<RateLimitWindow>,
    #[serde(default)]
    rate_limit_reached_type: Option<String>,
    #[serde(default)]
    spend_control_reached: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RateLimitWindow {
    used_percent: i32,
    #[serde(default)]
    window_duration_mins: Option<i64>,
    #[serde(default)]
    resets_at: Option<i64>,
}

fn normalize_rate_limits(response: GetAccountRateLimitsResponse) -> QuotaSnapshot {
    let ordinary_usage_allowed = response.ordinary_usage_allowed;
    let buckets: Vec<(String, RateLimitSnapshot)> = match response.rate_limits_by_limit_id {
        Some(by_id) if !by_id.is_empty() => by_id.into_iter().collect(),
        _ => vec![(
            response
                .rate_limits
                .limit_id
                .clone()
                .unwrap_or_else(|| "default".into()),
            response.rate_limits,
        )],
    };

    let mut windows = Vec::new();
    for (bucket_key, bucket) in buckets {
        let bucket_id = safe_identifier(
            bucket
                .limit_id
                .as_deref()
                .filter(|value| !value.is_empty())
                .unwrap_or(&bucket_key),
        );
        let limited = ordinary_usage_allowed == Some(false)
            || bucket.rate_limit_reached_type.is_some()
            || bucket.spend_control_reached == Some(true);
        for (index, window) in [bucket.primary, bucket.secondary]
            .into_iter()
            .flatten()
            .enumerate()
        {
            let used = window.used_percent.clamp(0, 100);
            let remaining = 100_i32.saturating_sub(used);
            let kind = classify_window(window.window_duration_mins);
            let status = if limited {
                "limited"
            } else if remaining == 0 {
                "exhausted"
            } else {
                "available"
            };
            let duration_id = window
                .window_duration_mins
                .map(|value| value.to_string())
                .unwrap_or_else(|| "unknown".into());
            windows.push(QuotaWindow {
                id: format!("{bucket_id}:{duration_id}:{index}"),
                kind,
                label: bucket.limit_name.clone(),
                duration_minutes: window.window_duration_mins.map(Value::from),
                used_percent: Some(Value::from(used)),
                remaining_percent: Some(Value::from(remaining)),
                resets_at: window.resets_at.map(Value::from),
                limit_status: Value::String(status.into()),
            });
        }
    }
    windows.sort_by_key(|window| {
        window
            .duration_minutes
            .as_ref()
            .and_then(Value::as_i64)
            .unwrap_or(i64::MAX)
    });
    QuotaSnapshot {
        provider_id: ProviderId::Codex,
        fetched_at: unix_now(),
        windows,
    }
}

fn classify_window(duration_minutes: Option<i64>) -> QuotaWindowKind {
    match duration_minutes {
        Some(300) => QuotaWindowKind::Rolling,
        Some(10_080) => QuotaWindowKind::Weekly,
        _ => QuotaWindowKind::Other,
    }
}

fn safe_identifier(value: &str) -> String {
    let value: String = value
        .chars()
        .take(64)
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect();
    if value.is_empty() {
        "unknown".into()
    } else {
        value
    }
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .min(i64::MAX as u64) as i64
}

fn classify_rpc_error(error: RpcError, method: &str, version: Option<String>) -> ProviderFailure {
    match error {
        RpcError::Remote { code, message: _ } if matches!(code, -32600 | -32601) => {
            ProviderFailure {
                kind: ProviderFailureKind::Unsupported {
                    missing: vec![method.into()],
                },
                message: format!("required App Server capability is unavailable: {method}"),
                version,
            }
        }
        RpcError::Remote { code, message }
            if matches!(code, 401 | 403)
                || message.to_ascii_lowercase().contains("auth")
                || message.to_ascii_lowercase().contains("login") =>
        {
            ProviderFailure {
                kind: ProviderFailureKind::Authentication,
                message: "Codex authentication is required".into(),
                version,
            }
        }
        RpcError::Spawn(message) => ProviderFailure {
            kind: ProviderFailureKind::Transient,
            message: format!("spawn failure: {}", sanitize_text(&message)),
            version,
        },
        RpcError::ProcessExited(message) => ProviderFailure {
            kind: ProviderFailureKind::ProcessExited,
            message: if method == "initialize" {
                format!(
                    "process exited before handshake completed: {}",
                    sanitize_text(&message)
                )
            } else {
                format!(
                    "process exited during {method}: {}",
                    sanitize_text(&message)
                )
            },
            version,
        },
        RpcError::Timeout { .. } => ProviderFailure {
            kind: ProviderFailureKind::Transient,
            message: if method == "initialize" {
                "App Server handshake timed out".into()
            } else {
                format!("App Server request timed out: {method}")
            },
            version,
        },
        RpcError::InvalidResponse(message) => ProviderFailure {
            kind: ProviderFailureKind::Transient,
            message: format!(
                "invalid JSON-RPC response during {method}: {}",
                sanitize_text(&message)
            ),
            version,
        },
        other => ProviderFailure {
            kind: ProviderFailureKind::Transient,
            message: sanitize_text(&other.to_string()),
            version,
        },
    }
}

fn unavailable(message: impl Into<String>) -> ProviderFailure {
    ProviderFailure {
        kind: ProviderFailureKind::Unavailable,
        message: sanitize_text(&message.into()),
        version: None,
    }
}

fn select_candidate(
    candidates: &[PathBuf],
) -> Result<(PathBuf, CodexExecutableKind), ProviderFailure> {
    for candidate in candidates {
        if has_extension(candidate, "exe") && candidate.is_file() {
            return candidate
                .canonicalize()
                .map(|path| (path, CodexExecutableKind::Native))
                .map_err(|error| unavailable(format!("Codex path is invalid: {error}")));
        }
    }
    for candidate in candidates {
        if is_trusted_cmd_shim(candidate) {
            return candidate
                .canonicalize()
                .map(|path| (path, CodexExecutableKind::TrustedCmdShim))
                .map_err(|error| unavailable(format!("Codex shim path is invalid: {error}")));
        }
    }
    Err(unavailable(
        "No native Codex executable or trusted npm codex.cmd shim was found",
    ))
}

fn is_trusted_cmd_shim(path: &Path) -> bool {
    if !has_extension(path, "cmd")
        || path.file_name() != Some(OsStr::new("codex.cmd"))
        || path
            .to_string_lossy()
            .chars()
            .any(|character| matches!(character, '&' | '|' | '<' | '>' | '^' | '%' | '!'))
    {
        return false;
    }
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() || metadata.len() > 16 * 1024 {
        return false;
    }
    let Ok(content) = std::fs::read_to_string(path) else {
        return false;
    };
    content
        .to_ascii_lowercase()
        .contains(r"node_modules\@openai\codex\bin\codex.js")
}

fn has_extension(path: &Path, expected: &str) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| extension.eq_ignore_ascii_case(expected))
}

async fn collect_version(path: &Path, kind: CodexExecutableKind) -> Option<String> {
    let installation = CodexInstallation {
        path: path.to_path_buf(),
        kind,
        version: None,
    };
    let mut command = codex_command(&installation, &["--version"]).ok()?;
    command.kill_on_drop(true);
    let output = time::timeout(Duration::from_secs(5), command.output())
        .await
        .ok()?
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = sanitize_text(String::from_utf8_lossy(&output.stdout).trim());
    text.strip_prefix("codex-cli ").map(str::to_owned)
}

fn codex_command(
    installation: &CodexInstallation,
    arguments: &[&str],
) -> Result<Command, ProviderFailure> {
    let mut command = match installation.kind {
        CodexExecutableKind::Native => Command::new(&installation.path),
        CodexExecutableKind::TrustedCmdShim => {
            if !is_trusted_cmd_shim(&installation.path) {
                return Err(unavailable("Codex command shim failed trust validation"));
            }
            let mut command = Command::new(system_cmd());
            command.args(["/d", "/c", "call"]);
            command.arg(cmd_compatible_path(&installation.path));
            command
        }
    };
    command.args(arguments);
    hide_window(&mut command);
    Ok(command)
}

fn locator_command() -> PathBuf {
    #[cfg(windows)]
    {
        return std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .map(|root| root.join("System32").join("where.exe"))
            .filter(|path| path.is_file())
            .unwrap_or_else(|| PathBuf::from("where.exe"));
    }
    #[cfg(not(windows))]
    PathBuf::from("which")
}

#[cfg(windows)]
fn desktop_codex_candidates() -> Vec<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .map(|root| root.join("OpenAI").join("Codex").join("bin"))
        .map(|root| desktop_codex_candidates_from(&root))
        .unwrap_or_default()
}

#[cfg(windows)]
fn desktop_codex_candidates_from(root: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let direct = root.join("codex.exe");
    if direct.is_file() {
        candidates.push(direct);
    }
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let candidate = entry.path().join("codex.exe");
            if candidate.is_file() {
                candidates.push(candidate);
            }
        }
    }
    candidates.sort_by(|left, right| {
        modified_time(right)
            .cmp(&modified_time(left))
            .then_with(|| left.cmp(right))
    });
    candidates
}

#[cfg(windows)]
fn modified_time(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
}

fn deduplicate_candidates(candidates: &mut Vec<PathBuf>) {
    let mut unique = Vec::<PathBuf>::new();
    candidates.retain(|candidate| {
        let normalized = candidate
            .canonicalize()
            .unwrap_or_else(|_| candidate.to_path_buf());
        if unique.iter().any(|existing| existing == &normalized) {
            false
        } else {
            unique.push(normalized);
            true
        }
    });
}

fn cmd_compatible_path(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        let value = path.to_string_lossy();
        if let Some(rest) = value.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!(r"\\{rest}"));
        }
        if let Some(rest) = value.strip_prefix(r"\\?\") {
            return PathBuf::from(rest);
        }
    }
    path.to_path_buf()
}

#[cfg(test)]
fn diagnostic_path(path: &Path) -> String {
    let normalized = cmd_compatible_path(path);
    for (variable, label) in [
        ("LOCALAPPDATA", "%LOCALAPPDATA%"),
        ("APPDATA", "%APPDATA%"),
        ("USERPROFILE", "%USERPROFILE%"),
    ] {
        if let Some(root) = std::env::var_os(variable).map(PathBuf::from) {
            if let Ok(relative) = normalized.strip_prefix(root) {
                return sanitize_text(&PathBuf::from(label).join(relative).to_string_lossy());
            }
        }
    }
    sanitize_text(&normalized.to_string_lossy())
}

fn system_cmd() -> PathBuf {
    std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .map(|root| root.join("System32").join("cmd.exe"))
        .filter(|path| path.is_file())
        .unwrap_or_else(|| PathBuf::from("cmd.exe"))
}

fn hide_window(command: &mut Command) {
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn discovery_fixture() -> PathBuf {
        static NEXT_FIXTURE_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let fixture_id = NEXT_FIXTURE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("s4quota-discovery-{unique}-{fixture_id}"));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn limits_fixture() -> GetAccountRateLimitsResponse {
        serde_json::from_value(json!({
            "ordinaryUsageAllowed": true,
            "rateLimits": {},
            "rateLimitsByLimitId": {
                "codex": {
                    "limitId": "codex",
                    "limitName": "Codex",
                    "primary": {"usedPercent": 25, "windowDurationMins": 300, "resetsAt": 200},
                    "secondary": {"usedPercent": 40, "windowDurationMins": 10080, "resetsAt": 300}
                },
                "future-bucket": {
                    "limitId": "future-bucket",
                    "primary": {"usedPercent": 140, "windowDurationMins": 30, "resetsAt": null}
                }
            }
        }))
        .unwrap()
    }

    #[test]
    fn normalizes_multiple_buckets_without_primary_secondary_semantics() {
        let snapshot = normalize_rate_limits(limits_fixture());
        assert_eq!(snapshot.provider_id, ProviderId::Codex);
        assert_eq!(snapshot.windows.len(), 3);
        assert!(snapshot.windows.iter().any(|window| {
            window.kind == QuotaWindowKind::Rolling && window.duration_minutes == Some(json!(300))
        }));
        assert!(snapshot.windows.iter().any(|window| {
            window.kind == QuotaWindowKind::Weekly && window.duration_minutes == Some(json!(10_080))
        }));
        let unknown = snapshot
            .windows
            .iter()
            .find(|window| window.duration_minutes == Some(json!(30)))
            .unwrap();
        assert_eq!(unknown.kind, QuotaWindowKind::Other);
        assert_eq!(unknown.used_percent, Some(json!(100)));
        assert_eq!(unknown.remaining_percent, Some(json!(0)));
    }

    #[test]
    fn reached_state_is_not_inferred_away_by_percentages() {
        let response = serde_json::from_value(json!({
            "ordinaryUsageAllowed": false,
            "rateLimits": {
                "limitId": "codex",
                "primary": {"usedPercent": 1, "windowDurationMins": 300}
            }
        }))
        .unwrap();
        let snapshot = normalize_rate_limits(response);
        assert_eq!(snapshot.windows[0].limit_status, json!("limited"));
    }

    #[test]
    fn capability_and_auth_errors_are_classified_separately() {
        let unsupported = classify_rpc_error(
            RpcError::Remote {
                code: -32601,
                message: "method not found".into(),
            },
            RATE_LIMITS_READ,
            Some("diagnostic".into()),
        );
        let auth = classify_rpc_error(
            RpcError::Remote {
                code: 401,
                message: "unauthorized".into(),
            },
            ACCOUNT_READ,
            None,
        );
        assert!(matches!(
            unsupported.kind,
            ProviderFailureKind::Unsupported { .. }
        ));
        assert_eq!(auth.kind, ProviderFailureKind::Authentication);
    }

    #[test]
    fn process_failures_keep_handshake_phase_and_diagnostics_distinct() {
        let spawn = classify_rpc_error(RpcError::Spawn("not found".into()), "spawn", None);
        let exited = classify_rpc_error(
            RpcError::ProcessExited("stdout reached EOF; exit code 1; stderr: fixture".into()),
            "initialize",
            Some("diagnostic".into()),
        );
        let timeout = classify_rpc_error(
            RpcError::Timeout {
                method: "initialize".into(),
            },
            "initialize",
            None,
        );
        let invalid = classify_rpc_error(
            RpcError::InvalidResponse("invalid JSON frame".into()),
            "initialize",
            None,
        );

        assert!(spawn.message.starts_with("spawn failure:"));
        assert!(exited
            .message
            .starts_with("process exited before handshake completed:"));
        assert_eq!(timeout.message, "App Server handshake timed out");
        assert!(invalid.message.starts_with("invalid JSON-RPC response"));
    }

    #[test]
    fn discovery_prefers_native_and_accepts_only_a_known_npm_cmd_shim() {
        let root = discovery_fixture();
        let native = root.join("codex.exe");
        let shim = root.join("codex.cmd");
        std::fs::write(&native, b"fixture").unwrap();
        std::fs::write(
            &shim,
            br#"@echo off
node "%~dp0\node_modules\@openai\codex\bin\codex.js" %*
"#,
        )
        .unwrap();

        let selected = select_candidate(&[shim.clone(), native.clone()]).unwrap();
        assert_eq!(selected.1, CodexExecutableKind::Native);
        let fallback = select_candidate(std::slice::from_ref(&shim)).unwrap();
        assert_eq!(fallback.1, CodexExecutableKind::TrustedCmdShim);

        std::fs::write(&shim, b"@echo off\necho untrusted\n").unwrap();
        assert!(select_candidate(&[shim]).is_err());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn production_spawn_arguments_use_default_stdio_transport() {
        assert_eq!(app_server_arguments(), &["app-server"]);

        let root = discovery_fixture();
        let native = root.join("codex.exe");
        std::fs::write(&native, b"fixture").unwrap();
        let installation = CodexInstallation {
            path: native.canonicalize().unwrap(),
            kind: CodexExecutableKind::Native,
            version: None,
        };
        let command = codex_command(&installation, app_server_arguments()).unwrap();
        let arguments: Vec<String> = command
            .as_std()
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect();
        assert_eq!(arguments, vec!["app-server"]);
        assert!(!arguments.iter().any(|argument| argument == "--stdio"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(windows)]
    #[test]
    fn desktop_discovery_finds_versioned_native_installations() {
        let root = discovery_fixture();
        let build = root.join("build-id");
        std::fs::create_dir_all(&build).unwrap();
        let native = build.join("codex.exe");
        std::fs::write(&native, b"fixture").unwrap();

        let candidates = desktop_codex_candidates_from(&root);
        assert_eq!(candidates, vec![native]);
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn trusted_cmd_shim_works_with_job_object_and_canonical_path() {
        let root = discovery_fixture();
        let shim = root.join("codex.cmd");
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("spike")
            .join("codex-app-server")
            .join("fixtures")
            .join("fake-server.mjs");
        std::fs::write(
            &shim,
            format!(
                "@echo off\r\nREM node_modules\\@openai\\codex\\bin\\codex.js\r\nnode.exe \"{}\" normal %*\r\n",
                fixture.display()
            ),
        )
        .unwrap();
        let installation = CodexInstallation {
            path: shim.canonicalize().unwrap(),
            kind: CodexExecutableKind::TrustedCmdShim,
            version: None,
        };
        let command = codex_command(&installation, app_server_arguments()).unwrap();
        let arguments: Vec<String> = command
            .as_std()
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect();
        assert_eq!(arguments[0], "/d");
        assert_eq!(arguments[1], "/c");
        assert_eq!(arguments[2], "call");
        assert!(!arguments[3].starts_with(r"\\?\"));
        assert_eq!(arguments[4], "app-server");

        let client = JsonRpcClient::spawn(command, JsonRpcConfig::default())
            .await
            .unwrap();
        let response = client
            .request(
                "initialize",
                Some(json!({"clientInfo":{"name":"test","version":"0"}})),
            )
            .await
            .unwrap();
        assert_eq!(response["userAgent"], "fake");
        client.shutdown().await.unwrap();
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    #[ignore = "requires the installed Codex App Server and an authenticated account"]
    async fn live_provider_probe_is_sanitized() {
        use crate::provider_manager::{
            channel, BackoffConfig, ProviderCommand, ProviderManagerConfig,
        };

        let provider = CodexProvider::default();
        let installation = provider.discover().await.unwrap();
        let kind = installation.kind();
        let version = installation.version().map(str::to_owned);
        let executable = diagnostic_path(&installation.path);
        let working_directory = std::env::current_dir()
            .map(|path| diagnostic_path(&path))
            .unwrap_or_else(|_| "unavailable".into());
        let mut session = provider.spawn(&installation).await.unwrap();
        let process_id = session.process_id().unwrap();
        let job_object = session.job_object_active();
        let initialize_sent = true;
        provider.initialize(&mut session).await.unwrap();
        let initialize_responded = true;
        let first = provider.refresh(&mut session).await.unwrap();
        let second = provider.refresh(&mut session).await.unwrap();
        let (protocol_errors, stderr_bytes) = session.diagnostic_counts();
        let sanitized_stderr = session.client.stderr_diagnostics();

        let durations: Vec<i64> = second
            .windows
            .iter()
            .filter_map(|window| window.duration_minutes.as_ref()?.as_i64())
            .collect();
        let all_percentages_valid = second.windows.iter().all(|window| {
            window
                .remaining_percent
                .as_ref()
                .and_then(Value::as_i64)
                .is_some_and(|value| (0..=100).contains(&value))
        });
        let all_resets_future_or_unknown = second.windows.iter().all(|window| {
            window
                .resets_at
                .as_ref()
                .and_then(Value::as_i64)
                .is_none_or(|value| value > unix_now())
        });
        assert_eq!(first.provider_id, ProviderId::Codex);
        assert!(durations.contains(&300));
        assert!(durations.contains(&10_080));
        assert!(all_percentages_valid);

        let CodexSession { client, .. } = session;
        let shutdown = client.shutdown().await.unwrap();
        #[cfg(windows)]
        let orphaned = crate::windows_job::process_is_running(process_id);
        #[cfg(not(windows))]
        let orphaned = false;
        assert!(!orphaned);

        let (manager, runtime, publication) = channel();
        let mut manager_state = publication.state_rx;
        let manager_task = tokio::spawn(runtime.run(
            CodexProvider::default(),
            ProviderManagerConfig {
                polling_interval: Duration::from_millis(250),
                backoff: BackoffConfig::default(),
            },
        ));
        manager.send(ProviderCommand::Start).await.unwrap();
        wait_for_ready(&mut manager_state).await;
        manager.send(ProviderCommand::Refresh).await.unwrap();
        wait_for_refresh_cycle(&mut manager_state).await;
        wait_for_refresh_cycle(&mut manager_state).await;
        manager.send(ProviderCommand::Shutdown).await.unwrap();
        time::timeout(Duration::from_secs(5), manager_task)
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            *manager_state.borrow(),
            crate::domain::ProviderState::Stopped
        ));

        println!(
            "{}",
            json!({
                "result": "success",
                "discovery": match kind { CodexExecutableKind::Native => "native", CodexExecutableKind::TrustedCmdShim => "trusted_cmd_shim" },
                "executable": executable,
                "versionDiagnostic": version,
                "spawnArguments": app_server_arguments(),
                "workingDirectory": working_directory,
                "initializeSent": initialize_sent,
                "initializeResponded": initialize_responded,
                "authenticatedAccountRead": true,
                "windowCount": second.windows.len(),
                "durationMinutes": durations,
                "hasFiveHour": true,
                "hasWeekly": true,
                "allRemainingPercentagesValid": all_percentages_valid,
                "allResetsFutureOrUnknown": all_resets_future_or_unknown,
                "manualRefreshSucceeded": true,
                "managerManualRefreshSucceeded": true,
                "managerPollingSucceeded": true,
                "jobObjectActive": job_object,
                "protocolErrorCount": protocol_errors,
                "sanitizedStderrBytes": stderr_bytes,
                "sanitizedStderr": sanitized_stderr,
                "exitCode": shutdown.exit.code,
                "orphanedAfterShutdown": orphaned
            })
        );
    }

    async fn wait_for_ready(state: &mut watch::Receiver<crate::domain::ProviderState>) {
        time::timeout(Duration::from_secs(15), async {
            loop {
                if matches!(*state.borrow(), crate::domain::ProviderState::Ready { .. }) {
                    return;
                }
                state.changed().await.unwrap();
            }
        })
        .await
        .unwrap();
    }

    async fn wait_for_refresh_cycle(state: &mut watch::Receiver<crate::domain::ProviderState>) {
        time::timeout(Duration::from_secs(15), async {
            loop {
                state.changed().await.unwrap();
                if matches!(
                    *state.borrow(),
                    crate::domain::ProviderState::Refreshing { .. }
                ) {
                    break;
                }
            }
            loop {
                state.changed().await.unwrap();
                if matches!(*state.borrow(), crate::domain::ProviderState::Ready { .. }) {
                    return;
                }
            }
        })
        .await
        .unwrap();
    }
}
