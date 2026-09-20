use crate::domain::{ProviderId, QuotaSnapshot, QuotaWindow, QuotaWindowKind};

pub struct MockProvider;

impl MockProvider {
    pub fn snapshot() -> QuotaSnapshot {
        QuotaSnapshot {
            provider_id: ProviderId::Mock,
            fetched_at: 1_700_000_000,
            windows: vec![
                QuotaWindow {
                    id: "five-hour".into(),
                    kind: QuotaWindowKind::Rolling,
                    label: Some("Five-hour window".into()),
                    duration_minutes: Some(serde_json::json!(300)),
                    used_percent: Some(serde_json::json!(25)),
                    remaining_percent: Some(serde_json::json!(75)),
                    resets_at: Some(serde_json::json!(1_700_010_800)),
                    limit_status: serde_json::json!("available"),
                },
                QuotaWindow {
                    id: "weekly".into(),
                    kind: QuotaWindowKind::Weekly,
                    label: Some("Weekly window".into()),
                    duration_minutes: Some(serde_json::json!(10_080)),
                    used_percent: Some(serde_json::json!(42)),
                    remaining_percent: Some(serde_json::json!(58)),
                    resets_at: Some(serde_json::json!(1_700_604_800)),
                    limit_status: serde_json::json!("available"),
                },
            ],
        }
    }
}
