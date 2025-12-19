//! Trace Sampling

use std::sync::atomic::{AtomicU64, Ordering};

/// Sampling decision
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SamplingDecision {
    /// Sample this trace
    Sample,
    /// Don't sample this trace
    Drop,
}

/// Trait for sampling strategies
pub trait SamplingStrategy: Send + Sync {
    /// Decide whether to sample a trace
    fn should_sample(&self, trace_id: &str) -> SamplingDecision;

    /// Get the strategy name
    fn name(&self) -> &'static str;
}

/// Always sample
pub struct AlwaysSample;

impl SamplingStrategy for AlwaysSample {
    fn should_sample(&self, _trace_id: &str) -> SamplingDecision {
        SamplingDecision::Sample
    }

    fn name(&self) -> &'static str {
        "always"
    }
}

/// Never sample
pub struct NeverSample;

impl SamplingStrategy for NeverSample {
    fn should_sample(&self, _trace_id: &str) -> SamplingDecision {
        SamplingDecision::Drop
    }

    fn name(&self) -> &'static str {
        "never"
    }
}

/// Rate-based sampling (sample N% of traces)
pub struct RateSampler {
    /// Rate as a value from 0 to 100
    rate: u32,
    /// Counter for deterministic sampling
    counter: AtomicU64,
}

impl RateSampler {
    /// Create a new rate sampler
    ///
    /// # Arguments
    ///
    /// * `rate` - Percentage of traces to sample (0-100)
    pub fn new(rate: u32) -> Self {
        Self {
            rate: rate.min(100),
            counter: AtomicU64::new(0),
        }
    }

    /// Create a sampler that samples 1 in N traces
    pub fn one_in(n: u64) -> Self {
        let rate = if n > 0 { 100 / n.min(100) } else { 0 };
        Self::new(rate as u32)
    }
}

impl SamplingStrategy for RateSampler {
    fn should_sample(&self, _trace_id: &str) -> SamplingDecision {
        let count = self.counter.fetch_add(1, Ordering::Relaxed);
        if (count % 100) < self.rate as u64 {
            SamplingDecision::Sample
        } else {
            SamplingDecision::Drop
        }
    }

    fn name(&self) -> &'static str {
        "rate"
    }
}

/// Hash-based deterministic sampling
pub struct HashSampler {
    /// Rate as a value from 0 to 100
    rate: u32,
}

impl HashSampler {
    /// Create a new hash sampler
    pub fn new(rate: u32) -> Self {
        Self {
            rate: rate.min(100),
        }
    }
}

impl SamplingStrategy for HashSampler {
    fn should_sample(&self, trace_id: &str) -> SamplingDecision {
        // Simple hash based on trace ID
        let hash: u64 = trace_id
            .bytes()
            .fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));

        if (hash % 100) < self.rate as u64 {
            SamplingDecision::Sample
        } else {
            SamplingDecision::Drop
        }
    }

    fn name(&self) -> &'static str {
        "hash"
    }
}

/// Trace sampler that uses a strategy
pub struct TraceSampler {
    strategy: Box<dyn SamplingStrategy>,
}

impl TraceSampler {
    /// Create a new sampler with a strategy
    pub fn new<S: SamplingStrategy + 'static>(strategy: S) -> Self {
        Self {
            strategy: Box::new(strategy),
        }
    }

    /// Create an always-sample sampler
    pub fn always() -> Self {
        Self::new(AlwaysSample)
    }

    /// Create a never-sample sampler
    pub fn never() -> Self {
        Self::new(NeverSample)
    }

    /// Create a rate-based sampler
    pub fn rate(rate: u32) -> Self {
        Self::new(RateSampler::new(rate))
    }

    /// Create a hash-based sampler
    pub fn hash(rate: u32) -> Self {
        Self::new(HashSampler::new(rate))
    }

    /// Check if a trace should be sampled
    pub fn should_sample(&self, trace_id: &str) -> SamplingDecision {
        self.strategy.should_sample(trace_id)
    }

    /// Get the strategy name
    pub fn strategy_name(&self) -> &'static str {
        self.strategy.name()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_always_sample() {
        let sampler = TraceSampler::always();

        for i in 0..100 {
            assert_eq!(
                sampler.should_sample(&format!("trace-{}", i)),
                SamplingDecision::Sample
            );
        }
    }

    #[test]
    fn test_never_sample() {
        let sampler = TraceSampler::never();

        for i in 0..100 {
            assert_eq!(
                sampler.should_sample(&format!("trace-{}", i)),
                SamplingDecision::Drop
            );
        }
    }

    #[test]
    fn test_rate_sampler() {
        let sampler = TraceSampler::rate(50);

        let mut sampled = 0;
        for i in 0..1000 {
            if sampler.should_sample(&format!("trace-{}", i)) == SamplingDecision::Sample {
                sampled += 1;
            }
        }

        // Should sample approximately 50%
        assert!(sampled >= 400 && sampled <= 600);
    }

    #[test]
    fn test_hash_sampler_deterministic() {
        let sampler = TraceSampler::hash(50);

        // Same trace ID should always get same decision
        let decision1 = sampler.should_sample("test-trace-123");
        let decision2 = sampler.should_sample("test-trace-123");

        assert_eq!(decision1, decision2);
    }

    #[test]
    fn test_one_in() {
        let sampler = RateSampler::one_in(10);

        let mut sampled = 0;
        for i in 0..1000 {
            if sampler.should_sample(&format!("trace-{}", i)) == SamplingDecision::Sample {
                sampled += 1;
            }
        }

        // Should sample approximately 10%
        assert!(sampled >= 50 && sampled <= 150);
    }

    #[test]
    fn test_strategy_name() {
        assert_eq!(TraceSampler::always().strategy_name(), "always");
        assert_eq!(TraceSampler::never().strategy_name(), "never");
        assert_eq!(TraceSampler::rate(50).strategy_name(), "rate");
        assert_eq!(TraceSampler::hash(50).strategy_name(), "hash");
    }
}
