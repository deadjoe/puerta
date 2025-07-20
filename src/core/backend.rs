/// Backend service management
use crate::core::Backend;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Backend pool for managing multiple backend services
pub type BackendPool = Arc<RwLock<HashMap<String, Backend>>>;

/// Backend manager for operations on backend services
pub struct BackendManager {
    backends: BackendPool,
}

impl BackendManager {
    pub fn new() -> Self {
        Self {
            backends: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add a backend to the pool
    pub async fn add_backend(&self, backend: Backend) {
        let mut backends = self.backends.write().await;
        backends.insert(backend.id.clone(), backend);
    }

    /// Remove a backend from the pool
    pub async fn remove_backend(&self, backend_id: &str) -> Option<Backend> {
        let mut backends = self.backends.write().await;
        backends.remove(backend_id)
    }

    /// Get a backend by ID
    pub async fn get_backend(&self, backend_id: &str) -> Option<Backend> {
        let backends = self.backends.read().await;
        backends.get(backend_id).cloned()
    }

    /// Get all healthy backends
    pub async fn get_healthy_backends(&self) -> Vec<Backend> {
        let backends = self.backends.read().await;
        backends.values().filter(|b| b.healthy).cloned().collect()
    }

    /// Get the backend pool reference
    pub fn get_pool(&self) -> BackendPool {
        Arc::clone(&self.backends)
    }
}

impl Default for BackendManager {
    fn default() -> Self {
        Self::new()
    }
}
