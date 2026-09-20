use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ProviderId {
    Mock,
    Codex,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum QuotaWindowKind {
    Rolling,
    Weekly,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct QuotaWindow {
    pub id: String,
    pub kind: QuotaWindowKind,
    pub label: Option<String>,
    /// Provider payload values stay open until the App Server schema is bound.
    pub duration_minutes: Option<serde_json::Value>,
    pub used_percent: Option<serde_json::Value>,
    pub remaining_percent: Option<serde_json::Value>,
    pub resets_at: Option<serde_json::Value>,
    pub limit_status: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct QuotaSnapshot {
    pub provider_id: ProviderId,
    pub fetched_at: i64,
    pub windows: Vec<QuotaWindow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum ProviderState {
    Stopped,
    Discovering,
    Starting {
        executable: String,
    },
    Handshaking {
        process_id: Option<u32>,
    },
    Refreshing {
        previous_snapshot: Option<QuotaSnapshot>,
    },
    Ready {
        snapshot: QuotaSnapshot,
    },
    Degraded {
        snapshot: Option<QuotaSnapshot>,
        error: String,
        retry_at: Option<i64>,
    },
    NeedsAuthentication,
    Unsupported {
        missing_capabilities: Vec<String>,
        version: Option<String>,
    },
    Unavailable {
        reason: String,
    },
    Stopping,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderEvent {
    StartRequested,
    ExecutableFound {
        path: String,
    },
    ExecutableNotFound {
        reason: String,
    },
    DiscoveryFailed {
        error: String,
        retry_at: Option<i64>,
    },
    ProcessStarted {
        process_id: Option<u32>,
    },
    ProcessStartFailed {
        error: String,
        retry_at: Option<i64>,
    },
    HandshakeSucceeded,
    HandshakeFailed {
        error: String,
        retry_at: Option<i64>,
    },
    RefreshRequested,
    RefreshSucceeded {
        snapshot: QuotaSnapshot,
    },
    RefreshFailed {
        error: String,
        retry_at: Option<i64>,
    },
    AuthenticationRequired,
    CapabilitiesMissing {
        missing: Vec<String>,
        version: Option<String>,
    },
    ProcessExited {
        reason: String,
        retry_at: Option<i64>,
    },
    RetryBackoffElapsed,
    ShutdownRequested,
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidTransition;

impl ProviderState {
    pub fn snapshot(&self) -> Option<&QuotaSnapshot> {
        match self {
            Self::Ready { snapshot }
            | Self::Refreshing {
                previous_snapshot: Some(snapshot),
            }
            | Self::Degraded {
                snapshot: Some(snapshot),
                ..
            } => Some(snapshot),
            _ => None,
        }
    }

    pub fn transition(self, event: ProviderEvent) -> Result<Self, InvalidTransition> {
        use ProviderEvent::*;
        use ProviderState::*;
        match (self, event) {
            (ProviderState::Stopped, StartRequested) => Ok(Discovering),
            (Discovering, ExecutableFound { path }) => Ok(Starting { executable: path }),
            (Discovering, ExecutableNotFound { reason }) => Ok(Unavailable { reason }),
            (Discovering, DiscoveryFailed { error, retry_at }) => Ok(Degraded {
                snapshot: None,
                error,
                retry_at,
            }),
            (Starting { .. }, ProcessStarted { process_id }) => Ok(Handshaking { process_id }),
            (Starting { .. }, ProcessStartFailed { error, retry_at }) => Ok(Degraded {
                snapshot: None,
                error,
                retry_at,
            }),
            (Handshaking { .. }, HandshakeSucceeded) => Ok(Refreshing {
                previous_snapshot: None,
            }),
            (Handshaking { .. }, HandshakeFailed { error, retry_at }) => Ok(Degraded {
                snapshot: None,
                error,
                retry_at,
            }),
            (Refreshing { previous_snapshot }, RefreshRequested) => {
                Ok(Refreshing { previous_snapshot })
            }
            (Refreshing { .. }, RefreshSucceeded { snapshot }) => Ok(Ready { snapshot }),
            (Refreshing { previous_snapshot }, RefreshFailed { error, retry_at }) => Ok(Degraded {
                snapshot: previous_snapshot,
                error,
                retry_at,
            }),
            (Handshaking { .. }, AuthenticationRequired) => Ok(NeedsAuthentication),
            (Refreshing { .. }, AuthenticationRequired) => Ok(NeedsAuthentication),
            (Ready { .. }, AuthenticationRequired) => Ok(NeedsAuthentication),
            (
                Handshaking { .. } | Refreshing { .. } | Ready { .. },
                CapabilitiesMissing { missing, version },
            ) => Ok(Unsupported {
                missing_capabilities: missing,
                version,
            }),
            (Ready { snapshot }, RefreshRequested) => Ok(Refreshing {
                previous_snapshot: Some(snapshot),
            }),
            (Ready { snapshot }, ProcessExited { reason, retry_at }) => Ok(Degraded {
                snapshot: Some(snapshot),
                error: reason,
                retry_at,
            }),
            (Refreshing { previous_snapshot }, ProcessExited { reason, retry_at }) => {
                Ok(Degraded {
                    snapshot: previous_snapshot,
                    error: reason,
                    retry_at,
                })
            }
            (Starting { .. }, ProcessExited { reason, retry_at })
            | (Handshaking { .. }, ProcessExited { reason, retry_at }) => Ok(Degraded {
                snapshot: None,
                error: reason,
                retry_at,
            }),
            (Degraded { snapshot, .. }, RefreshRequested) => Ok(Refreshing {
                previous_snapshot: snapshot,
            }),
            (Degraded { snapshot, .. }, RefreshSucceeded { snapshot: fresh }) => {
                let _ = snapshot;
                Ok(Ready { snapshot: fresh })
            }
            (Degraded { .. }, RetryBackoffElapsed) => Ok(Discovering),
            (NeedsAuthentication, StartRequested) => Ok(Discovering),
            (Unsupported { .. }, StartRequested) => Ok(Discovering),
            (Unavailable { .. }, StartRequested) => Ok(Discovering),
            (
                Discovering
                | Starting { .. }
                | Handshaking { .. }
                | Refreshing { .. }
                | Ready { .. }
                | Degraded { .. }
                | NeedsAuthentication
                | Unsupported { .. }
                | Unavailable { .. },
                ShutdownRequested,
            ) => Ok(Stopping),
            (ProviderState::Stopped, ShutdownRequested) => Ok(ProviderState::Stopped),
            (Stopping, ProviderEvent::Stopped) => Ok(ProviderState::Stopped),
            _ => Err(InvalidTransition),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> QuotaSnapshot {
        QuotaSnapshot {
            provider_id: ProviderId::Mock,
            fetched_at: 1,
            windows: vec![QuotaWindow {
                id: "five-hour".into(),
                kind: QuotaWindowKind::Rolling,
                label: Some("5 hours".into()),
                duration_minutes: Some(serde_json::json!(300)),
                used_percent: Some(serde_json::json!(25)),
                remaining_percent: Some(serde_json::json!(75)),
                resets_at: Some(serde_json::json!(2)),
                limit_status: serde_json::json!("available"),
            }],
        }
    }

    #[test]
    fn follows_start_handshake_refresh_ready_path() {
        let state = ProviderState::Stopped
            .transition(ProviderEvent::StartRequested)
            .unwrap();
        let state = state
            .transition(ProviderEvent::ExecutableFound {
                path: "mock".into(),
            })
            .unwrap();
        let state = state
            .transition(ProviderEvent::ProcessStarted { process_id: None })
            .unwrap();
        let state = state.transition(ProviderEvent::HandshakeSucceeded).unwrap();
        let state = state
            .transition(ProviderEvent::RefreshSucceeded {
                snapshot: snapshot(),
            })
            .unwrap();
        assert!(matches!(state, ProviderState::Ready { .. }));
    }

    #[test]
    fn preserves_last_snapshot_when_refresh_fails() {
        let ready = ProviderState::Ready {
            snapshot: snapshot(),
        };
        let refreshing = ready.transition(ProviderEvent::RefreshRequested).unwrap();
        let degraded = refreshing
            .transition(ProviderEvent::RefreshFailed {
                error: "temporary".into(),
                retry_at: Some(5),
            })
            .unwrap();
        assert!(matches!(
            degraded,
            ProviderState::Degraded {
                snapshot: Some(_),
                ..
            }
        ));
    }

    #[test]
    fn rejects_invalid_transition() {
        assert!(ProviderState::Stopped
            .transition(ProviderEvent::RefreshRequested)
            .is_err());
    }

    #[test]
    fn models_auth_and_capability_failures_separately() {
        let auth = ProviderState::Handshaking { process_id: None }
            .transition(ProviderEvent::AuthenticationRequired)
            .unwrap();
        let auth_after_initialization = ProviderState::Ready {
            snapshot: snapshot(),
        }
        .transition(ProviderEvent::AuthenticationRequired)
        .unwrap();
        let unsupported = ProviderState::Handshaking { process_id: None }
            .transition(ProviderEvent::CapabilitiesMissing {
                missing: vec!["account/rateLimits/read".into()],
                version: Some("diagnostic".into()),
            })
            .unwrap();
        assert!(matches!(auth, ProviderState::NeedsAuthentication));
        assert!(matches!(
            auth_after_initialization,
            ProviderState::NeedsAuthentication
        ));
        assert!(matches!(unsupported, ProviderState::Unsupported { .. }));
    }

    #[test]
    fn distinguishes_discovery_startup_and_handshake_failures() {
        let unavailable = ProviderState::Discovering
            .transition(ProviderEvent::ExecutableNotFound {
                reason: "missing".into(),
            })
            .unwrap();
        let discovery_failure = ProviderState::Discovering
            .transition(ProviderEvent::DiscoveryFailed {
                error: "locator timed out".into(),
                retry_at: Some(1),
            })
            .unwrap();
        let startup = ProviderState::Starting {
            executable: "codex".into(),
        }
        .transition(ProviderEvent::ProcessStartFailed {
            error: "failed".into(),
            retry_at: Some(2),
        })
        .unwrap();
        let handshake = ProviderState::Handshaking {
            process_id: Some(7),
        }
        .transition(ProviderEvent::HandshakeFailed {
            error: "timeout".into(),
            retry_at: Some(3),
        })
        .unwrap();
        assert!(matches!(unavailable, ProviderState::Unavailable { .. }));
        assert!(matches!(
            discovery_failure,
            ProviderState::Degraded { snapshot: None, .. }
        ));
        assert!(matches!(
            startup,
            ProviderState::Degraded { snapshot: None, .. }
        ));
        assert!(matches!(
            handshake,
            ProviderState::Degraded { snapshot: None, .. }
        ));
    }

    #[test]
    fn supports_retry_and_pre_ready_process_exit() {
        let retrying = ProviderState::Degraded {
            snapshot: None,
            error: "crash".into(),
            retry_at: Some(4),
        };
        assert!(matches!(
            retrying
                .transition(ProviderEvent::RetryBackoffElapsed)
                .unwrap(),
            ProviderState::Discovering
        ));
        let exited = ProviderState::Starting {
            executable: "codex".into(),
        }
        .transition(ProviderEvent::ProcessExited {
            reason: "exit".into(),
            retry_at: Some(5),
        })
        .unwrap();
        assert!(matches!(
            exited,
            ProviderState::Degraded { snapshot: None, .. }
        ));
    }

    #[test]
    fn preserves_previous_snapshot_when_process_exits_during_refresh() {
        let previous = snapshot();
        let degraded = ProviderState::Refreshing {
            previous_snapshot: Some(previous.clone()),
        }
        .transition(ProviderEvent::ProcessExited {
            reason: "process exited".into(),
            retry_at: Some(10),
        })
        .unwrap();

        assert_eq!(degraded.snapshot(), Some(&previous));
        assert!(matches!(degraded, ProviderState::Degraded { .. }));
    }
}
