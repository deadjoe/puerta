/// MongoDB mongos health checker
use super::{HealthChecker, HealthStatus};
use crate::core::Backend;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// MongoDB health checker implementation
/// Uses ismaster command to check mongos availability and status
pub struct MongoDBHealthChecker {
    check_interval: Duration,
    check_timeout: Duration,
    max_retries: u32,
    retry_delay: Duration,
}

impl MongoDBHealthChecker {
    pub fn new() -> Self {
        Self {
            check_interval: Duration::from_secs(10),
            check_timeout: Duration::from_secs(5),
            max_retries: 3,
            retry_delay: Duration::from_millis(500),
        }
    }
    
    pub fn with_config(check_interval: Duration, check_timeout: Duration, max_retries: u32, retry_delay: Duration) -> Self {
        Self {
            check_interval,
            check_timeout,
            max_retries,
            retry_delay,
        }
    }

    /// Perform comprehensive MongoDB health check with retry mechanism
    async fn mongodb_health_check_with_retry(&self, backend: &Backend) -> HealthStatus {
        for attempt in 0..=self.max_retries {
            let status = self.mongodb_wire_protocol_check(backend).await;
            
            match status {
                HealthStatus::Healthy => return status,
                HealthStatus::Unhealthy { .. } | HealthStatus::Timeout => {
                    if attempt < self.max_retries {
                        log::warn!("MongoDB health check attempt {} failed for {}, retrying in {:?}", 
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
    
    /// Implement proper MongoDB Wire Protocol health check using ismaster command
    async fn mongodb_wire_protocol_check(&self, backend: &Backend) -> HealthStatus {
        let stream = match TcpStream::connect(backend.addr).await {
            Ok(stream) => stream,
            Err(e) => {
                return HealthStatus::Unhealthy {
                    reason: format!("Connection failed: {e}"),
                };
            }
        };
        
        let mut stream = stream;
        
        // Create ismaster command in MongoDB Wire Protocol format
        let ismaster_command = self.create_ismaster_command();
        
        // Send ismaster command
        if let Err(e) = stream.write_all(&ismaster_command).await {
            return HealthStatus::Unhealthy {
                reason: format!("Failed to send ismaster command: {e}"),
            };
        }
        
        // Read response header (16 bytes)
        let mut header = [0u8; 16];
        if let Err(e) = stream.read_exact(&mut header).await {
            return HealthStatus::Unhealthy {
                reason: format!("Failed to read response header: {e}"),
            };
        }
        
        // Parse message length from header (first 4 bytes, little-endian)
        let message_length = u32::from_le_bytes([header[0], header[1], header[2], header[3]]) as usize;
        
        if !(16..=48 * 1024 * 1024).contains(&message_length) {
            return HealthStatus::Unhealthy {
                reason: format!("Invalid message length: {message_length}"),
            };
        }
        
        // Read remaining message body
        let body_length = message_length - 16;
        let mut body = vec![0u8; body_length];
        if let Err(e) = stream.read_exact(&mut body).await {
            return HealthStatus::Unhealthy {
                reason: format!("Failed to read response body: {e}"),
            };
        }
        
        // Parse response to check if ismaster succeeded
        self.parse_ismaster_response(&body)
    }
    
    /// Create MongoDB Wire Protocol ismaster command
    fn create_ismaster_command(&self) -> Vec<u8> {
        // MongoDB Wire Protocol message structure:
        // - Message Length (4 bytes)
        // - Request ID (4 bytes) 
        // - Response To (4 bytes)
        // - OpCode (4 bytes) - OP_QUERY = 2004
        // - Flags (4 bytes)
        // - Collection Name (null-terminated string)
        // - Number to Skip (4 bytes)
        // - Number to Return (4 bytes)
        // - Query Document (BSON)
        
        let mut command = Vec::new();
        
        // Placeholder for message length (will be filled later)
        command.extend_from_slice(&[0u8; 4]);
        
        // Request ID (arbitrary)
        command.extend_from_slice(&1u32.to_le_bytes());
        
        // Response To (0 for new request)
        command.extend_from_slice(&0u32.to_le_bytes());
        
        // OpCode (OP_QUERY = 2004)
        command.extend_from_slice(&2004u32.to_le_bytes());
        
        // Flags (0)
        command.extend_from_slice(&0u32.to_le_bytes());
        
        // Collection name: "admin.$cmd" (null-terminated)
        command.extend_from_slice(b"admin.$cmd\0");
        
        // Number to skip (0)
        command.extend_from_slice(&0u32.to_le_bytes());
        
        // Number to return (1)
        command.extend_from_slice(&1u32.to_le_bytes());
        
        // Query document: {"ismaster": 1} in BSON format
        let bson_query = self.create_ismaster_bson();
        command.extend_from_slice(&bson_query);
        
        // Update message length
        let total_length = command.len() as u32;
        command[0..4].copy_from_slice(&total_length.to_le_bytes());
        
        command
    }
    
    /// Create BSON document for ismaster query: {"ismaster": 1}
    fn create_ismaster_bson(&self) -> Vec<u8> {
        let mut bson = Vec::new();
        
        // Document length placeholder
        bson.extend_from_slice(&[0u8; 4]);
        
        // Field type: int32 (0x10)
        bson.push(0x10);
        
        // Field name: "ismaster" (null-terminated)
        bson.extend_from_slice(b"ismaster\0");
        
        // Field value: 1 (int32)
        bson.extend_from_slice(&1i32.to_le_bytes());
        
        // Document terminator
        bson.push(0x00);
        
        // Update document length
        let doc_length = bson.len() as u32;
        bson[0..4].copy_from_slice(&doc_length.to_le_bytes());
        
        bson
    }
    
    /// Parse ismaster response to determine health status
    fn parse_ismaster_response(&self, body: &[u8]) -> HealthStatus {
        if body.len() < 4 {
            return HealthStatus::Unhealthy {
                reason: "Response body too short".to_string(),
            };
        }
        
        // For a comprehensive implementation, we would parse the full BSON response
        // For now, we check if we received a valid BSON document structure
        let doc_length = u32::from_le_bytes([body[0], body[1], body[2], body[3]]) as usize;
        
        if doc_length > body.len() || doc_length < 5 {
            return HealthStatus::Unhealthy {
                reason: "Invalid BSON document length".to_string(),
            };
        }
        
        // Check for document terminator
        if body.get(doc_length - 1) != Some(&0x00) {
            return HealthStatus::Unhealthy {
                reason: "Invalid BSON document terminator".to_string(),
            };
        }
        
        // If we got a valid BSON response, consider the mongos healthy
        // In a full implementation, we would parse the response to check:
        // - ismaster: true/false
        // - msg: "isdbgrid" (indicates this is a mongos)
        // - hosts: array of shard hosts
        // - setName: replica set name (if applicable)
        
        HealthStatus::Healthy
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

        // Delegate to the new Wire Protocol implementation
        self.mongodb_wire_protocol_check(backend).await
    }
}

#[async_trait::async_trait]
impl HealthChecker for MongoDBHealthChecker {
    async fn check_health(&self, backend: &Backend) -> HealthStatus {
        // Use the enhanced MongoDB Wire Protocol health check with retry mechanism
        self.mongodb_health_check_with_retry(backend).await
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
