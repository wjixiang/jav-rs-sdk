use std::collections::BTreeSet;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub jitter: f64,
    pub retry_statuses: BTreeSet<u16>,
    pub respect_retry_after: bool,
    pub retry_connection_errors: bool,
    pub retry_timeout_errors: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 2,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(5),
            jitter: 0.25,
            retry_statuses: (500..600).chain([408, 429]).collect(),
            respect_retry_after: true,
            retry_connection_errors: true,
            retry_timeout_errors: true,
        }
    }
}

impl RetryPolicy {
    pub fn disabled() -> Self {
        Self {
            max_retries: 0,
            ..Self::default()
        }
    }

    pub fn retry_status(mut self, status: u16) -> Self {
        self.retry_statuses.insert(status);
        self
    }

    pub fn should_retry_status(&self, status: u16) -> bool {
        self.retry_statuses.contains(&status)
    }

    pub(crate) fn delay(&self, attempt: u32, retry_after: Option<Duration>) -> Duration {
        if self.respect_retry_after
            && let Some(delay) = retry_after
        {
            return delay;
        }

        let exponent = attempt.min(16);
        let base = self.initial_delay.saturating_mul(1u32 << exponent);
        let base = base.min(self.max_delay);
        self.apply_jitter(base)
    }

    fn apply_jitter(&self, delay: Duration) -> Duration {
        if self.jitter <= 0.0 {
            return delay;
        }

        let jitter = self.jitter.clamp(0.0, 1.0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |time| time.subsec_nanos());
        let factor = 1.0 - jitter * (f64::from(nanos) / 1_000_000_000.0);
        Duration::from_secs_f64(delay.as_secs_f64() * factor)
    }
}
