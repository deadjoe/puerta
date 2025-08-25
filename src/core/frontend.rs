/// Frontend connection management
use crate::core::Frontend;
use fnv::FnvHashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Frontend connection manager
pub struct FrontendManager {
    connections: Arc<RwLock<FnvHashMap<String, Frontend>>>,
}

impl FrontendManager {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(FnvHashMap::default())),
        }
    }

    /// Register a new frontend connection
    pub async fn register_connection(&self, frontend: Frontend) {
        let mut connections = self.connections.write().await;
        connections.insert(frontend.id.clone(), frontend);
    }

    /// Unregister a frontend connection
    pub async fn unregister_connection(&self, connection_id: &str) -> Option<Frontend> {
        let mut connections = self.connections.write().await;
        connections.remove(connection_id)
    }

    /// Get connection by ID
    pub async fn get_connection(&self, connection_id: &str) -> Option<Frontend> {
        let connections = self.connections.read().await;
        connections.get(connection_id).cloned()
    }

    /// Get all connections from a specific client address
    pub async fn get_connections_by_client(&self, client_addr: SocketAddr) -> Vec<Frontend> {
        let connections = self.connections.read().await;
        connections
            .values()
            .filter(|conn| conn.client_addr == client_addr)
            .cloned()
            .collect()
    }

    /// Get total connection count
    pub async fn connection_count(&self) -> usize {
        let connections = self.connections.read().await;
        connections.len()
    }
}

impl Default for FrontendManager {
    fn default() -> Self {
        Self::new()
    }
}
