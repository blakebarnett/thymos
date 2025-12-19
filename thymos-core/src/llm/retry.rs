//! Retry Logic for LLM Providers
//!
//! Implements exponential backoff with jitter for transient failures.

use std::time::Duration;

/// Retry configuration
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_attempts: usize,
    /// Initial delay before first retry
    pub initial_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Multiplier for exponential backoff
    pub backoff_multiplier: f64,
    /// Add jitter to prevent thundering herd
    pub add_jitter: bool,
    /// Retryable error codes (HTTP status codes)
    pub retryable_status_codes: Vec<u16>,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            add_jitter: true,
            retryable_status_codes: vec![
                429, // Too Many Requests
                500, // Internal Server Error
                502, // Bad Gateway
                503, // Service Unavailable
                504, // Gateway Timeout
            ],
        }
    }
}

impl RetryConfig {
    /// Create a config with no retries
    pub fn no_retry() -> Self {
        Self {
            max_attempts: 1,
            ..Default::default()
        }
    }

    /// Create a config for aggressive retries
    pub fn aggressive() -> Self {
        Self {
            max_attempts: 5,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 2.0,
            add_jitter: true,
            retryable_status_codes: vec![429, 500, 502, 503, 504],
        }
    }

    /// Builder: set max attempts
    pub fn with_max_attempts(mut self, attempts: usize) -> Self {
        self.max_attempts = attempts.max(1);
        self
    }

    /// Builder: set initial delay
    pub fn with_initial_delay(mut self, delay: Duration) -> Self {
        self.initial_delay = delay;
        self
    }

    /// Builder: set max delay
    pub fn with_max_delay(mut self, delay: Duration) -> Self {
        self.max_delay = delay;
        self
    }

    /// Builder: set backoff multiplier
    pub fn with_backoff_multiplier(mut self, multiplier: f64) -> Self {
        self.backoff_multiplier = multiplier.max(1.0);
        self
    }

    /// Builder: enable/disable jitter
    pub fn with_jitter(mut self, add_jitter: bool) -> Self {
        self.add_jitter = add_jitter;
        self
    }

    /// Calculate delay for a given attempt (0-indexed)
    pub fn delay_for_attempt(&self, attempt: usize) -> Duration {
        let base_delay = self.initial_delay.as_millis() as f64
            * self.backoff_multiplier.powi(attempt as i32);

        let clamped_delay = base_delay.min(self.max_delay.as_millis() as f64);

        let final_delay = if self.add_jitter {
            // Add up to 25% jitter
            let jitter = clamped_delay * 0.25 * rand_jitter();
            clamped_delay + jitter
        } else {
            clamped_delay
        };

        Duration::from_millis(final_delay as u64)
    }

    /// Check if a status code is retryable
    pub fn is_retryable_status(&self, status: u16) -> bool {
        self.retryable_status_codes.contains(&status)
    }
}

/// Simple pseudo-random jitter (0.0 to 1.0)
/// Uses a simple LCG for determinism in tests
fn rand_jitter() -> f64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEED: AtomicU64 = AtomicU64::new(0);

    // LCG parameters
    const A: u64 = 1103515245;
    const C: u64 = 12345;
    const M: u64 = 1 << 31;

    let seed = SEED.fetch_add(1, Ordering::Relaxed);
    let time_component = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);

    let combined = seed.wrapping_add(time_component);
    let next = (A.wrapping_mul(combined).wrapping_add(C)) % M;

    (next as f64) / (M as f64)
}

/// Retry state tracker
#[derive(Debug)]
pub struct RetryState {
    config: RetryConfig,
    attempt: usize,
    last_error: Option<String>,
}

impl RetryState {
    /// Create a new retry state
    pub fn new(config: RetryConfig) -> Self {
        Self {
            config,
            attempt: 0,
            last_error: None,
        }
    }

    /// Check if we should retry
    pub fn should_retry(&self) -> bool {
        self.attempt < self.config.max_attempts
    }

    /// Record an attempt
    pub fn record_attempt(&mut self, error: impl Into<String>) {
        self.attempt += 1;
        self.last_error = Some(error.into());
    }

    /// Get the delay before next retry
    pub fn next_delay(&self) -> Duration {
        self.config.delay_for_attempt(self.attempt)
    }

    /// Get current attempt number (1-indexed)
    pub fn current_attempt(&self) -> usize {
        self.attempt
    }

    /// Get remaining attempts
    pub fn remaining_attempts(&self) -> usize {
        self.config.max_attempts.saturating_sub(self.attempt)
    }

    /// Get the last error
    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }
}

/// Execute an async operation with retries
pub async fn with_retry<F, Fut, T, E>(
    config: &RetryConfig,
    operation: F,
) -> Result<T, E>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let mut state = RetryState::new(config.clone());

    loop {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                state.record_attempt(e.to_string());

                if !state.should_retry() {
                    return Err(e);
                }

                let delay = state.next_delay();
                tokio::time::sleep(delay).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_attempts, 3);
        assert!(config.add_jitter);
    }

    #[test]
    fn test_retry_config_no_retry() {
        let config = RetryConfig::no_retry();
        assert_eq!(config.max_attempts, 1);
    }

    #[test]
    fn test_delay_calculation() {
        let config = RetryConfig::default().with_jitter(false);

        let delay0 = config.delay_for_attempt(0);
        let delay1 = config.delay_for_attempt(1);
        let delay2 = config.delay_for_attempt(2);

        // Exponential backoff: 500ms, 1000ms, 2000ms
        assert_eq!(delay0.as_millis(), 500);
        assert_eq!(delay1.as_millis(), 1000);
        assert_eq!(delay2.as_millis(), 2000);
    }

    #[test]
    fn test_delay_capped_at_max() {
        let config = RetryConfig::default()
            .with_jitter(false)
            .with_max_delay(Duration::from_secs(1));

        let delay10 = config.delay_for_attempt(10);
        assert_eq!(delay10, Duration::from_secs(1));
    }

    #[test]
    fn test_retryable_status() {
        let config = RetryConfig::default();
        assert!(config.is_retryable_status(429));
        assert!(config.is_retryable_status(503));
        assert!(!config.is_retryable_status(400));
        assert!(!config.is_retryable_status(401));
    }

    #[test]
    fn test_retry_state() {
        let config = RetryConfig::default().with_max_attempts(3);
        let mut state = RetryState::new(config);

        assert!(state.should_retry());
        assert_eq!(state.remaining_attempts(), 3);

        state.record_attempt("error 1");
        assert!(state.should_retry());
        assert_eq!(state.remaining_attempts(), 2);

        state.record_attempt("error 2");
        assert!(state.should_retry());
        assert_eq!(state.remaining_attempts(), 1);

        state.record_attempt("error 3");
        assert!(!state.should_retry());
        assert_eq!(state.remaining_attempts(), 0);
        assert_eq!(state.last_error(), Some("error 3"));
    }

    #[tokio::test]
    async fn test_with_retry_success() {
        let config = RetryConfig::default();
        let result = with_retry(&config, || async { Ok::<_, &str>("success") }).await;
        assert_eq!(result.unwrap(), "success");
    }

    #[tokio::test]
    async fn test_with_retry_eventual_success() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let attempts = AtomicUsize::new(0);
        let config = RetryConfig::default()
            .with_max_attempts(3)
            .with_initial_delay(Duration::from_millis(10));

        let result = with_retry(&config, || {
            let attempt = attempts.fetch_add(1, Ordering::SeqCst);
            async move {
                if attempt < 2 {
                    Err("transient error")
                } else {
                    Ok("success")
                }
            }
        })
        .await;

        assert_eq!(result.unwrap(), "success");
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_with_retry_exhausted() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let attempts = AtomicUsize::new(0);
        let config = RetryConfig::default()
            .with_max_attempts(2)
            .with_initial_delay(Duration::from_millis(10));

        let result: Result<(), &str> = with_retry(&config, || {
            attempts.fetch_add(1, Ordering::SeqCst);
            async { Err("persistent error") }
        })
        .await;

        assert!(result.is_err());
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }
}
