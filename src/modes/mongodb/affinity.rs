/// Session affinity management for MongoDB connections
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use sha2::{Sha256, Digest};
use std::fmt;

/// Client identification strategy for session affinity
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ClientIdentifier {
    /// Traditional socket address (has NAT limitations)
    SocketAddr(SocketAddr),
    /// Connection fingerprint based on connection characteristics
    ConnectionFingerprint(String),
    /// Client-provided session ID (from MongoDB connection metadata)
    SessionId(String),
    /// Hybrid approach combining multiple factors
    Hybrid {
        socket_addr: SocketAddr,
        fingerprint: String,
    },
}

impl fmt::Display for ClientIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClientIdentifier::SocketAddr(addr) => write!(f, "socket:{}", addr),
            ClientIdentifier::ConnectionFingerprint(fp) => write!(f, "fingerprint:{}", fp),
            ClientIdentifier::SessionId(id) => write!(f, "session:{}", id),
            ClientIdentifier::Hybrid { socket_addr, fingerprint } => {
                write!(f, "hybrid:{}:{}", socket_addr, fingerprint)
            }
        }
    }
}

/// Client session information for affinity tracking
#[derive(Debug, Clone)]
pub struct ClientSession {
    pub client_id: ClientIdentifier,
    pub backend_id: String,
    pub first_connection: SystemTime,
    pub last_activity: SystemTime,
    pub connection_count: u64,
    pub nat_friendly: bool,
}

/// Session affinity manager specifically for MongoDB
/// Ensures that the same client always connects to the same mongos
/// Now supports multiple client identification strategies for NAT-friendly operation
pub struct AffinityManager {
    sessions: Arc<RwLock<HashMap<ClientIdentifier, ClientSession>>>,
    session_timeout: Duration,
    identification_strategy: ClientIdentificationStrategy,
}

/// Strategy for identifying clients
#[derive(Debug, Clone)]
pub enum ClientIdentificationStrategy {
    /// Use socket address only (legacy, has NAT issues)
    SocketAddressOnly,
    /// Use connection fingerprinting (NAT-friendly)
    ConnectionFingerprint,
    /// Use client-provided session IDs when available
    SessionId,
    /// Adaptive strategy that tries multiple approaches
    Adaptive,
}

impl AffinityManager {
    pub fn new(session_timeout: Duration) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            session_timeout,
            identification_strategy: ClientIdentificationStrategy::Adaptive,
        }
    }
    
    pub fn with_strategy(session_timeout: Duration, strategy: ClientIdentificationStrategy) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            session_timeout,
            identification_strategy: strategy,
        }
    }
    
    /// Generate client identifier based on the configured strategy
    pub fn generate_client_identifier(&self, client_addr: SocketAddr, connection_data: Option<&[u8]>) -> ClientIdentifier {
        match &self.identification_strategy {
            ClientIdentificationStrategy::SocketAddressOnly => {
                ClientIdentifier::SocketAddr(client_addr)
            }
            ClientIdentificationStrategy::ConnectionFingerprint => {
                let fingerprint = self.generate_connection_fingerprint(client_addr, connection_data);
                ClientIdentifier::ConnectionFingerprint(fingerprint)
            }
            ClientIdentificationStrategy::SessionId => {
                // Try to extract session ID from connection data, fallback to fingerprint
                if let Some(session_id) = self.extract_session_id(connection_data) {
                    ClientIdentifier::SessionId(session_id)
                } else {
                    let fingerprint = self.generate_connection_fingerprint(client_addr, connection_data);
                    ClientIdentifier::ConnectionFingerprint(fingerprint)
                }
            }
            ClientIdentificationStrategy::Adaptive => {
                let fingerprint = self.generate_connection_fingerprint(client_addr, connection_data);
                ClientIdentifier::Hybrid {
                    socket_addr: client_addr,
                    fingerprint,
                }
            }
        }
    }
    
    /// Generate connection fingerprint based on connection characteristics
    fn generate_connection_fingerprint(&self, client_addr: SocketAddr, connection_data: Option<&[u8]>) -> String {
        let mut hasher = Sha256::new();
        
        // Include IP address (but not port to handle NAT port changes)
        hasher.update(client_addr.ip().to_string().as_bytes());
        
        // Include connection timing characteristics
        hasher.update(SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)
                     .unwrap_or_default().as_secs().to_string().as_bytes());
        
        // Include connection data characteristics if available
        if let Some(data) = connection_data {
            // Use first few bytes as connection signature (e.g., MongoDB handshake)
            let signature_len = std::cmp::min(data.len(), 32);
            hasher.update(&data[..signature_len]);
        }
        
        // Create a shorter, more manageable fingerprint
        let result = hasher.finalize();
        hex::encode(&result[..8]) // Use first 8 bytes for shorter fingerprint
    }
    
    /// Extract session ID from MongoDB connection data if available
    fn extract_session_id(&self, connection_data: Option<&[u8]>) -> Option<String> {
        // In a full implementation, this would parse MongoDB Wire Protocol
        // to extract client session information from the handshake
        // For now, return None to fallback to fingerprinting
        connection_data.and_then(|_data| {
            // TODO: Implement MongoDB Wire Protocol parsing to extract session ID
            // This would look for client metadata in the initial handshake
            None
        })
    }

    /// Get or assign backend for client with session affinity (enhanced with new identification strategies)
    pub async fn get_backend_for_client(
        &self,
        client_addr: SocketAddr,
        available_backends: &[String],
        selection_fn: impl FnOnce(&[String]) -> Option<String>,
        connection_data: Option<&[u8]>,
    ) -> Option<String> {
        // Generate client identifier based on configured strategy
        let client_id = self.generate_client_identifier(client_addr, connection_data);
        
        // Check if client has existing session
        {
            let mut sessions = self.sessions.write().await;
            if let Some(session) = sessions.get_mut(&client_id) {
                // Verify backend is still available
                if available_backends.contains(&session.backend_id) {
                    // Update session activity
                    session.last_activity = SystemTime::now();
                    session.connection_count += 1;
                    log::debug!("Reusing existing session for client {}: backend {}", 
                              client_id, session.backend_id);
                    return Some(session.backend_id.clone());
                } else {
                    // Backend no longer available, remove session
                    log::warn!("Backend {} no longer available for client {}, removing session", 
                             session.backend_id, client_id);
                    sessions.remove(&client_id);
                }
            }
        }

        // No existing session or backend unavailable, create new session
        if let Some(backend_id) = selection_fn(available_backends) {
            let nat_friendly = matches!(client_id, 
                ClientIdentifier::ConnectionFingerprint(_) | 
                ClientIdentifier::SessionId(_) | 
                ClientIdentifier::Hybrid { .. }
            );
            
            let session = ClientSession {
                client_id: client_id.clone(),
                backend_id: backend_id.clone(),
                first_connection: SystemTime::now(),
                last_activity: SystemTime::now(),
                connection_count: 1,
                nat_friendly,
            };

            // Store new session
            {
                let mut sessions = self.sessions.write().await;
                sessions.insert(client_id.clone(), session);
            }
            
            log::info!("Created new session for client {} (NAT-friendly: {}): backend {}", 
                     client_id, nat_friendly, backend_id);

            Some(backend_id)
        } else {
            log::error!("No available backends for client {}", client_id);
            None
        }
    }
    
    /// Legacy method for backward compatibility
    pub async fn get_backend_for_client_legacy(
        &self,
        client_addr: SocketAddr,
        available_backends: &[String],
        selection_fn: impl FnOnce(&[String]) -> Option<String>,
    ) -> Option<String> {
        self.get_backend_for_client(client_addr, available_backends, selection_fn, None).await
    }

    /// Remove client session (when client disconnects) - enhanced version
    pub async fn remove_client_session(&self, client_id: &ClientIdentifier) -> Option<ClientSession> {
        let mut sessions = self.sessions.write().await;
        let removed = sessions.remove(client_id);
        if let Some(ref session) = removed {
            log::info!("Removed session for client {}: backend {}", client_id, session.backend_id);
        }
        removed
    }
    
    /// Remove client session by socket address (legacy compatibility)
    pub async fn remove_client_session_by_addr(&self, client_addr: SocketAddr) -> Option<ClientSession> {
        let client_id = ClientIdentifier::SocketAddr(client_addr);
        self.remove_client_session(&client_id).await
    }

    /// Get session information for a client - enhanced version
    pub async fn get_client_session(&self, client_id: &ClientIdentifier) -> Option<ClientSession> {
        let sessions = self.sessions.read().await;
        sessions.get(client_id).cloned()
    }
    
    /// Get session information by socket address (legacy compatibility)
    pub async fn get_client_session_by_addr(&self, client_addr: SocketAddr) -> Option<ClientSession> {
        let client_id = ClientIdentifier::SocketAddr(client_addr);
        self.get_client_session(&client_id).await
    }

    /// Get all sessions for a specific backend
    pub async fn get_sessions_for_backend(&self, backend_id: &str) -> Vec<ClientSession> {
        let sessions = self.sessions.read().await;
        sessions
            .values()
            .filter(|session| session.backend_id == backend_id)
            .cloned()
            .collect()
    }

    /// Clean up expired sessions
    pub async fn cleanup_expired_sessions(&self) -> usize {
        let mut sessions = self.sessions.write().await;
        let now = SystemTime::now();
        let mut expired_clients = Vec::new();

        for (client_addr, session) in sessions.iter() {
            if let Ok(elapsed) = now.duration_since(session.last_activity) {
                if elapsed > self.session_timeout {
                    expired_clients.push(client_addr.clone());
                }
            }
        }

        for client_addr in &expired_clients {
            sessions.remove(client_addr);
        }

        expired_clients.len()
    }

    /// Get session statistics
    pub async fn get_statistics(&self) -> AffinityStatistics {
        let sessions = self.sessions.read().await;
        let total_sessions = sessions.len();

        let mut backend_counts = HashMap::new();
        let mut total_connections = 0;

        for session in sessions.values() {
            *backend_counts
                .entry(session.backend_id.clone())
                .or_insert(0) += 1;
            total_connections += session.connection_count;
        }

        AffinityStatistics {
            total_sessions,
            total_connections,
            backend_distribution: backend_counts,
        }
    }

    /// Start background cleanup task
    pub async fn start_cleanup_task(self: Arc<Self>) {
        let cleanup_interval = self.session_timeout / 4;
        let mut interval = tokio::time::interval(cleanup_interval);

        loop {
            interval.tick().await;
            let cleaned = self.cleanup_expired_sessions().await;
            if cleaned > 0 {
                tracing::debug!("Cleaned up {} expired MongoDB sessions", cleaned);
            }
        }
    }
}

/// Statistics about session affinity
#[derive(Debug, Clone)]
pub struct AffinityStatistics {
    pub total_sessions: usize,
    pub total_connections: u64,
    pub backend_distribution: HashMap<String, usize>,
}

impl Default for AffinityManager {
    fn default() -> Self {
        Self::new(Duration::from_secs(3600)) // 1 hour default
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[tokio::test]
    async fn test_session_affinity() {
        let manager = AffinityManager::new(Duration::from_secs(60));
        let client_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12345);
        let backends = vec!["backend1".to_string(), "backend2".to_string()];

        // First call should create new session
        let backend1 = manager
            .get_backend_for_client(client_addr, &backends, |backends| Some(backends[0].clone()), None)
            .await;
        assert_eq!(backend1, Some("backend1".to_string()));

        // Second call should return same backend due to affinity
        let backend2 = manager
            .get_backend_for_client(client_addr, &backends, |backends| Some(backends[1].clone()), None)
            .await;
        assert_eq!(backend2, Some("backend1".to_string())); // Should still be backend1

        // Verify session was created
        let client_id = ClientIdentifier::SocketAddr(client_addr);
        let session = manager.get_client_session(&client_id).await;
        assert!(session.is_some());
        assert_eq!(session.unwrap().backend_id, "backend1");
    }

    #[tokio::test]
    async fn test_session_cleanup() {
        let manager = AffinityManager::new(Duration::from_millis(10)); // Very short timeout
        let client_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12345);
        let backends = vec!["backend1".to_string()];

        // Create session
        manager
            .get_backend_for_client(client_addr, &backends, |backends| Some(backends[0].clone()), None)
            .await;

        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(20)).await;

        // Cleanup should remove the session
        let cleaned = manager.cleanup_expired_sessions().await;
        assert_eq!(cleaned, 1);

        // Session should be gone
        let client_id = ClientIdentifier::SocketAddr(client_addr);
        let session = manager.get_client_session(&client_id).await;
        assert!(session.is_none());
    }
}
