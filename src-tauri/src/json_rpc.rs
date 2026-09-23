use crate::sanitize::{sanitize_text, BoundedDiagnostics};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::{ChildStdin, Command};
use tokio::sync::{broadcast, mpsc, oneshot, watch};
use tokio::time::{self, Instant, MissedTickBehavior};

#[cfg(windows)]
use crate::windows_job::JobObject;

const DEFAULT_MAX_FRAME_BYTES: usize = 1024 * 1024;
const DEFAULT_MAX_STDERR_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone)]
pub struct JsonRpcConfig {
    pub request_timeout: Duration,
    pub shutdown_grace: Duration,
    pub max_frame_bytes: usize,
    pub max_stderr_bytes: usize,
}

impl Default for JsonRpcConfig {
    fn default() -> Self {
        Self {
            request_timeout: Duration::from_secs(8),
            shutdown_grace: Duration::from_millis(1_500),
            max_frame_bytes: DEFAULT_MAX_FRAME_BYTES,
            max_stderr_bytes: DEFAULT_MAX_STDERR_BYTES,
        }
    }
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum RpcError {
    #[error("spawn failure: {0}")]
    Spawn(String),
    #[error("transport error: {0}")]
    Transport(String),
    #[error("JSON-RPC request timed out: {method}")]
    Timeout { method: String },
    #[error("invalid JSON-RPC response: {0}")]
    InvalidResponse(String),
    #[error("JSON-RPC {code}: {message}")]
    Remote { code: i64, message: String },
    #[error("app-server exited: {0}")]
    ProcessExited(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessExit {
    pub code: Option<i32>,
    pub success: bool,
}

impl ProcessExit {
    pub fn summary(&self) -> String {
        match self.code {
            Some(code) => format!("exit code {code}"),
            None => "terminated without an exit code".into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RpcNotification {
    pub method: String,
    pub _params: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShutdownResult {
    pub graceful: bool,
    pub exit: ProcessExit,
}

pub struct JsonRpcClient {
    command_tx: mpsc::Sender<ClientCommand>,
    notification_tx: broadcast::Sender<RpcNotification>,
    exit_rx: watch::Receiver<Option<ProcessExit>>,
    force_tx: mpsc::Sender<()>,
    #[allow(dead_code)]
    diagnostics: Arc<Mutex<BoundedDiagnostics>>,
    #[allow(dead_code)]
    protocol_errors: Arc<AtomicUsize>,
    process_id: u32,
    #[allow(dead_code)]
    job_object_active: bool,
    config: JsonRpcConfig,
}

impl JsonRpcClient {
    pub async fn spawn(mut command: Command, config: JsonRpcConfig) -> Result<Self, RpcError> {
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = command
            .spawn()
            .map_err(|error| RpcError::Spawn(sanitize_text(&error.to_string())))?;
        let process_id = child
            .id()
            .ok_or_else(|| RpcError::Transport("spawned process has no process id".into()))?;

        #[cfg(windows)]
        let job = {
            let job = JobObject::kill_on_close()
                .map_err(|error| RpcError::Transport(sanitize_text(&error.to_string())))?;
            let process_handle = child.raw_handle().ok_or_else(|| {
                RpcError::Transport("spawned process has no native process handle".into())
            })?;
            if let Err(error) = job.assign_raw(process_handle as _) {
                let _ = child.start_kill();
                return Err(RpcError::Transport(format!(
                    "failed to attach app-server to kill-on-close job: {}",
                    sanitize_text(&error.to_string())
                )));
            }
            Some(job)
        };
        #[cfg(not(windows))]
        let job: Option<()> = None;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| RpcError::Transport("app-server stdin was not piped".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| RpcError::Transport("app-server stdout was not piped".into()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| RpcError::Transport("app-server stderr was not piped".into()))?;

        let diagnostics = Arc::new(Mutex::new(BoundedDiagnostics::new(config.max_stderr_bytes)));
        let protocol_errors = Arc::new(AtomicUsize::new(0));
        let (writer_tx, writer_rx) = mpsc::channel(64);
        let (inbound_tx, inbound_rx) = mpsc::channel(64);
        let (command_tx, command_rx) = mpsc::channel(64);
        let (notification_tx, _) = broadcast::channel(64);
        let (exit_tx, exit_rx) = watch::channel(None);
        let (force_tx, force_rx) = mpsc::channel(1);

        tokio::spawn(writer_task(stdin, writer_rx, inbound_tx.clone()));
        tokio::spawn(reader_task(
            stdout,
            inbound_tx.clone(),
            config.max_frame_bytes,
        ));
        tokio::spawn(stderr_task(stderr, Arc::clone(&diagnostics)));
        tokio::spawn(dispatcher_task(
            command_rx,
            writer_tx,
            inbound_rx,
            notification_tx.clone(),
            Arc::clone(&protocol_errors),
            config.request_timeout,
        ));
        tokio::spawn(supervisor_task(child, force_rx, exit_tx, inbound_tx, job));

        Ok(Self {
            command_tx,
            notification_tx,
            exit_rx,
            force_tx,
            diagnostics,
            protocol_errors,
            process_id,
            job_object_active: cfg!(windows),
            config,
        })
    }

    pub fn process_id(&self) -> u32 {
        self.process_id
    }

    #[allow(dead_code)]
    pub fn job_object_active(&self) -> bool {
        self.job_object_active
    }

    pub fn subscribe_notifications(&self) -> broadcast::Receiver<RpcNotification> {
        self.notification_tx.subscribe()
    }

    pub fn subscribe_exit(&self) -> watch::Receiver<Option<ProcessExit>> {
        self.exit_rx.clone()
    }

    #[allow(dead_code)]
    pub fn stderr_diagnostics(&self) -> String {
        self.diagnostics
            .lock()
            .map(|value| value.snapshot())
            .unwrap_or_else(|_| "diagnostic buffer unavailable".into())
    }

    #[allow(dead_code)]
    pub fn protocol_error_count(&self) -> usize {
        self.protocol_errors.load(Ordering::Relaxed)
    }

    pub async fn request(&self, method: &str, params: Option<Value>) -> Result<Value, RpcError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.command_tx
            .send(ClientCommand::Request {
                method: method.into(),
                params,
                reply: reply_tx,
            })
            .await
            .map_err(|_| RpcError::ProcessExited("request dispatcher stopped".into()))?;
        let result = reply_rx
            .await
            .map_err(|_| RpcError::ProcessExited("request dispatcher stopped".into()))?;
        match result {
            Ok(value) => Ok(value),
            Err(error) => Err(self.enrich_process_error(error).await),
        }
    }

    pub async fn notify(&self, method: &str, params: Option<Value>) -> Result<(), RpcError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.command_tx
            .send(ClientCommand::Notify {
                method: method.into(),
                params,
                reply: reply_tx,
            })
            .await
            .map_err(|_| RpcError::ProcessExited("request dispatcher stopped".into()))?;
        let result = reply_rx
            .await
            .map_err(|_| RpcError::ProcessExited("request dispatcher stopped".into()))?;
        match result {
            Ok(()) => Ok(()),
            Err(error) => Err(self.enrich_process_error(error).await),
        }
    }

    async fn enrich_process_error(&self, error: RpcError) -> RpcError {
        let RpcError::ProcessExited(reason) = error else {
            return error;
        };

        let mut exit_rx = self.exit_rx.clone();
        let current_exit = exit_rx.borrow().clone();
        let exit = if let Some(exit) = current_exit {
            Some(exit)
        } else {
            time::timeout(Duration::from_millis(500), wait_for_exit(&mut exit_rx))
                .await
                .ok()
                .and_then(Result::ok)
        };
        time::sleep(Duration::from_millis(10)).await;
        let stderr = self.stderr_diagnostics();
        RpcError::ProcessExited(process_failure_summary(&reason, exit.as_ref(), &stderr))
    }

    pub async fn shutdown(self) -> Result<ShutdownResult, RpcError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        let _ = self
            .command_tx
            .send(ClientCommand::Close { reply: reply_tx })
            .await;
        let _ = reply_rx.await;

        let mut exit_rx = self.exit_rx.clone();
        if let Some(exit) = exit_rx.borrow().clone() {
            return Ok(ShutdownResult {
                graceful: true,
                exit,
            });
        }

        let graceful = time::timeout(self.config.shutdown_grace, wait_for_exit(&mut exit_rx)).await;
        if let Ok(exit) = graceful {
            return Ok(ShutdownResult {
                graceful: true,
                exit: exit?,
            });
        }

        self.force_tx
            .send(())
            .await
            .map_err(|_| RpcError::ProcessExited("process supervisor stopped".into()))?;
        let exit = time::timeout(Duration::from_secs(5), wait_for_exit(&mut exit_rx))
            .await
            .map_err(|_| RpcError::Transport("forced shutdown timed out".into()))??;
        Ok(ShutdownResult {
            graceful: false,
            exit,
        })
    }
}

async fn wait_for_exit(
    exit_rx: &mut watch::Receiver<Option<ProcessExit>>,
) -> Result<ProcessExit, RpcError> {
    loop {
        if let Some(exit) = exit_rx.borrow().clone() {
            return Ok(exit);
        }
        exit_rx
            .changed()
            .await
            .map_err(|_| RpcError::ProcessExited("process supervisor stopped".into()))?;
    }
}

enum ClientCommand {
    Request {
        method: String,
        params: Option<Value>,
        reply: oneshot::Sender<Result<Value, RpcError>>,
    },
    Notify {
        method: String,
        params: Option<Value>,
        reply: oneshot::Sender<Result<(), RpcError>>,
    },
    Close {
        reply: oneshot::Sender<()>,
    },
}

enum WriterCommand {
    Frame(Vec<u8>),
    Close,
}

enum Inbound {
    Message(Value),
    ProtocolError(String),
    Closed(String),
}

struct PendingRequest {
    method: String,
    deadline: Instant,
    protocol_revision: usize,
    reply: oneshot::Sender<Result<Value, RpcError>>,
}

async fn dispatcher_task(
    mut command_rx: mpsc::Receiver<ClientCommand>,
    writer_tx: mpsc::Sender<WriterCommand>,
    mut inbound_rx: mpsc::Receiver<Inbound>,
    notification_tx: broadcast::Sender<RpcNotification>,
    protocol_errors: Arc<AtomicUsize>,
    request_timeout: Duration,
) {
    let mut next_id = 1_u64;
    let mut pending = HashMap::<u64, PendingRequest>::new();
    let mut protocol_revision = 0_usize;
    let mut last_protocol_error: Option<String> = None;
    let mut timer = time::interval(Duration::from_millis(25));
    timer.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            command = command_rx.recv() => match command {
                Some(ClientCommand::Request { method, params, reply }) => {
                    let id = next_id;
                    next_id = next_id.saturating_add(1);
                    let frame = encode_message(rpc_message(&method, Some(id), params));
                    if writer_tx.send(WriterCommand::Frame(frame)).await.is_err() {
                        let _ = reply.send(Err(RpcError::ProcessExited("stdin writer stopped".into())));
                        continue;
                    }
                    pending.insert(id, PendingRequest {
                        method,
                        deadline: Instant::now() + request_timeout,
                        protocol_revision,
                        reply,
                    });
                }
                Some(ClientCommand::Notify { method, params, reply }) => {
                    let frame = encode_message(rpc_message(&method, None, params));
                    let result = writer_tx.send(WriterCommand::Frame(frame)).await
                        .map_err(|_| RpcError::ProcessExited("stdin writer stopped".into()));
                    let _ = reply.send(result);
                }
                Some(ClientCommand::Close { reply }) => {
                    let _ = writer_tx.send(WriterCommand::Close).await;
                    let _ = reply.send(());
                }
                None => break,
            },
            inbound = inbound_rx.recv() => match inbound {
                Some(Inbound::Message(message)) => {
                    if let Some(id) = message.get("id").and_then(Value::as_u64) {
                        if let Some(request) = pending.remove(&id) {
                            let result = if let Some(error) = message.get("error") {
                                Err(RpcError::Remote {
                                    code: error.get("code").and_then(Value::as_i64).unwrap_or(-32_000),
                                    message: sanitize_text(
                                        error.get("message").and_then(Value::as_str).unwrap_or("remote error")
                                    ),
                                })
                            } else {
                                Ok(message.get("result").cloned().unwrap_or(Value::Null))
                            };
                            let _ = request.reply.send(result);
                        }
                    } else if let Some(method) = message.get("method").and_then(Value::as_str) {
                        let _ = notification_tx.send(RpcNotification {
                            method: method.into(),
                            _params: message.get("params").cloned(),
                        });
                    }
                }
                Some(Inbound::ProtocolError(reason)) => {
                    protocol_errors.fetch_add(1, Ordering::Relaxed);
                    protocol_revision = protocol_revision.saturating_add(1);
                    last_protocol_error = Some(sanitize_text(&reason));
                }
                Some(Inbound::Closed(reason)) => {
                    fail_pending_after_close(
                        &mut pending,
                        &reason,
                        protocol_revision,
                        last_protocol_error.as_deref(),
                    );
                    break;
                }
                None => {
                    fail_pending_after_close(
                        &mut pending,
                        "stdout reader stopped",
                        protocol_revision,
                        last_protocol_error.as_deref(),
                    );
                    break;
                }
            },
            _ = timer.tick() => {
                let now = Instant::now();
                let expired: Vec<u64> = pending.iter()
                    .filter_map(|(id, request)| (request.deadline <= now).then_some(*id))
                    .collect();
                for id in expired {
                    if let Some(request) = pending.remove(&id) {
                        let error = if request.protocol_revision < protocol_revision {
                            RpcError::InvalidResponse(
                                last_protocol_error
                                    .clone()
                                    .unwrap_or_else(|| "malformed protocol frame".into()),
                            )
                        } else {
                            RpcError::Timeout { method: request.method }
                        };
                        let _ = request.reply.send(Err(error));
                    }
                }
            }
        }
    }
    fail_pending(
        &mut pending,
        RpcError::ProcessExited("request dispatcher stopped".into()),
    );
}

fn fail_pending(pending: &mut HashMap<u64, PendingRequest>, error: RpcError) {
    for (_, request) in pending.drain() {
        let _ = request.reply.send(Err(error.clone()));
    }
}

fn fail_pending_after_close(
    pending: &mut HashMap<u64, PendingRequest>,
    reason: &str,
    protocol_revision: usize,
    last_protocol_error: Option<&str>,
) {
    for (_, request) in pending.drain() {
        let error = if request.protocol_revision < protocol_revision {
            RpcError::InvalidResponse(
                last_protocol_error
                    .map(sanitize_text)
                    .unwrap_or_else(|| "malformed protocol frame".into()),
            )
        } else {
            RpcError::ProcessExited(sanitize_text(reason))
        };
        let _ = request.reply.send(Err(error));
    }
}

fn process_failure_summary(reason: &str, exit: Option<&ProcessExit>, stderr: &str) -> String {
    let reason = sanitize_text(reason);
    let exit = exit
        .map(ProcessExit::summary)
        .unwrap_or_else(|| "exit code unavailable".into());
    let stderr = sanitize_text(stderr.trim());
    if stderr.is_empty() {
        sanitize_text(&format!("{reason}; {exit}; stderr empty"))
    } else {
        sanitize_text(&format!("{reason}; {exit}; stderr: {stderr}"))
    }
}

fn encode_message(value: Value) -> Vec<u8> {
    let mut frame = serde_json::to_vec(&value).expect("JSON-RPC values are serializable");
    frame.push(b'\n');
    frame
}

fn rpc_message(method: &str, id: Option<u64>, params: Option<Value>) -> Value {
    let mut message = Map::new();
    message.insert("method".into(), Value::String(method.into()));
    if let Some(id) = id {
        message.insert("id".into(), Value::Number(id.into()));
    }
    if let Some(params) = params {
        message.insert("params".into(), params);
    }
    Value::Object(message)
}

async fn writer_task(
    mut stdin: ChildStdin,
    mut writer_rx: mpsc::Receiver<WriterCommand>,
    inbound_tx: mpsc::Sender<Inbound>,
) {
    while let Some(command) = writer_rx.recv().await {
        let result = match command {
            WriterCommand::Frame(frame) => stdin.write_all(&frame).await.and_then(|_| Ok(())),
            WriterCommand::Close => {
                let _ = stdin.shutdown().await;
                return;
            }
        };
        if let Err(error) = result {
            let _ = inbound_tx
                .send(Inbound::Closed(sanitize_text(&error.to_string())))
                .await;
            return;
        }
        if let Err(error) = stdin.flush().await {
            let _ = inbound_tx
                .send(Inbound::Closed(sanitize_text(&error.to_string())))
                .await;
            return;
        }
    }
    let _ = stdin.shutdown().await;
}

async fn reader_task<R>(mut stdout: R, inbound_tx: mpsc::Sender<Inbound>, max_frame: usize)
where
    R: AsyncRead + Unpin,
{
    let mut decoder = JsonlDecoder::new(max_frame);
    let mut chunk = [0_u8; 8 * 1024];
    loop {
        match stdout.read(&mut chunk).await {
            Ok(0) => {
                if let Some(error) = decoder.finish() {
                    let _ = inbound_tx.send(Inbound::ProtocolError(error)).await;
                }
                let _ = inbound_tx
                    .send(Inbound::Closed("stdout reached EOF".into()))
                    .await;
                return;
            }
            Ok(size) => {
                for frame in decoder.push(&chunk[..size]) {
                    let inbound = match frame {
                        Ok(value) => Inbound::Message(value),
                        Err(error) => Inbound::ProtocolError(error),
                    };
                    if inbound_tx.send(inbound).await.is_err() {
                        return;
                    }
                }
            }
            Err(error) => {
                let _ = inbound_tx
                    .send(Inbound::Closed(sanitize_text(&error.to_string())))
                    .await;
                return;
            }
        }
    }
}

async fn stderr_task<R>(mut stderr: R, diagnostics: Arc<Mutex<BoundedDiagnostics>>)
where
    R: AsyncRead + Unpin,
{
    let mut chunk = [0_u8; 4 * 1024];
    loop {
        match stderr.read(&mut chunk).await {
            Ok(0) | Err(_) => return,
            Ok(size) => {
                if let Ok(mut buffer) = diagnostics.lock() {
                    buffer.push(&chunk[..size]);
                }
            }
        }
    }
}

async fn supervisor_task(
    mut child: tokio::process::Child,
    mut force_rx: mpsc::Receiver<()>,
    exit_tx: watch::Sender<Option<ProcessExit>>,
    inbound_tx: mpsc::Sender<Inbound>,
    #[cfg(windows)] job: Option<JobObject>,
    #[cfg(not(windows))] _job: Option<()>,
) {
    let status = tokio::select! {
        status = child.wait() => status,
        _ = force_rx.recv() => {
            #[cfg(windows)]
            if let Some(job) = job.as_ref() {
                let _ = job.terminate(1);
            }
            #[cfg(not(windows))]
            let _ = child.start_kill();
            child.wait().await
        }
    };

    let exit = match status {
        Ok(status) => ProcessExit {
            code: status.code(),
            success: status.success(),
        },
        Err(error) => {
            let reason = sanitize_text(&error.to_string());
            let _ = inbound_tx.send(Inbound::Closed(reason.clone())).await;
            ProcessExit {
                code: None,
                success: false,
            }
        }
    };
    let _ = exit_tx.send(Some(exit.clone()));
    let _ = inbound_tx.send(Inbound::Closed(exit.summary())).await;
}

#[derive(Debug)]
struct JsonlDecoder {
    buffer: Vec<u8>,
    max_frame: usize,
    discarding_oversized: bool,
}

impl JsonlDecoder {
    fn new(max_frame: usize) -> Self {
        Self {
            buffer: Vec::new(),
            max_frame,
            discarding_oversized: false,
        }
    }

    fn push(&mut self, bytes: &[u8]) -> Vec<Result<Value, String>> {
        let mut frames = Vec::new();
        for byte in bytes {
            if *byte == b'\n' {
                if self.discarding_oversized {
                    frames.push(Err("frame exceeded configured limit".into()));
                    self.discarding_oversized = false;
                    self.buffer.clear();
                    continue;
                }
                if self.buffer.last() == Some(&b'\r') {
                    self.buffer.pop();
                }
                if !self.buffer.is_empty() {
                    frames.push(
                        serde_json::from_slice(&self.buffer)
                            .map_err(|_| "invalid JSON frame".into()),
                    );
                }
                self.buffer.clear();
            } else if !self.discarding_oversized {
                self.buffer.push(*byte);
                if self.buffer.len() > self.max_frame {
                    self.buffer.clear();
                    self.discarding_oversized = true;
                }
            }
        }
        frames
    }

    fn finish(&mut self) -> Option<String> {
        if self.discarding_oversized {
            Some("oversized frame ended at EOF".into())
        } else if self.buffer.is_empty() {
            None
        } else {
            Some("partial frame ended at EOF".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    #[test]
    fn incremental_decoder_handles_partial_and_multiple_frames() {
        let mut decoder = JsonlDecoder::new(1024);
        assert!(decoder.push(b"{\"id\":").is_empty());
        let frames = decoder.push(b"1}\n{\"method\":\"updated\"}\n");
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].as_ref().unwrap()["id"], 1);
        assert_eq!(frames[1].as_ref().unwrap()["method"], "updated");
    }

    #[test]
    fn decoder_isolates_invalid_and_oversized_frames() {
        let mut decoder = JsonlDecoder::new(8);
        let frames = decoder.push(b"not-json\n{\"tooLarge\":true}\n{}\n");
        assert_eq!(frames.len(), 3);
        assert!(frames[0].is_err());
        assert!(frames[1].is_err());
        assert!(frames[2].is_ok());
    }

    fn fixture_command(mode: &str) -> Command {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("spike")
            .join("codex-app-server")
            .join("fixtures")
            .join("fake-server.mjs");
        let mut command = Command::new("node.exe");
        command.arg(fixture).arg(mode);
        command
    }

    #[tokio::test]
    async fn correlates_requests_times_out_and_drains_stderr() {
        let config = JsonRpcConfig {
            request_timeout: Duration::from_secs(2),
            max_stderr_bytes: 4_096,
            ..Default::default()
        };
        let client = JsonRpcClient::spawn(fixture_command("stderr-flood"), config)
            .await
            .unwrap();
        let initialized = client
            .request(
                "initialize",
                Some(json!({"clientInfo":{"name":"test","version":"0"}})),
            )
            .await
            .unwrap();
        assert_eq!(initialized["userAgent"], "fake");
        let (first, second) = tokio::join!(
            client.request("account/rateLimits/read", None),
            client.request("account/rateLimits/read", None)
        );
        assert_eq!(first.unwrap()["rateLimits"]["limitId"], "codex");
        assert_eq!(second.unwrap()["rateLimits"]["limitId"], "codex");
        assert!(matches!(
            client.request("test/no-response", None).await,
            Err(RpcError::Timeout { .. })
        ));
        assert_eq!(
            client
                .request("account/rateLimits/read", None)
                .await
                .unwrap()["rateLimits"]["limitId"],
            "codex"
        );
        time::sleep(Duration::from_millis(25)).await;
        assert!(client.stderr_diagnostics().len() <= 4_096);
        assert!(client.job_object_active());
        assert!(client.shutdown().await.unwrap().graceful);
    }

    #[tokio::test]
    async fn malformed_frames_are_isolated() {
        let client = JsonRpcClient::spawn(fixture_command("malformed"), Default::default())
            .await
            .unwrap();
        client
            .request(
                "initialize",
                Some(json!({"clientInfo":{"name":"test","version":"0"}})),
            )
            .await
            .unwrap();
        assert_eq!(client.protocol_error_count(), 1);
        client.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn unexpected_exit_preserves_code_and_sanitized_stderr() {
        let client = JsonRpcClient::spawn(fixture_command("crash-stderr"), Default::default())
            .await
            .unwrap();
        let error = client
            .request(
                "initialize",
                Some(json!({"clientInfo":{"name":"test","version":"0"}})),
            )
            .await
            .unwrap_err();
        let text = error.to_string();
        assert!(matches!(error, RpcError::ProcessExited(_)));
        assert!(text.contains("exit code 23"));
        assert!(text.contains("[REDACTED]"));
        assert!(text.contains("email=[REDACTED]"));
        assert!(!text.contains("fixture-secret"));
        assert!(!text.contains("fixture@example.com"));
    }

    #[tokio::test]
    async fn malformed_response_is_distinct_from_eof_and_timeout() {
        let client = JsonRpcClient::spawn(fixture_command("invalid-only"), Default::default())
            .await
            .unwrap();
        let error = client
            .request(
                "initialize",
                Some(json!({"clientInfo":{"name":"test","version":"0"}})),
            )
            .await
            .unwrap_err();
        assert!(matches!(error, RpcError::InvalidResponse(_)));
    }

    #[tokio::test]
    async fn forced_shutdown_terminates_hanging_process_tree() {
        let config = JsonRpcConfig {
            shutdown_grace: Duration::from_millis(100),
            ..Default::default()
        };
        let client = JsonRpcClient::spawn(fixture_command("tree-hang"), config)
            .await
            .unwrap();
        let mut notifications = client.subscribe_notifications();
        client
            .request(
                "initialize",
                Some(json!({"clientInfo":{"name":"test","version":"0"}})),
            )
            .await
            .unwrap();
        let note = time::timeout(Duration::from_secs(2), notifications.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(note.method, "test/grandchild");
        let shutdown = client.shutdown().await.unwrap();
        assert!(!shutdown.graceful);
    }
}
