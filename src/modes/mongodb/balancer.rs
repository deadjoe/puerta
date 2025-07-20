/// Load balancing algorithms for MongoDB mongos instances
use crate::core::Backend;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Load balancing algorithm trait
pub trait LoadBalancingAlgorithm: Send + Sync {
    /// Select a backend from the available healthy backends
    fn select_backend(&self, backends: &[Backend]) -> Option<usize>;
}

/// Round-robin load balancing algorithm
pub struct RoundRobin {
    counter: AtomicUsize,
}

impl RoundRobin {
    pub fn new() -> Self {
        Self {
            counter: AtomicUsize::new(0),
        }
    }
}

impl LoadBalancingAlgorithm for RoundRobin {
    fn select_backend(&self, backends: &[Backend]) -> Option<usize> {
        if backends.is_empty() {
            return None;
        }

        let index = self.counter.fetch_add(1, Ordering::Relaxed) % backends.len();
        Some(index)
    }
}

/// Weighted round-robin algorithm
pub struct WeightedRoundRobin {
    counter: AtomicUsize,
}

impl WeightedRoundRobin {
    pub fn new() -> Self {
        Self {
            counter: AtomicUsize::new(0),
        }
    }
}

impl LoadBalancingAlgorithm for WeightedRoundRobin {
    fn select_backend(&self, backends: &[Backend]) -> Option<usize> {
        if backends.is_empty() {
            return None;
        }

        // Calculate total weight
        let total_weight: usize = backends.iter().map(|b| b.weight).sum();
        if total_weight == 0 {
            // Fall back to simple round-robin if no weights
            let index = self.counter.fetch_add(1, Ordering::Relaxed) % backends.len();
            return Some(index);
        }

        // Use weighted selection
        let position = self.counter.fetch_add(1, Ordering::Relaxed) % total_weight;
        let mut current_weight = 0;

        for (index, backend) in backends.iter().enumerate() {
            current_weight += backend.weight;
            if position < current_weight {
                return Some(index);
            }
        }

        // Fallback (shouldn't happen)
        Some(0)
    }
}

/// Least connections algorithm
pub struct LeastConnections;

impl LeastConnections {
    pub fn new() -> Self {
        Self
    }
}

impl LoadBalancingAlgorithm for LeastConnections {
    fn select_backend(&self, backends: &[Backend]) -> Option<usize> {
        if backends.is_empty() {
            return None;
        }

        // For now, just return round-robin since we don't track connections yet
        // TODO: Implement actual connection counting
        Some(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Backend;
    use std::net::SocketAddr;

    fn create_test_backend(id: &str, weight: usize) -> Backend {
        let mut backend = Backend::new_mongodb(
            id.to_string(),
            "127.0.0.1:27017".parse::<SocketAddr>().unwrap(),
        );
        backend.weight = weight;
        backend
    }

    #[test]
    fn test_round_robin() {
        let rr = RoundRobin::new();
        let backends = vec![
            create_test_backend("backend1", 1),
            create_test_backend("backend2", 1),
            create_test_backend("backend3", 1),
        ];

        // Test that it cycles through backends
        assert_eq!(rr.select_backend(&backends), Some(0));
        assert_eq!(rr.select_backend(&backends), Some(1));
        assert_eq!(rr.select_backend(&backends), Some(2));
        assert_eq!(rr.select_backend(&backends), Some(0));
    }

    #[test]
    fn test_weighted_round_robin() {
        let wrr = WeightedRoundRobin::new();
        let backends = vec![
            create_test_backend("backend1", 3),
            create_test_backend("backend2", 1),
        ];

        // Backend1 should be selected 3 times for every 1 time backend2 is selected
        let mut counts = [0, 0];
        for _ in 0..8 {
            if let Some(index) = wrr.select_backend(&backends) {
                counts[index] += 1;
            }
        }

        // Backend1 should have been selected 6 times, backend2 should have been selected 2 times
        assert_eq!(counts[0], 6);
        assert_eq!(counts[1], 2);
    }
}
