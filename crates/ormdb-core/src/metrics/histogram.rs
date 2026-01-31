//! Simple histogram for latency tracking.
//!
//! This module provides a fixed-bucket histogram implementation
//! for tracking query latencies with percentile support.

use std::sync::atomic::{AtomicU64, Ordering};

/// Fixed-bucket histogram for latency measurements.
///
/// Uses pre-defined bucket boundaries optimized for typical query latencies.
/// All operations are lock-free using atomic operations.
pub struct Histogram {
    /// Bucket boundaries in microseconds.
    buckets: Vec<u64>,
    /// Counts per bucket (cumulative - each bucket counts values <= boundary).
    counts: Vec<AtomicU64>,
    /// Sum of all observed values.
    sum: AtomicU64,
    /// Total count of observations.
    count: AtomicU64,
    /// Maximum observed value.
    max: AtomicU64,
}

impl Histogram {
    /// Create a histogram with default latency buckets.
    ///
    /// Buckets: 100us, 500us, 1ms, 5ms, 10ms, 50ms, 100ms, 500ms, 1s, 5s
    pub fn latency() -> Self {
        let buckets = vec![
            100,       // 100 microseconds
            500,       // 500 microseconds
            1_000,     // 1 millisecond
            5_000,     // 5 milliseconds
            10_000,    // 10 milliseconds
            50_000,    // 50 milliseconds
            100_000,   // 100 milliseconds
            500_000,   // 500 milliseconds
            1_000_000, // 1 second
            5_000_000, // 5 seconds
        ];
        let counts: Vec<AtomicU64> = buckets.iter().map(|_| AtomicU64::new(0)).collect();
        Self {
            buckets,
            counts,
            sum: AtomicU64::new(0),
            count: AtomicU64::new(0),
            max: AtomicU64::new(0),
        }
    }

    /// Record a value in microseconds.
    pub fn observe(&self, value_us: u64) {
        self.sum.fetch_add(value_us, Ordering::Relaxed);
        self.count.fetch_add(1, Ordering::Relaxed);

        // Update max using compare-and-swap
        let mut current_max = self.max.load(Ordering::Relaxed);
        while value_us > current_max {
            match self.max.compare_exchange_weak(
                current_max,
                value_us,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => current_max = actual,
            }
        }

        // Increment the appropriate bucket
        for (i, &boundary) in self.buckets.iter().enumerate() {
            if value_us <= boundary {
                self.counts[i].fetch_add(1, Ordering::Relaxed);
                return;
            }
        }
        // Value exceeds all buckets, count in last bucket
        if let Some(last) = self.counts.last() {
            last.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Get the total count of observations.
    pub fn count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }

    /// Get the sum of all observed values.
    pub fn sum(&self) -> u64 {
        self.sum.load(Ordering::Relaxed)
    }

    /// Get the maximum observed value.
    pub fn max(&self) -> u64 {
        self.max.load(Ordering::Relaxed)
    }

    /// Get average value.
    pub fn avg(&self) -> u64 {
        let count = self.count.load(Ordering::Relaxed);
        if count == 0 {
            return 0;
        }
        self.sum.load(Ordering::Relaxed) / count
    }

    /// Get approximate percentile (e.g., 0.50 for P50, 0.99 for P99).
    ///
    /// Returns the upper boundary of the bucket containing the target percentile.
    pub fn percentile(&self, p: f64) -> u64 {
        let total = self.count.load(Ordering::Relaxed);
        if total == 0 {
            return 0;
        }

        let target = (total as f64 * p).ceil() as u64;
        let mut cumulative = 0u64;

        for (i, count) in self.counts.iter().enumerate() {
            cumulative += count.load(Ordering::Relaxed);
            if cumulative >= target {
                return self.buckets[i];
            }
        }

        // All values exceed the largest bucket
        *self.buckets.last().unwrap_or(&0)
    }

    /// Get P50 (median) latency.
    pub fn p50(&self) -> u64 {
        self.percentile(0.50)
    }

    /// Get P99 latency.
    pub fn p99(&self) -> u64 {
        self.percentile(0.99)
    }

    /// Reset all counters to zero.
    pub fn reset(&self) {
        self.sum.store(0, Ordering::Relaxed);
        self.count.store(0, Ordering::Relaxed);
        self.max.store(0, Ordering::Relaxed);
        for count in &self.counts {
            count.store(0, Ordering::Relaxed);
        }
    }

    /// Get a snapshot of all bucket counts.
    pub fn snapshot(&self) -> Vec<(u64, u64)> {
        self.buckets
            .iter()
            .zip(self.counts.iter())
            .map(|(&boundary, count)| (boundary, count.load(Ordering::Relaxed)))
            .collect()
    }
}

impl Default for Histogram {
    fn default() -> Self {
        Self::latency()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_histogram_observe() {
        let hist = Histogram::latency();
        hist.observe(50);    // 50us - bucket 0 (<=100)
        hist.observe(200);   // 200us - bucket 1 (<=500)
        hist.observe(1500);  // 1.5ms - bucket 3 (<=5000)

        assert_eq!(hist.count(), 3);
        assert_eq!(hist.sum(), 50 + 200 + 1500);
    }

    #[test]
    fn test_histogram_avg() {
        let hist = Histogram::latency();
        hist.observe(100);
        hist.observe(200);
        hist.observe(300);

        assert_eq!(hist.avg(), 200);
    }

    #[test]
    fn test_histogram_max() {
        let hist = Histogram::latency();
        hist.observe(100);
        hist.observe(5000);
        hist.observe(1000);

        assert_eq!(hist.max(), 5000);
    }

    #[test]
    fn test_histogram_percentile() {
        let hist = Histogram::latency();
        // Add 100 values in bucket 0 (<=100us)
        for _ in 0..100 {
            hist.observe(50);
        }
        // P50 and P99 should both be 100 (first bucket boundary)
        assert_eq!(hist.p50(), 100);
        assert_eq!(hist.p99(), 100);

        // Add 100 values in bucket 2 (<=1000us)
        for _ in 0..100 {
            hist.observe(800);
        }
        // Now P50 should be 1000 (middle-ish) and P99 should be 1000
        assert_eq!(hist.p50(), 100);  // First 100 samples are in bucket 0
        assert_eq!(hist.p99(), 1000); // 99th percentile is in bucket 2
    }

    #[test]
    fn test_histogram_empty() {
        let hist = Histogram::latency();
        assert_eq!(hist.count(), 0);
        assert_eq!(hist.avg(), 0);
        assert_eq!(hist.p50(), 0);
        assert_eq!(hist.p99(), 0);
        assert_eq!(hist.max(), 0);
    }

    #[test]
    fn test_histogram_reset() {
        let hist = Histogram::latency();
        hist.observe(1000);
        hist.observe(2000);

        assert_eq!(hist.count(), 2);

        hist.reset();

        assert_eq!(hist.count(), 0);
        assert_eq!(hist.sum(), 0);
        assert_eq!(hist.max(), 0);
    }

    #[test]
    fn test_histogram_snapshot() {
        let hist = Histogram::latency();
        hist.observe(50);  // bucket 0
        hist.observe(200); // bucket 1

        let snapshot = hist.snapshot();
        assert_eq!(snapshot[0], (100, 1));  // 1 value <= 100
        assert_eq!(snapshot[1], (500, 1));  // 1 value <= 500
    }
}
