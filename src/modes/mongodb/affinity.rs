/// Session affinity management for MongoDB connections

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;

/// Client session information for affinity tracking
#[derive(Debug, Clone)]
pub struct ClientSession {
    pub client_addr: SocketAddr,
    pub backend_id: String,
    pub first_connection: SystemTime,
    pub last_activity: SystemTime,
    pub connection_count: u64,
}

/// Session affinity manager specifically for MongoDB
/// Ensures that the same client always connects to the same mongos
pub struct AffinityManager {
    sessions: Arc<RwLock<HashMap<SocketAddr, ClientSession>>>,
    session_timeout: Duration,
}

impl AffinityManager {
    pub fn new(session_timeout: Duration) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            session_timeout,
        }
    }

    /// Get or assign backend for client with session affinity
    pub async fn get_backend_for_client(
        &self,
        client_addr: SocketAddr,
        available_backends: &[String],
        selection_fn: impl FnOnce(&[String]) -> Option<String>,
    ) -> Option<String> {
        // Check if client has existing session
        {
            let mut sessions = self.sessions.write().await;
            if let Some(session) = sessions.get_mut(&client_addr) {
                // Verify backend is still available
                if available_backends.contains(&session.backend_id) {
                    // Update session activity
                    session.last_activity = SystemTime::now();
                    session.connection_count += 1;
                    return Some(session.backend_id.clone());
                } else {
                    // Backend no longer available, remove session
                    sessions.remove(&client_addr);
                }
            }
        }

        // No existing session or backend unavailable, create new session
        if let Some(backend_id) = selection_fn(available_backends) {
            let now = SystemTime::now();
            let session = ClientSession {
                client_addr,
                backend_id: backend_id.clone(),
                first_connection: now,
                last_activity: now,
                connection_count: 1,
            };

            let mut sessions = self.sessions.write().await;
            sessions.insert(client_addr, session);
            Some(backend_id)
        } else {
            None
        }
    }

    /// Remove client session (when client disconnects)
    pub async fn remove_client_session(&self, client_addr: SocketAddr) -> Option<ClientSession> {
        let mut sessions = self.sessions.write().await;
        sessions.remove(&client_addr)
    }

    /// Get session information for a client
    pub async fn get_client_session(&self, client_addr: SocketAddr) -> Option<ClientSession> {
        let sessions = self.sessions.read().await;
        sessions.get(&client_addr).cloned()
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
                    expired_clients.push(*client_addr);
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
            *backend_counts.entry(session.backend_id.clone()).or_insert(0) += 1;
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
            .get_backend_for_client(client_addr, &backends, |backends| Some(backends[0].clone()))
            .await;
        assert_eq!(backend1, Some("backend1".to_string()));

        // Second call should return same backend due to affinity
        let backend2 = manager
            .get_backend_for_client(client_addr, &backends, |backends| Some(backends[1].clone()))
            .await;
        assert_eq!(backend2, Some("backend1".to_string())); // Should still be backend1

        // Verify session was created
        let session = manager.get_client_session(client_addr).await;
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
            .get_backend_for_client(client_addr, &backends, |backends| Some(backends[0].clone()))
            .await;

        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(20)).await;

        // Cleanup should remove the session
        let cleaned = manager.cleanup_expired_sessions().await;
        assert_eq!(cleaned, 1);

        // Session should be gone
        let session = manager.get_client_session(client_addr).await;
        assert!(session.is_none());
    }
}