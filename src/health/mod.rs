/// Health checking for MongoDB and Redis backends
pub mod mongodb;
pub mod redis;

use crate::core::{Backend, BackendMetadata};
use std::time::{Duration, SystemTime};
use tokio::time::timeout;
use async_trait::async_trait;
use std::fmt;

/// Health status of a backend
#[derive(Debug, Clone, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Unhealthy { reason: String },
    Timeout,
    Unknown,
}

impl fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HealthStatus::Healthy => write!(f, "Healthy"),
            HealthStatus::Unhealthy { reason } => write!(f, "Unhealthy: {}", reason),
            HealthStatus::Timeout => write!(f, "Timeout"),
            HealthStatus::Unknown => write!(f, "Unknown"),
        }
    }
}

impl HealthStatus {
    /// Check if the status represents a healthy backend
    pub fn is_healthy(&self) -> bool {
        matches!(self, HealthStatus::Healthy)
    }
}

/// Health checker trait
#[async_trait::async_trait]
pub trait HealthChecker: Send + Sync {
    /// Perform health check on a backend
    async fn check_health(&self, backend: &Backend) -> HealthStatus;

    /// Get the check interval for this health checker
    fn check_interval(&self) -> Duration;

    /// Get the timeout for health checks
    fn check_timeout(&self) -> Duration;
}

/// Generic health check manager
pub struct HealthCheckManager {
    checker: Box<dyn HealthChecker>,
}

impl HealthCheckManager {
    pub fn new(checker: Box<dyn HealthChecker>) -> Self {
        Self { checker }
    }

    /// Perform health check with timeout
    pub async fn check_backend_health(&self, backend: &mut Backend) -> HealthStatus {
        let check_timeout = self.checker.check_timeout();

        let status = match timeout(check_timeout, self.checker.check_health(backend)).await {
            Ok(status) => status,
            Err(_) => HealthStatus::Timeout,
        };

        // Update backend status
        backend.last_health_check = Some(SystemTime::now());
        backend.healthy = matches!(status, HealthStatus::Healthy);

        status
    }

    /// Run continuous health checking for a backend
    pub async fn run_health_checks(&self, backend: &mut Backend) {
        let mut interval = tokio::time::interval(self.checker.check_interval());

        loop {
            interval.tick().await;

            let status = self.check_backend_health(backend).await;

            match status {
                HealthStatus::Healthy => {
                    tracing::debug!("Backend {} is healthy", backend.id);
                }
                HealthStatus::Unhealthy { reason } => {
                    tracing::warn!("Backend {} is unhealthy: {}", backend.id, reason);
                }
                HealthStatus::Timeout => {
                    tracing::warn!("Health check timeout for backend {}", backend.id);
                }
                HealthStatus::Unknown => {
                    tracing::warn!("Unknown health status for backend {}", backend.id);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Backend, BackendMetadata};
    use crate::health::mongodb::MongoDBHealthChecker;
    use std::net::SocketAddr;
    use std::time::{Duration, SystemTime};
    use async_trait::async_trait;

    // Mock health checker for testing
    struct MockHealthChecker {
        should_pass: bool,
    }

    #[async_trait]
    impl HealthChecker for MockHealthChecker {
        async fn check_health(&self, _backend: &Backend) -> HealthStatus {
            if self.should_pass {
                HealthStatus::Healthy
            } else {
                HealthStatus::Unhealthy {
                    reason: "Mock failure".to_string(),
                }
            }
        }

        fn check_timeout(&self) -> Duration {
            Duration::from_secs(1)
        }

        fn check_interval(&self) -> Duration {
            Duration::from_secs(5)
        }
    }

    fn create_test_backend(id: &str, healthy: bool) -> Backend {
        Backend {
            id: id.to_string(),
            addr: "127.0.0.1:8080".parse::<SocketAddr>().unwrap(),
            weight: 1,
            healthy,
            last_health_check: Some(SystemTime::now()),
            metadata: BackendMetadata::MongoDB {
                version: Some("4.4.0".to_string()),
                is_primary: true,
                connection_count: 0,
            },
        }
    }

    #[test]
    fn test_health_status_display() {
        assert_eq!(HealthStatus::Healthy.to_string(), "Healthy");
        assert_eq!(HealthStatus::Unhealthy { reason: "test".to_string() }.to_string(), "Unhealthy: test");
        assert_eq!(HealthStatus::Timeout.to_string(), "Timeout");
        assert_eq!(HealthStatus::Unknown.to_string(), "Unknown");
    }

    #[test]
    fn test_health_status_is_healthy() {
        assert!(HealthStatus::Healthy.is_healthy());
        assert!(!HealthStatus::Unhealthy { reason: "test".to_string() }.is_healthy());
        assert!(!HealthStatus::Timeout.is_healthy());
        assert!(!HealthStatus::Unknown.is_healthy());
    }

    #[tokio::test]
    async fn test_health_check_manager_creation() {
        let checker = Box::new(MockHealthChecker { should_pass: true });
        let manager = HealthCheckManager::new(checker);
        // Just verify the manager was created successfully
        assert!(true);
    }

    #[tokio::test]
    async fn test_health_check_success() {
        let checker = Box::new(MockHealthChecker { should_pass: true });
        let manager = HealthCheckManager::new(checker);
        let mut backend = create_test_backend("test1", false);
        
        let status = manager.check_backend_health(&mut backend).await;
        assert!(status.is_healthy());
        assert!(backend.healthy); // Backend should be updated
    }

    #[tokio::test]
    async fn test_health_check_failure() {
        let checker = Box::new(MockHealthChecker { should_pass: false });
        let manager = HealthCheckManager::new(checker);
        let mut backend = create_test_backend("test1", true);
        
        let status = manager.check_backend_health(&mut backend).await;
        assert!(!status.is_healthy());
        assert!(!backend.healthy); // Backend should be updated
    }

    #[test]
    fn test_create_health_checker_mongodb() {
        let backend = create_test_backend("test", true);
        let checker = create_health_checker(&backend);
        
        // Verify we get a MongoDB health checker
        assert_eq!(checker.check_timeout(), Duration::from_secs(5));
    }

    #[test]
    fn test_create_health_checker_redis() {
        let mut backend = create_test_backend("test", true);
        backend.metadata = BackendMetadata::Redis {
            node_id: "test-node".to_string(),
            slot_ranges: vec![],
            is_master: true,
            replication_offset: Some(0),
        };
        
        let checker = create_health_checker(&backend);
        
        // Verify we get a Redis health checker
        assert_eq!(checker.check_timeout(), Duration::from_secs(3));
    }
}

/// Utility function to create appropriate health checker based on backend type
pub fn create_health_checker(backend: &Backend) -> Box<dyn HealthChecker> {
    match &backend.metadata {
        BackendMetadata::MongoDB { .. } => Box::new(mongodb::MongoDBHealthChecker::new()),
        BackendMetadata::Redis { .. } => Box::new(redis::RedisHealthChecker::new()),
    }
}
