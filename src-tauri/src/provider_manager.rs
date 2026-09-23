use crate::domain::{ProviderEvent, ProviderState, QuotaSnapshot};
use async_trait::async_trait;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, watch};
use tokio::time::{self, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderCommand {
    Start,
    Refresh,
    Shutdown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderFailureKind {
    Unavailable,
    Authentication,
    Unsupported { missing: Vec<String> },
    Transient,
    ProcessExited,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderFailure {
    pub kind: ProviderFailureKind,
    pub message: String,
    pub version: Option<String>,
}

impl ProviderFailure {
    #[allow(dead_code)]
    pub fn transient(message: impl Into<String>) -> Self {
        Self {
            kind: ProviderFailureKind::Transient,
            message: message.into(),
            version: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionSignal {
    RefreshSuggested,
    ProcessExited(String),
}

#[async_trait]
pub trait ProviderSession: Send {
    fn process_id(&self) -> Option<u32>;
    async fn next_signal(&mut self) -> SessionSignal;
    async fn shutdown(self) -> Result<(), ProviderFailure>;
}

#[async_trait]
pub trait ProviderDriver: Send + Sync + 'static {
    type Installation: Clone + Send + Sync + 'static;
    type Session: ProviderSession;

    async fn discover(&self) -> Result<Self::Installation, ProviderFailure>;
    fn executable_label(&self, installation: &Self::Installation) -> String;
    fn version(&self, installation: &Self::Installation) -> Option<String>;
    async fn spawn(
        &self,
        installation: &Self::Installation,
    ) -> Result<Self::Session, ProviderFailure>;
    async fn initialize(&self, session: &mut Self::Session) -> Result<(), ProviderFailure>;
    async fn refresh(&self, session: &mut Self::Session) -> Result<QuotaSnapshot, ProviderFailure>;
}

#[derive(Debug, Clone)]
pub struct BackoffConfig {
    pub initial: Duration,
    pub maximum: Duration,
    pub jitter_ratio: f64,
}

impl Default for BackoffConfig {
    fn default() -> Self {
        Self {
            initial: Duration::from_secs(1),
            maximum: Duration::from_secs(60),
            jitter_ratio: 0.2,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProviderManagerConfig {
    pub polling_interval: Duration,
    pub backoff: BackoffConfig,
}

impl Default for ProviderManagerConfig {
    fn default() -> Self {
        Self {
            polling_interval: Duration::from_secs(60),
            backoff: BackoffConfig::default(),
        }
    }
}

#[derive(Debug)]
struct Backoff {
    attempt: u32,
    config: BackoffConfig,
}

impl Backoff {
    fn new(config: BackoffConfig) -> Self {
        Self { attempt: 0, config }
    }

    fn reset(&mut self) {
        self.attempt = 0;
    }

    fn next_delay(&mut self) -> Duration {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos() as u64;
        self.next_delay_with_seed(seed)
    }

    fn next_delay_with_seed(&mut self, seed: u64) -> Duration {
        let multiplier = 1_u32.checked_shl(self.attempt.min(20)).unwrap_or(u32::MAX);
        self.attempt = self.attempt.saturating_add(1);
        let base = self
            .config
            .initial
            .saturating_mul(multiplier)
            .min(self.config.maximum);
        let unit = (seed % 10_001) as f64 / 10_000.0;
        let factor = 1.0 + ((unit * 2.0) - 1.0) * self.config.jitter_ratio;
        base.mul_f64(factor.max(0.0)).min(self.config.maximum)
    }
}

#[derive(Clone)]
pub struct ProviderManagerHandle {
    command_tx: mpsc::Sender<ProviderCommand>,
}

pub struct ProviderManagerRuntime {
    command_rx: mpsc::Receiver<ProviderCommand>,
    state_tx: watch::Sender<ProviderState>,
}

pub struct ProviderPublicationReceiver {
    pub state_rx: watch::Receiver<ProviderState>,
}

pub fn channel() -> (
    ProviderManagerHandle,
    ProviderManagerRuntime,
    ProviderPublicationReceiver,
) {
    let (command_tx, command_rx) = mpsc::channel(16);
    let (state_tx, state_rx) = watch::channel(ProviderState::Stopped);
    (
        ProviderManagerHandle { command_tx },
        ProviderManagerRuntime {
            command_rx,
            state_tx,
        },
        ProviderPublicationReceiver { state_rx },
    )
}

impl ProviderManagerHandle {
    pub fn try_send(
        &self,
        command: ProviderCommand,
    ) -> Result<(), mpsc::error::TrySendError<ProviderCommand>> {
        self.command_tx.try_send(command)
    }

    pub async fn send(
        &self,
        command: ProviderCommand,
    ) -> Result<(), mpsc::error::SendError<ProviderCommand>> {
        self.command_tx.send(command).await
    }
}

impl ProviderManagerRuntime {
    pub async fn run<D>(mut self, driver: D, config: ProviderManagerConfig)
    where
        D: ProviderDriver,
    {
        let mut state = ProviderState::Stopped;
        let mut session: Option<D::Session> = None;
        let mut retry_at: Option<Instant> = None;
        let mut next_poll: Option<Instant> = None;
        let mut backoff = Backoff::new(config.backoff.clone());

        loop {
            let action = if let Some(deadline) = retry_at {
                tokio::select! {
                    command = self.command_rx.recv() => match command {
                        Some(command) => ManagerAction::Command(command),
                        None => ManagerAction::Command(ProviderCommand::Shutdown),
                    },
                    _ = time::sleep_until(deadline) => ManagerAction::Retry,
                }
            } else if let Some(active) = session.as_mut() {
                let poll_at = next_poll.unwrap_or_else(|| Instant::now() + config.polling_interval);
                tokio::select! {
                    command = self.command_rx.recv() => match command {
                        Some(command) => ManagerAction::Command(command),
                        None => ManagerAction::Command(ProviderCommand::Shutdown),
                    },
                    _ = time::sleep_until(poll_at) => ManagerAction::Refresh,
                    signal = active.next_signal() => ManagerAction::Signal(signal),
                }
            } else {
                match self.command_rx.recv().await {
                    Some(command) => ManagerAction::Command(command),
                    None => ManagerAction::Command(ProviderCommand::Shutdown),
                }
            };

            match action {
                ManagerAction::Command(ProviderCommand::Shutdown) => {
                    self.transition(&mut state, ProviderEvent::ShutdownRequested);
                    if let Some(active) = session.take() {
                        let _ = active.shutdown().await;
                    }
                    self.transition(&mut state, ProviderEvent::Stopped);
                    return;
                }
                ManagerAction::Command(ProviderCommand::Start) => {
                    if matches!(
                        state,
                        ProviderState::Ready { .. } | ProviderState::Refreshing { .. }
                    ) {
                        continue;
                    }
                    retry_at = None;
                    if matches!(state, ProviderState::Degraded { .. }) {
                        self.transition(&mut state, ProviderEvent::RetryBackoffElapsed);
                    } else {
                        self.transition(&mut state, ProviderEvent::StartRequested);
                    }
                    session = self
                        .connect(&driver, &mut state, &mut backoff, &mut retry_at)
                        .await;
                    if session.is_some() && matches!(state, ProviderState::Ready { .. }) {
                        next_poll = Some(Instant::now() + config.polling_interval);
                    }
                }
                ManagerAction::Command(ProviderCommand::Refresh) => {
                    if session.is_some() {
                        self.transition(&mut state, ProviderEvent::RefreshRequested);
                        self.refresh(
                            &driver,
                            &mut session,
                            &mut state,
                            &mut backoff,
                            &mut retry_at,
                        )
                        .await;
                        next_poll = session
                            .as_ref()
                            .map(|_| Instant::now() + config.polling_interval);
                    } else if matches!(
                        state,
                        ProviderState::Degraded { .. }
                            | ProviderState::Unavailable { .. }
                            | ProviderState::NeedsAuthentication
                            | ProviderState::Unsupported { .. }
                            | ProviderState::Stopped
                    ) {
                        retry_at = None;
                        if matches!(state, ProviderState::Degraded { .. }) {
                            self.transition(&mut state, ProviderEvent::RetryBackoffElapsed);
                        } else {
                            self.transition(&mut state, ProviderEvent::StartRequested);
                        }
                        session = self
                            .connect(&driver, &mut state, &mut backoff, &mut retry_at)
                            .await;
                        next_poll = session
                            .as_ref()
                            .map(|_| Instant::now() + config.polling_interval);
                    }
                }
                ManagerAction::Refresh | ManagerAction::Signal(SessionSignal::RefreshSuggested) => {
                    if session.is_some() {
                        self.transition(&mut state, ProviderEvent::RefreshRequested);
                        self.refresh(
                            &driver,
                            &mut session,
                            &mut state,
                            &mut backoff,
                            &mut retry_at,
                        )
                        .await;
                        next_poll = session
                            .as_ref()
                            .map(|_| Instant::now() + config.polling_interval);
                    }
                }
                ManagerAction::Signal(SessionSignal::ProcessExited(reason)) => {
                    let delay = backoff.next_delay();
                    self.transition(
                        &mut state,
                        ProviderEvent::ProcessExited {
                            reason,
                            retry_at: Some(unix_after(delay)),
                        },
                    );
                    session = None;
                    retry_at = Some(Instant::now() + delay);
                    next_poll = None;
                }
                ManagerAction::Retry => {
                    retry_at = None;
                    self.transition(&mut state, ProviderEvent::RetryBackoffElapsed);
                    session = self
                        .connect(&driver, &mut state, &mut backoff, &mut retry_at)
                        .await;
                    next_poll = session
                        .as_ref()
                        .map(|_| Instant::now() + config.polling_interval);
                }
            }
        }
    }

    async fn connect<D>(
        &self,
        driver: &D,
        state: &mut ProviderState,
        backoff: &mut Backoff,
        retry_at: &mut Option<Instant>,
    ) -> Option<D::Session>
    where
        D: ProviderDriver,
    {
        let installation = match driver.discover().await {
            Ok(installation) => installation,
            Err(error) => {
                if error.kind == ProviderFailureKind::Unavailable {
                    self.transition(
                        state,
                        ProviderEvent::ExecutableNotFound {
                            reason: error.message,
                        },
                    );
                } else {
                    let delay = backoff.next_delay();
                    self.transition(
                        state,
                        ProviderEvent::DiscoveryFailed {
                            error: error.message,
                            retry_at: Some(unix_after(delay)),
                        },
                    );
                    *retry_at = Some(Instant::now() + delay);
                }
                return None;
            }
        };
        self.transition(
            state,
            ProviderEvent::ExecutableFound {
                path: driver.executable_label(&installation),
            },
        );

        let mut session = match driver.spawn(&installation).await {
            Ok(session) => session,
            Err(error) => {
                let delay = backoff.next_delay();
                self.transition(
                    state,
                    ProviderEvent::ProcessStartFailed {
                        error: error.message,
                        retry_at: Some(unix_after(delay)),
                    },
                );
                *retry_at = Some(Instant::now() + delay);
                return None;
            }
        };
        self.transition(
            state,
            ProviderEvent::ProcessStarted {
                process_id: session.process_id(),
            },
        );

        if let Err(error) = driver.initialize(&mut session).await {
            let delay = backoff.next_delay();
            let event = match error.kind {
                ProviderFailureKind::Authentication => ProviderEvent::AuthenticationRequired,
                ProviderFailureKind::Unsupported { missing } => {
                    ProviderEvent::CapabilitiesMissing {
                        missing,
                        version: error.version.or_else(|| driver.version(&installation)),
                    }
                }
                ProviderFailureKind::ProcessExited => ProviderEvent::ProcessExited {
                    reason: error.message,
                    retry_at: Some(unix_after(delay)),
                },
                _ => ProviderEvent::HandshakeFailed {
                    error: error.message,
                    retry_at: Some(unix_after(delay)),
                },
            };
            self.transition(state, event);
            let terminal = matches!(
                state,
                ProviderState::NeedsAuthentication | ProviderState::Unsupported { .. }
            );
            let _ = session.shutdown().await;
            if !terminal {
                *retry_at = Some(Instant::now() + delay);
            }
            return None;
        }

        self.transition(state, ProviderEvent::HandshakeSucceeded);
        let mut active = Some(session);
        self.refresh(driver, &mut active, state, backoff, retry_at)
            .await;
        active
    }

    async fn refresh<D>(
        &self,
        driver: &D,
        session: &mut Option<D::Session>,
        state: &mut ProviderState,
        backoff: &mut Backoff,
        retry_at: &mut Option<Instant>,
    ) where
        D: ProviderDriver,
    {
        let Some(active) = session.as_mut() else {
            return;
        };
        match driver.refresh(active).await {
            Ok(snapshot) => {
                backoff.reset();
                *retry_at = None;
                self.transition(state, ProviderEvent::RefreshSucceeded { snapshot });
            }
            Err(error) => {
                let delay = backoff.next_delay();
                let event = match error.kind {
                    ProviderFailureKind::Authentication => ProviderEvent::AuthenticationRequired,
                    ProviderFailureKind::Unsupported { missing } => {
                        ProviderEvent::CapabilitiesMissing {
                            missing,
                            version: error.version,
                        }
                    }
                    ProviderFailureKind::ProcessExited => ProviderEvent::ProcessExited {
                        reason: error.message,
                        retry_at: Some(unix_after(delay)),
                    },
                    _ => ProviderEvent::RefreshFailed {
                        error: error.message,
                        retry_at: Some(unix_after(delay)),
                    },
                };
                self.transition(state, event);
                let terminal = matches!(
                    state,
                    ProviderState::NeedsAuthentication | ProviderState::Unsupported { .. }
                );
                if let Some(failed) = session.take() {
                    let _ = failed.shutdown().await;
                }
                if !terminal {
                    *retry_at = Some(Instant::now() + delay);
                }
            }
        }
    }

    fn transition(&self, state: &mut ProviderState, event: ProviderEvent) {
        if let Ok(next) = state.clone().transition(event) {
            *state = next.clone();
            self.state_tx.send_replace(next);
        }
    }
}

enum ManagerAction {
    Command(ProviderCommand),
    Refresh,
    Retry,
    Signal(SessionSignal),
}

fn unix_after(duration: Duration) -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .saturating_add(duration)
        .as_secs()
        .min(i64::MAX as u64) as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{ProviderId, QuotaWindow};
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct FakeCounts {
        connections: usize,
        refreshes: usize,
        shutdowns: usize,
        fail_first_refresh: bool,
    }

    #[derive(Clone)]
    struct FakeDriver {
        counts: Arc<Mutex<FakeCounts>>,
    }

    struct FakeSession {
        counts: Arc<Mutex<FakeCounts>>,
    }

    #[async_trait]
    impl ProviderSession for FakeSession {
        fn process_id(&self) -> Option<u32> {
            Some(42)
        }

        async fn next_signal(&mut self) -> SessionSignal {
            std::future::pending().await
        }

        async fn shutdown(self) -> Result<(), ProviderFailure> {
            self.counts.lock().unwrap().shutdowns += 1;
            Ok(())
        }
    }

    #[async_trait]
    impl ProviderDriver for FakeDriver {
        type Installation = String;
        type Session = FakeSession;

        async fn discover(&self) -> Result<Self::Installation, ProviderFailure> {
            Ok("fake.exe".into())
        }

        fn executable_label(&self, installation: &Self::Installation) -> String {
            installation.clone()
        }

        fn version(&self, _installation: &Self::Installation) -> Option<String> {
            Some("diagnostic".into())
        }

        async fn spawn(
            &self,
            _installation: &Self::Installation,
        ) -> Result<Self::Session, ProviderFailure> {
            self.counts.lock().unwrap().connections += 1;
            Ok(FakeSession {
                counts: Arc::clone(&self.counts),
            })
        }

        async fn initialize(&self, _session: &mut Self::Session) -> Result<(), ProviderFailure> {
            Ok(())
        }

        async fn refresh(
            &self,
            _session: &mut Self::Session,
        ) -> Result<QuotaSnapshot, ProviderFailure> {
            let mut counts = self.counts.lock().unwrap();
            counts.refreshes += 1;
            if counts.fail_first_refresh && counts.refreshes == 1 {
                return Err(ProviderFailure::transient("simulated failure"));
            }
            Ok(QuotaSnapshot {
                provider_id: ProviderId::Mock,
                fetched_at: counts.refreshes as i64,
                windows: Vec::<QuotaWindow>::new(),
            })
        }
    }

    async fn wait_until_ready(state: &mut watch::Receiver<ProviderState>) {
        time::timeout(Duration::from_secs(2), async {
            loop {
                if matches!(*state.borrow(), ProviderState::Ready { .. }) {
                    return;
                }
                state.changed().await.unwrap();
            }
        })
        .await
        .unwrap();
    }

    #[test]
    fn backoff_is_jittered_and_capped() {
        let mut backoff = Backoff::new(BackoffConfig {
            initial: Duration::from_secs(1),
            maximum: Duration::from_secs(5),
            jitter_ratio: 0.2,
        });
        for seed in [0, 5_000, 10_000, 7_500, 2_500] {
            assert!(backoff.next_delay_with_seed(seed) <= Duration::from_secs(5));
        }
    }

    #[tokio::test]
    async fn recovers_after_a_simulated_transient_failure() {
        let counts = Arc::new(Mutex::new(FakeCounts {
            fail_first_refresh: true,
            ..Default::default()
        }));
        let driver = FakeDriver {
            counts: Arc::clone(&counts),
        };
        let (handle, runtime, publication) = channel();
        let mut state = publication.state_rx;
        let task = tokio::spawn(runtime.run(
            driver,
            ProviderManagerConfig {
                polling_interval: Duration::from_secs(60),
                backoff: BackoffConfig {
                    initial: Duration::from_millis(10),
                    maximum: Duration::from_millis(20),
                    jitter_ratio: 0.0,
                },
            },
        ));
        handle.send(ProviderCommand::Start).await.unwrap();
        wait_until_ready(&mut state).await;
        assert!(counts.lock().unwrap().connections >= 2);
        handle.send(ProviderCommand::Shutdown).await.unwrap();
        task.await.unwrap();
    }

    #[tokio::test]
    async fn polls_and_accepts_manual_refresh_commands() {
        let counts = Arc::new(Mutex::new(FakeCounts::default()));
        let driver = FakeDriver {
            counts: Arc::clone(&counts),
        };
        let (handle, runtime, publication) = channel();
        let mut state = publication.state_rx;
        let task = tokio::spawn(runtime.run(
            driver,
            ProviderManagerConfig {
                polling_interval: Duration::from_millis(25),
                backoff: BackoffConfig::default(),
            },
        ));
        handle.send(ProviderCommand::Start).await.unwrap();
        wait_until_ready(&mut state).await;
        handle.send(ProviderCommand::Refresh).await.unwrap();
        time::timeout(Duration::from_secs(2), async {
            loop {
                if counts.lock().unwrap().refreshes >= 3 {
                    break;
                }
                time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        handle.send(ProviderCommand::Shutdown).await.unwrap();
        task.await.unwrap();
        assert!(counts.lock().unwrap().shutdowns >= 1);
    }
}
