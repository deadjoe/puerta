/// Health checking for MongoDB and Redis backends

pub mod mongodb;
pub mod redis;

use crate::core::{Backend, BackendMetadata};
use std::time::{Duration, SystemTime};
use tokio::time::timeout;

/// Health check result
#[derive(Debug, Clone)]
pub enum HealthStatus {
    Healthy,
    Unhealthy { reason: String },
    Timeout,
    Unknown,
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

/// Utility function to create appropriate health checker based on backend type
pub fn create_health_checker(backend: &Backend) -> Box<dyn HealthChecker> {
    match &backend.metadata {
        BackendMetadata::MongoDB { .. } => {
            Box::new(mongodb::MongoDBHealthChecker::new())
        }
        BackendMetadata::Redis { .. } => {
            Box::new(redis::RedisHealthChecker::new())
        }
    }
}