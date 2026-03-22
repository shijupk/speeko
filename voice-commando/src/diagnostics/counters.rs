use bevy::prelude::*;
use std::collections::VecDeque;
use std::time::Duration;

#[derive(Resource)]
pub struct LatencyTracker {
    samples: VecDeque<Duration>,
    capacity: usize,
}

impl LatencyTracker {
    pub fn new(capacity: usize) -> Self {
        Self {
            samples: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn record(&mut self, latency: Duration) {
        if self.samples.len() >= self.capacity {
            self.samples.pop_front();
        }
        self.samples.push_back(latency);
    }

    pub fn average(&self) -> Duration {
        if self.samples.is_empty() {
            return Duration::ZERO;
        }
        let total: Duration = self.samples.iter().sum();
        total / self.samples.len() as u32
    }

    pub fn p95(&self) -> Duration {
        if self.samples.is_empty() {
            return Duration::ZERO;
        }
        let mut sorted: Vec<Duration> = self.samples.iter().cloned().collect();
        sorted.sort();
        let idx = (sorted.len() as f32 * 0.95) as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    pub fn count(&self) -> usize {
        self.samples.len()
    }
}

impl Default for LatencyTracker {
    fn default() -> Self {
        Self::new(100)
    }
}
