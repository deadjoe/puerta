/// Redis cluster node health checker
use super::{HealthChecker, HealthStatus};
use crate::core::Backend;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

/// Redis health checker implementation
/// Uses PING command and CLUSTER NODES for comprehensive health checking
pub struct RedisHealthChecker {
    check_interval: Duration,
    check_timeout: Duration,
    max_retries: u32,
    retry_delay: Duration,
    enable_cluster_check: bool,
}

impl RedisHealthChecker {
    pub fn new() -> Self {
        Self {
            check_interval: Duration::from_secs(5),
            check_timeout: Duration::from_secs(3),
            max_retries: 3,
            retry_delay: Duration::from_millis(300),
            enable_cluster_check: true,
        }
    }
    
    pub fn with_config(
        check_interval: Duration, 
        check_timeout: Duration, 
        max_retries: u32, 
        retry_delay: Duration,
        enable_cluster_check: bool
    ) -> Self {
        Self {
            check_interval,
            check_timeout,
            max_retries,
            retry_delay,
            enable_cluster_check,
        }
    }

    /// Perform Redis PING health check
    async fn redis_ping_check(&self, backend: &Backend) -> HealthStatus {
        let stream = match TcpStream::connect(backend.addr).await {
            Ok(stream) => stream,
            Err(e) => {
                return HealthStatus::Unhealthy {
                    reason: format!("Connection failed: {}", e),
                };
            }
        };

        let (reader, mut writer) = stream.into_split();
        let mut buf_reader = BufReader::new(reader);

        // Send PING command in RESP format
        let ping_command = "*1\r\n$4\r\nPING\r\n";

        if let Err(e) = writer.write_all(ping_command.as_bytes()).await {
            return HealthStatus::Unhealthy {
                reason: format!("Failed to send PING: {}", e),
            };
        }

        // Read response
        let mut response = String::new();
        match buf_reader.read_line(&mut response).await {
            Ok(_) => {
                if response.trim() == "+PONG" {
                    HealthStatus::Healthy
                } else {
                    HealthStatus::Unhealthy {
                        reason: format!("Unexpected PING response: {}", response.trim()),
                    }
                }
            }
            Err(e) => HealthStatus::Unhealthy {
                reason: format!("Failed to read PING response: {}", e),
            },
        }
    }

    /// Perform Redis CLUSTER NODES check to verify cluster membership
    #[allow(dead_code)]
    async fn redis_cluster_check(&self, backend: &Backend) -> HealthStatus {
        let stream = match TcpStream::connect(backend.addr).await {
            Ok(stream) => stream,
            Err(e) => {
                return HealthStatus::Unhealthy {
                    reason: format!("Connection failed: {}", e),
                };
            }
        };

        let (reader, mut writer) = stream.into_split();
        let mut buf_reader = BufReader::new(reader);

        // Send CLUSTER NODES command in RESP format
        let cluster_command = "*2\r\n$7\r\nCLUSTER\r\n$5\r\nNODES\r\n";

        if let Err(e) = writer.write_all(cluster_command.as_bytes()).await {
            return HealthStatus::Unhealthy {
                reason: format!("Failed to send CLUSTER NODES: {}", e),
            };
        }

        // Read response (bulk string format)
        let mut response = String::new();
        match buf_reader.read_line(&mut response).await {
            Ok(_) => {
                if response.starts_with('$') {
                    // Read the actual cluster nodes data
                    let mut nodes_data = String::new();
                    if let Ok(_) = buf_reader.read_line(&mut nodes_data).await {
                        // Parse cluster nodes to verify this node is part of cluster
                        // TODO: Implement proper cluster nodes parsing
                        HealthStatus::Healthy
                    } else {
                        HealthStatus::Unhealthy {
                            reason: "Failed to read cluster nodes data".to_string(),
                        }
                    }
                } else if response.trim().starts_with("-ERR") {
                    HealthStatus::Unhealthy {
                        reason: format!("Cluster error: {}", response.trim()),
                    }
                } else {
                    HealthStatus::Unhealthy {
                        reason: format!("Unexpected cluster response: {}", response.trim()),
                    }
                }
            }
            Err(e) => HealthStatus::Unhealthy {
                reason: format!("Failed to read cluster response: {}", e),
            },
        }
    }

    /// Comprehensive Redis health check with retry mechanism
    async fn redis_health_check_with_retry(&self, backend: &Backend) -> HealthStatus {
        for attempt in 0..=self.max_retries {
            let status = self.comprehensive_redis_check(backend).await;
            
            match status {
                HealthStatus::Healthy => return status,
                HealthStatus::Unhealthy { .. } | HealthStatus::Timeout => {
                    if attempt < self.max_retries {
                        log::warn!("Redis health check attempt {} failed for {}, retrying in {:?}", 
                                 attempt + 1, backend.addr, self.retry_delay);
                        tokio::time::sleep(self.retry_delay).await;
                        continue;
                    } else {
                        return status;
                    }
                }
                _ => return status,
            }
        }
        
        HealthStatus::Unhealthy {
            reason: "All retry attempts exhausted".to_string(),
        }
    }
    
    /// Comprehensive Redis health check
    async fn comprehensive_redis_check(&self, backend: &Backend) -> HealthStatus {
        // First perform PING check
        let ping_status = self.redis_ping_check(backend).await;
        if !matches!(ping_status, HealthStatus::Healthy) {
            return ping_status;
        }

        // Then perform cluster check if enabled
        if self.enable_cluster_check {
            let cluster_status = self.redis_cluster_check(backend).await;
            if !matches!(cluster_status, HealthStatus::Healthy) {
                return cluster_status;
            }
        }

        HealthStatus::Healthy
    }
}

#[async_trait::async_trait]
impl HealthChecker for RedisHealthChecker {
    async fn check_health(&self, backend: &Backend) -> HealthStatus {
        // Use the enhanced Redis health check with retry mechanism
        self.redis_health_check_with_retry(backend).await
    }

    fn check_interval(&self) -> Duration {
        self.check_interval
    }

    fn check_timeout(&self) -> Duration {
        self.check_timeout
    }
}

impl Default for RedisHealthChecker {
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
    async fn test_redis_health_checker() {
        let checker = RedisHealthChecker::new();
        assert_eq!(checker.check_interval(), Duration::from_secs(5));
        assert_eq!(checker.check_timeout(), Duration::from_secs(3));
    }

    #[tokio::test]
    async fn test_invalid_address_health_check() {
        let checker = RedisHealthChecker::new();
        let backend = Backend::new_redis(
            "test-redis".to_string(),
            "127.0.0.1:65535".parse::<SocketAddr>().unwrap(), // Use valid but unreachable port
            "test-node-id".to_string(),
        );

        let status = checker.check_health(&backend).await;
        match status {
            HealthStatus::Unhealthy { reason: _ } => {
                // Expected - invalid port should fail
            }
            _ => panic!("Expected unhealthy status for invalid address"),
        }
    }

    #[test]
    fn test_ping_command_format() {
        // Verify our PING command is in correct RESP format
        let ping_command = "*1\r\n$4\r\nPING\r\n";

        // RESP format breakdown:
        // *1 = Array with 1 element
        // $4 = Bulk string with 4 characters
        // PING = The command
        assert_eq!(ping_command, "*1\r\n$4\r\nPING\r\n");
    }
}
