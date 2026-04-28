use std::time::{Duration, Instant};

use time::{format_description::well_known::Rfc3339, OffsetDateTime};

#[derive(Clone, Debug)]
pub struct Clock {
    started_instant: Instant,
    started_at: String,
}

impl Clock {
    pub fn started_now() -> Self {
        Self {
            started_instant: Instant::now(),
            started_at: now_rfc3339(),
        }
    }

    pub fn started_at(&self) -> &str {
        &self.started_at
    }

    pub fn uptime_seconds(&self) -> u64 {
        self.started_instant.elapsed().as_secs()
    }
}

pub fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .expect("Rfc3339 formatting should not fail")
}

pub fn duration_ms(duration: Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}
