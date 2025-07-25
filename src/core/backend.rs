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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Backend, BackendMetadata};
    use std::net::SocketAddr;
    use std::time::SystemTime;

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

    #[tokio::test]
    async fn test_backend_manager_creation() {
        let manager = BackendManager::new();
        let backends = manager.backends.read().await;
        assert!(backends.is_empty());
    }

    #[tokio::test]
    async fn test_add_backend() {
        let manager = BackendManager::new();
        let backend = create_test_backend("test1", true);
        
        manager.add_backend(backend.clone()).await;
        
        let retrieved = manager.get_backend("test1").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, "test1");
    }

    #[tokio::test]
    async fn test_remove_backend() {
        let manager = BackendManager::new();
        let backend = create_test_backend("test1", true);
        
        manager.add_backend(backend).await;
        assert!(manager.get_backend("test1").await.is_some());
        
        let removed = manager.remove_backend("test1").await;
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().id, "test1");
        
        assert!(manager.get_backend("test1").await.is_none());
    }

    #[tokio::test]
    async fn test_get_healthy_backends() {
        let manager = BackendManager::new();
        
        let healthy_backend = create_test_backend("healthy", true);
        let unhealthy_backend = create_test_backend("unhealthy", false);
        
        manager.add_backend(healthy_backend).await;
        manager.add_backend(unhealthy_backend).await;
        
        let healthy_backends = manager.get_healthy_backends().await;
        assert_eq!(healthy_backends.len(), 1);
        assert_eq!(healthy_backends[0].id, "healthy");
        assert!(healthy_backends[0].healthy);
    }

    #[tokio::test]
    async fn test_backend_manager_default() {
        let manager = BackendManager::default();
        let backends = manager.backends.read().await;
        assert!(backends.is_empty());
    }

    #[tokio::test]
    async fn test_get_pool() {
        let manager = BackendManager::new();
        let pool = manager.get_pool();
        
        // Verify we can use the pool reference
        let backends = pool.read().await;
        assert!(backends.is_empty());
    }

    #[tokio::test]
    async fn test_multiple_backends() {
        let manager = BackendManager::new();
        
        for i in 0..5 {
            let backend = create_test_backend(&format!("backend{}", i), i % 2 == 0);
            manager.add_backend(backend).await;
        }
        
        let healthy_backends = manager.get_healthy_backends().await;
        assert_eq!(healthy_backends.len(), 3); // backends 0, 2, 4 are healthy
        
        // Test removal of non-existent backend
        let removed = manager.remove_backend("nonexistent").await;
        assert!(removed.is_none());
    }
}
