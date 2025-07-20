/// Session management for connection affinity

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;

/// Session information
#[derive(Debug, Clone)]
pub struct Session {
    pub client_addr: SocketAddr,
    pub backend_id: String,
    pub created_at: SystemTime,
    pub last_activity: SystemTime,
    pub connection_count: usize,
}

/// Session manager for tracking client-backend affinity
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<SocketAddr, Session>>>,
    session_timeout: Duration,
}

impl SessionManager {
    pub fn new(session_timeout: Duration) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            session_timeout,
        }
    }

    /// Create or update a session
    pub async fn create_session(&self, client_addr: SocketAddr, backend_id: String) {
        let mut sessions = self.sessions.write().await;
        let now = SystemTime::now();

        match sessions.get_mut(&client_addr) {
            Some(session) => {
                // Update existing session
                session.last_activity = now;
                session.connection_count += 1;
                if session.backend_id != backend_id {
                    // Backend changed (should be rare)
                    session.backend_id = backend_id;
                }
            }
            None => {
                // Create new session
                let session = Session {
                    client_addr,
                    backend_id,
                    created_at: now,
                    last_activity: now,
                    connection_count: 1,
                };
                sessions.insert(client_addr, session);
            }
        }
    }

    /// Get session for a client
    pub async fn get_session(&self, client_addr: SocketAddr) -> Option<Session> {
        let sessions = self.sessions.read().await;
        sessions.get(&client_addr).cloned()
    }

    /// Remove a session
    pub async fn remove_session(&self, client_addr: SocketAddr) -> Option<Session> {
        let mut sessions = self.sessions.write().await;
        sessions.remove(&client_addr)
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

    /// Get all sessions for a specific backend
    pub async fn get_sessions_for_backend(&self, backend_id: &str) -> Vec<Session> {
        let sessions = self.sessions.read().await;
        sessions
            .values()
            .filter(|session| session.backend_id == backend_id)
            .cloned()
            .collect()
    }

    /// Get total session count
    pub async fn session_count(&self) -> usize {
        let sessions = self.sessions.read().await;
        sessions.len()
    }

    /// Start background cleanup task
    pub async fn start_cleanup_task(self: Arc<Self>) {
        let mut interval = tokio::time::interval(self.session_timeout / 4);
        
        loop {
            interval.tick().await;
            let cleaned = self.cleanup_expired_sessions().await;
            if cleaned > 0 {
                tracing::debug!("Cleaned up {} expired sessions", cleaned);
            }
        }
    }
}