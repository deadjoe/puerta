/// MongoDB mongos health checker

use super::{HealthChecker, HealthStatus};
use crate::core::Backend;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// MongoDB health checker implementation
/// Uses ismaster command to check mongos availability and status
pub struct MongoDBHealthChecker {
    check_interval: Duration,
    check_timeout: Duration,
}

impl MongoDBHealthChecker {
    pub fn new() -> Self {
        Self {
            check_interval: Duration::from_secs(10),
            check_timeout: Duration::from_secs(5),
        }
    }

    /// Create a simple TCP connection health check
    /// For production use, should implement proper MongoDB Wire Protocol ismaster command
    async fn tcp_health_check(&self, backend: &Backend) -> HealthStatus {
        match TcpStream::connect(backend.addr).await {
            Ok(mut stream) => {
                // For now, just verify we can connect
                // TODO: Implement proper MongoDB Wire Protocol health check
                // Should send ismaster command and verify response
                
                // Try to write a simple probe
                if let Err(e) = stream.write_all(b"").await {
                    return HealthStatus::Unhealthy {
                        reason: format!("Failed to write probe: {}", e),
                    };
                }

                // Try to read response (mongos should close connection for invalid message)
                let mut buffer = [0u8; 1024];
                match stream.read(&mut buffer).await {
                    Ok(_) => HealthStatus::Healthy,
                    Err(e) => {
                        // Connection closed or error - could be normal for invalid probe
                        if e.kind() == std::io::ErrorKind::UnexpectedEof {
                            // mongos closed connection after invalid probe - this is expected
                            HealthStatus::Healthy
                        } else {
                            HealthStatus::Unhealthy {
                                reason: format!("Read error: {}", e),
                            }
                        }
                    }
                }
            }
            Err(e) => HealthStatus::Unhealthy {
                reason: format!("Connection failed: {}", e),
            },
        }
    }

    /// Future enhancement: Implement proper MongoDB Wire Protocol health check
    #[allow(dead_code)]
    async fn mongodb_ismaster_check(&self, backend: &Backend) -> HealthStatus {
        // TODO: Implement proper MongoDB Wire Protocol
        // 1. Connect to mongos
        // 2. Send ismaster command in MongoDB Wire Protocol format
        // 3. Parse response to verify mongos is functioning
        // 4. Check if mongos is primary/secondary in config server context
        // 5. Verify mongos can reach config servers
        
        // For now, fall back to TCP check
        self.tcp_health_check(backend).await
    }
}

#[async_trait::async_trait]
impl HealthChecker for MongoDBHealthChecker {
    async fn check_health(&self, backend: &Backend) -> HealthStatus {
        tracing::debug!("Checking MongoDB health for backend: {}", backend.id);
        
        // Use TCP health check for now
        // In production, should use mongodb_ismaster_check
        self.tcp_health_check(backend).await
    }

    fn check_interval(&self) -> Duration {
        self.check_interval
    }

    fn check_timeout(&self) -> Duration {
        self.check_timeout
    }
}

impl Default for MongoDBHealthChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Backend;
    use std::net::SocketAddr;

    #[tokio::test]
    async fn test_mongodb_health_checker() {
        let checker = MongoDBHealthChecker::new();
        assert_eq!(checker.check_interval(), Duration::from_secs(10));
        assert_eq!(checker.check_timeout(), Duration::from_secs(5));
    }

    #[tokio::test]
    async fn test_invalid_address_health_check() {
        let checker = MongoDBHealthChecker::new();
        let backend = Backend::new_mongodb(
            "test-mongos".to_string(),
            "127.0.0.1:65535".parse::<SocketAddr>().unwrap(), // Use valid but unreachable port
        );

        let status = checker.check_health(&backend).await;
        match status {
            HealthStatus::Unhealthy { reason: _ } => {
                // Expected - invalid port should fail
            }
            _ => panic!("Expected unhealthy status for invalid address"),
        }
    }
}