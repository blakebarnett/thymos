//! Circuit Breaker Pattern
//!
//! Prevents cascading failures by tracking error rates and temporarily
//! rejecting requests when a threshold is exceeded.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::RwLock;
use std::time::{Duration, Instant};

/// Circuit breaker state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation - requests pass through
    Closed,
    /// Circuit open - requests are rejected
    Open,
    /// Testing if service recovered
    HalfOpen,
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of failures before opening circuit
    pub failure_threshold: usize,
    /// Number of successes in half-open state to close
    pub success_threshold: usize,
    /// Time to wait before attempting recovery
    pub reset_timeout: Duration,
    /// Time window for counting failures
    pub failure_window: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 2,
            reset_timeout: Duration::from_secs(30),
            failure_window: Duration::from_secs(60),
        }
    }
}

impl CircuitBreakerConfig {
    /// Create a sensitive config (opens quickly)
    pub fn sensitive() -> Self {
        Self {
            failure_threshold: 3,
            success_threshold: 1,
            reset_timeout: Duration::from_secs(15),
            failure_window: Duration::from_secs(30),
        }
    }

    /// Create a tolerant config (takes more failures to open)
    pub fn tolerant() -> Self {
        Self {
            failure_threshold: 10,
            success_threshold: 3,
            reset_timeout: Duration::from_secs(60),
            failure_window: Duration::from_secs(120),
        }
    }

    /// Builder: set failure threshold
    pub fn with_failure_threshold(mut self, threshold: usize) -> Self {
        self.failure_threshold = threshold.max(1);
        self
    }

    /// Builder: set success threshold
    pub fn with_success_threshold(mut self, threshold: usize) -> Self {
        self.success_threshold = threshold.max(1);
        self
    }

    /// Builder: set reset timeout
    pub fn with_reset_timeout(mut self, timeout: Duration) -> Self {
        self.reset_timeout = timeout;
        self
    }
}

/// Circuit breaker for external service calls
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    state: RwLock<CircuitState>,
    failure_count: AtomicUsize,
    success_count: AtomicUsize,
    last_failure_time: AtomicU64,
    opened_at: AtomicU64,
}

impl CircuitBreaker {
    /// Create a new circuit breaker
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: RwLock::new(CircuitState::Closed),
            failure_count: AtomicUsize::new(0),
            success_count: AtomicUsize::new(0),
            last_failure_time: AtomicU64::new(0),
            opened_at: AtomicU64::new(0),
        }
    }

    /// Create with default config
    pub fn default_config() -> Self {
        Self::new(CircuitBreakerConfig::default())
    }

    /// Get current state
    pub fn state(&self) -> CircuitState {
        self.check_state_transition();
        *self.state.read().unwrap()
    }

    /// Check if request is allowed
    pub fn is_allowed(&self) -> bool {
        self.check_state_transition();

        let state = *self.state.read().unwrap();
        match state {
            CircuitState::Closed => true,
            CircuitState::Open => false,
            CircuitState::HalfOpen => {
                // Allow one request through in half-open state
                true
            }
        }
    }

    /// Record a successful call
    pub fn record_success(&self) {
        let state = *self.state.read().unwrap();

        match state {
            CircuitState::Closed => {
                // Reset failure count on success
                self.failure_count.store(0, Ordering::SeqCst);
            }
            CircuitState::HalfOpen => {
                let successes = self.success_count.fetch_add(1, Ordering::SeqCst) + 1;
                if successes >= self.config.success_threshold {
                    self.close();
                }
            }
            CircuitState::Open => {
                // Shouldn't happen, but ignore
            }
        }
    }

    /// Record a failed call
    pub fn record_failure(&self) {
        let now = Instant::now();
        let now_millis = now.elapsed().as_millis() as u64;

        let state = *self.state.read().unwrap();

        match state {
            CircuitState::Closed => {
                // Check if we should reset the failure window
                let last_failure = self.last_failure_time.load(Ordering::SeqCst);
                let window_millis = self.config.failure_window.as_millis() as u64;

                if now_millis.saturating_sub(last_failure) > window_millis {
                    // Reset failure count - window expired
                    self.failure_count.store(1, Ordering::SeqCst);
                } else {
                    let failures = self.failure_count.fetch_add(1, Ordering::SeqCst) + 1;
                    if failures >= self.config.failure_threshold {
                        self.open();
                    }
                }

                self.last_failure_time.store(now_millis, Ordering::SeqCst);
            }
            CircuitState::HalfOpen => {
                // Any failure in half-open state opens the circuit
                self.open();
            }
            CircuitState::Open => {
                // Already open, nothing to do
            }
        }
    }

    /// Open the circuit
    fn open(&self) {
        let mut state = self.state.write().unwrap();
        *state = CircuitState::Open;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        self.opened_at.store(now, Ordering::SeqCst);
        self.success_count.store(0, Ordering::SeqCst);
    }

    /// Close the circuit
    fn close(&self) {
        let mut state = self.state.write().unwrap();
        *state = CircuitState::Closed;
        self.failure_count.store(0, Ordering::SeqCst);
        self.success_count.store(0, Ordering::SeqCst);
    }

    /// Check if we should transition state
    fn check_state_transition(&self) {
        let state = *self.state.read().unwrap();

        if state == CircuitState::Open {
            let opened_at = self.opened_at.load(Ordering::SeqCst);
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);

            let reset_millis = self.config.reset_timeout.as_millis() as u64;

            if now.saturating_sub(opened_at) >= reset_millis {
                // Transition to half-open
                let mut state = self.state.write().unwrap();
                *state = CircuitState::HalfOpen;
                self.success_count.store(0, Ordering::SeqCst);
            }
        }
    }

    /// Get failure count
    pub fn failure_count(&self) -> usize {
        self.failure_count.load(Ordering::SeqCst)
    }

    /// Manually reset the circuit breaker
    pub fn reset(&self) {
        self.close();
    }
}

impl std::fmt::Debug for CircuitBreaker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CircuitBreaker")
            .field("state", &self.state())
            .field("failure_count", &self.failure_count())
            .field("config", &self.config)
            .finish()
    }
}

/// Execute an async operation with circuit breaker protection
pub async fn with_circuit_breaker<F, Fut, T, E>(
    circuit_breaker: &CircuitBreaker,
    operation: F,
) -> Result<T, CircuitBreakerError<E>>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
{
    if !circuit_breaker.is_allowed() {
        return Err(CircuitBreakerError::CircuitOpen);
    }

    match operation().await {
        Ok(result) => {
            circuit_breaker.record_success();
            Ok(result)
        }
        Err(e) => {
            circuit_breaker.record_failure();
            Err(CircuitBreakerError::OperationFailed(e))
        }
    }
}

/// Error type for circuit breaker operations
#[derive(Debug)]
pub enum CircuitBreakerError<E> {
    /// Circuit is open, request rejected
    CircuitOpen,
    /// Operation failed
    OperationFailed(E),
}

impl<E: std::fmt::Display> std::fmt::Display for CircuitBreakerError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CircuitBreakerError::CircuitOpen => write!(f, "Circuit breaker is open"),
            CircuitBreakerError::OperationFailed(e) => write!(f, "Operation failed: {}", e),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for CircuitBreakerError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CircuitBreakerError::CircuitOpen => None,
            CircuitBreakerError::OperationFailed(e) => Some(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker_initial_state() {
        let cb = CircuitBreaker::default_config();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.is_allowed());
    }

    #[test]
    fn test_circuit_breaker_opens_after_failures() {
        let config = CircuitBreakerConfig::default().with_failure_threshold(3);
        let cb = CircuitBreaker::new(config);

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.is_allowed());
    }

    #[test]
    fn test_circuit_breaker_success_resets_count() {
        let config = CircuitBreakerConfig::default().with_failure_threshold(3);
        let cb = CircuitBreaker::new(config);

        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.failure_count(), 2);

        cb.record_success();
        assert_eq!(cb.failure_count(), 0);
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_circuit_breaker_reset() {
        let config = CircuitBreakerConfig::default().with_failure_threshold(2);
        let cb = CircuitBreaker::new(config);

        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        cb.reset();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.is_allowed());
    }

    #[tokio::test]
    async fn test_with_circuit_breaker_success() {
        let cb = CircuitBreaker::default_config();

        let result = with_circuit_breaker(&cb, || async { Ok::<_, &str>("success") }).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
    }

    #[tokio::test]
    async fn test_with_circuit_breaker_open() {
        let config = CircuitBreakerConfig::default().with_failure_threshold(1);
        let cb = CircuitBreaker::new(config);

        // Trigger the circuit to open
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        let result: Result<(), CircuitBreakerError<&str>> =
            with_circuit_breaker(&cb, || async { Ok(()) }).await;

        assert!(matches!(result, Err(CircuitBreakerError::CircuitOpen)));
    }

    #[test]
    fn test_config_builders() {
        let config = CircuitBreakerConfig::default()
            .with_failure_threshold(10)
            .with_success_threshold(5)
            .with_reset_timeout(Duration::from_secs(120));

        assert_eq!(config.failure_threshold, 10);
        assert_eq!(config.success_threshold, 5);
        assert_eq!(config.reset_timeout, Duration::from_secs(120));
    }

    #[test]
    fn test_sensitive_config() {
        let config = CircuitBreakerConfig::sensitive();
        assert_eq!(config.failure_threshold, 3);
        assert_eq!(config.success_threshold, 1);
    }

    #[test]
    fn test_tolerant_config() {
        let config = CircuitBreakerConfig::tolerant();
        assert_eq!(config.failure_threshold, 10);
        assert_eq!(config.success_threshold, 3);
    }
}
