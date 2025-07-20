pub mod affinity;
/// MongoDB Sharded Cluster mode implementation
///
/// This mode provides session-aware load balancing across multiple mongos instances.
/// Key features:
/// - Session affinity: ensures same client always connects to same mongos
/// - TCP-level load balancing (no deep MongoDB protocol parsing needed)
/// - Health checking of mongos instances
/// - Weighted round-robin load balancing for new sessions
pub mod balancer;

use crate::core::Backend;
use crate::modes::{BackendPool, RoutingDecision};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

/// MongoDB mode configuration
#[derive(Debug, Clone)]
pub struct MongoDBConfig {
    pub mongos_endpoints: Vec<String>,
    pub session_affinity_enabled: bool,
    pub session_timeout_sec: u64,
    pub health_check_interval_sec: u64,
}

impl MongoDBConfig {
    /// Create a new MongoDB configuration with validation
    pub fn new(
        mongos_endpoints: Vec<String>,
        session_affinity_enabled: bool,
        session_timeout_sec: u64,
        health_check_interval_sec: u64,
    ) -> Result<Self, String> {
        // Validate endpoints
        if mongos_endpoints.is_empty() {
            return Err("At least one mongos endpoint is required".to_string());
        }

        // Validate each endpoint format
        for endpoint in &mongos_endpoints {
            if endpoint.trim().is_empty() {
                return Err("Empty mongos endpoint not allowed".to_string());
            }

            // Basic format validation (host:port)
            if !endpoint.contains(':') {
                return Err(format!(
                    "Invalid mongos endpoint format '{}': must be host:port",
                    endpoint
                ));
            }
        }

        // Validate timeouts
        if session_timeout_sec == 0 {
            return Err("Session timeout must be greater than 0".to_string());
        }

        if health_check_interval_sec == 0 {
            return Err("Health check interval must be greater than 0".to_string());
        }

        Ok(Self {
            mongos_endpoints,
            session_affinity_enabled,
            session_timeout_sec,
            health_check_interval_sec,
        })
    }

    /// Get the number of mongos endpoints
    pub fn endpoint_count(&self) -> usize {
        self.mongos_endpoints.len()
    }

    /// Check if configuration is valid
    pub fn is_valid(&self) -> bool {
        !self.mongos_endpoints.is_empty()
            && self.session_timeout_sec > 0
            && self.health_check_interval_sec > 0
    }
}

/// Session affinity manager
/// Tracks which client should connect to which mongos instance
pub struct SessionAffinityManager {
    /// Maps client IP -> mongos backend ID
    client_to_backend: Arc<RwLock<HashMap<SocketAddr, String>>>,
    /// Round-robin counter for new sessions
    round_robin_counter: AtomicUsize,
}

impl Clone for SessionAffinityManager {
    fn clone(&self) -> Self {
        Self {
            client_to_backend: Arc::clone(&self.client_to_backend),
            round_robin_counter: AtomicUsize::new(self.round_robin_counter.load(Ordering::Relaxed)),
        }
    }
}

/// MongoDB proxy handler
#[derive(Clone)]
pub struct MongoDBProxy {
    config: MongoDBConfig,
    backends: BackendPool,
    affinity_manager: SessionAffinityManager,
    health_manager: Option<Arc<crate::health::HealthCheckManager>>,
}

impl SessionAffinityManager {
    pub fn new() -> Self {
        Self {
            client_to_backend: Arc::new(RwLock::new(HashMap::new())),
            round_robin_counter: AtomicUsize::new(0),
        }
    }

    /// Get the current number of active sessions
    pub async fn session_count(&self) -> usize {
        self.client_to_backend.read().await.len()
    }

    /// Get or assign backend for a client
    pub async fn get_backend_for_client(
        &self,
        client_addr: SocketAddr,
        available_backends: &[String],
    ) -> Option<String> {
        // First check if client has existing affinity
        {
            let affinity_map = self.client_to_backend.read().await;
            if let Some(backend_id) = affinity_map.get(&client_addr) {
                // Verify the backend is still available
                if available_backends.contains(backend_id) {
                    return Some(backend_id.clone());
                }
            }
        }

        // No existing affinity or backend unavailable, assign new one
        if available_backends.is_empty() {
            return None;
        }

        let index =
            self.round_robin_counter.fetch_add(1, Ordering::Relaxed) % available_backends.len();
        let backend_id = available_backends[index].clone();

        // Store the new affinity
        {
            let mut affinity_map = self.client_to_backend.write().await;
            affinity_map.insert(client_addr, backend_id.clone());
        }

        Some(backend_id)
    }

    /// Remove client affinity (when client disconnects)
    pub async fn remove_client_affinity(&self, client_addr: SocketAddr) -> bool {
        let mut affinity_map = self.client_to_backend.write().await;
        affinity_map.remove(&client_addr).is_some()
    }

    /// Clear all sessions (for testing or reset)
    pub async fn clear_all_sessions(&self) {
        let mut affinity_map = self.client_to_backend.write().await;
        affinity_map.clear();
    }

    /// Get all active client addresses
    pub async fn get_active_clients(&self) -> Vec<SocketAddr> {
        let affinity_map = self.client_to_backend.read().await;
        affinity_map.keys().cloned().collect()
    }
}

impl MongoDBProxy {
    pub fn new(config: MongoDBConfig) -> Self {
        Self {
            config,
            backends: Arc::new(RwLock::new(HashMap::new())),
            affinity_manager: SessionAffinityManager::new(),
            health_manager: None,
        }
    }

    pub fn with_health_check(mut self) -> Self {
        let health_checker = Box::new(crate::health::mongodb::MongoDBHealthChecker::new());
        self.health_manager = Some(Arc::new(crate::health::HealthCheckManager::new(
            health_checker,
        )));
        self
    }

    /// Initialize backends from configuration
    pub async fn initialize_backends(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut backends = self.backends.write().await;

        for (index, endpoint) in self.config.mongos_endpoints.iter().enumerate() {
            let addr: SocketAddr = endpoint.parse()?;
            let backend_id = format!("mongos-{}", index);
            let backend = Backend::new_mongodb(backend_id.clone(), addr);
            backends.insert(backend_id, backend);
        }

        Ok(())
    }

    /// Start health checking for all backends
    pub async fn start_health_checks(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(health_manager) = &self.health_manager {
            let backends = self.backends.clone();
            let health_manager = Arc::clone(health_manager);

            tokio::spawn(async move {
                loop {
                    {
                        let mut backends_guard = backends.write().await;
                        for backend in backends_guard.values_mut() {
                            let status = health_manager.check_backend_health(backend).await;
                            log::debug!("Health check for {}: {:?}", backend.id, status);
                        }
                    }

                    // Sleep between health check cycles
                    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                }
            });
        }

        Ok(())
    }

    /// Route request to appropriate mongos based on session affinity
    pub async fn route_request(&self, client_addr: SocketAddr) -> RoutingDecision {
        let backends = self.backends.read().await;

        // Get healthy backends
        let healthy_backends: Vec<String> = backends
            .values()
            .filter(|b| b.healthy)
            .map(|b| b.id.clone())
            .collect();

        if healthy_backends.is_empty() {
            return RoutingDecision::Error {
                message: "No healthy mongos instances available".to_string(),
            };
        }

        // Use session affinity if enabled
        if self.config.session_affinity_enabled {
            if let Some(backend_id) = self
                .affinity_manager
                .get_backend_for_client(client_addr, &healthy_backends)
                .await
            {
                return RoutingDecision::Route { backend_id };
            }
        }

        // Fallback to simple round-robin for new sessions
        let index = self
            .affinity_manager
            .round_robin_counter
            .load(Ordering::Relaxed)
            % healthy_backends.len();
        let backend_id = healthy_backends[index].clone();

        RoutingDecision::Route { backend_id }
    }

    /// Handle client disconnection
    pub async fn handle_client_disconnect(&self, client_addr: SocketAddr) -> bool {
        if self.config.session_affinity_enabled {
            self.affinity_manager
                .remove_client_affinity(client_addr)
                .await
        } else {
            false
        }
    }

    /// Get backend pool for health checking
    pub fn get_backends(&self) -> BackendPool {
        Arc::clone(&self.backends)
    }

    /// Get configuration for testing/debugging
    pub fn get_config(&self) -> &MongoDBConfig {
        &self.config
    }

    /// Get affinity manager for testing/debugging
    pub fn get_affinity_manager(&self) -> &SessionAffinityManager {
        &self.affinity_manager
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn test_mongodb_config_creation() {
        let config = MongoDBConfig::new(
            vec!["127.0.0.1:27017".to_string(), "127.0.0.1:27018".to_string()],
            true,
            300,
            10,
        )
        .unwrap();

        assert_eq!(config.endpoint_count(), 2);
        assert!(config.session_affinity_enabled);
        assert_eq!(config.session_timeout_sec, 300);
        assert_eq!(config.health_check_interval_sec, 10);
        assert!(config.is_valid());
    }

    #[test]
    fn test_mongodb_config_validation_empty_endpoints() {
        let result = MongoDBConfig::new(vec![], true, 300, 10);

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "At least one mongos endpoint is required"
        );
    }

    #[test]
    fn test_mongodb_config_validation_empty_endpoint() {
        let result = MongoDBConfig::new(
            vec!["127.0.0.1:27017".to_string(), "".to_string()],
            true,
            300,
            10,
        );

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Empty mongos endpoint not allowed");
    }

    #[test]
    fn test_mongodb_config_validation_invalid_endpoint_format() {
        let result = MongoDBConfig::new(
            vec![
                "127.0.0.1:27017".to_string(),
                "invalid-endpoint".to_string(),
            ],
            true,
            300,
            10,
        );

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Invalid mongos endpoint format 'invalid-endpoint': must be host:port"
        );
    }

    #[test]
    fn test_mongodb_config_validation_zero_session_timeout() {
        let result = MongoDBConfig::new(vec!["127.0.0.1:27017".to_string()], true, 0, 10);

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Session timeout must be greater than 0"
        );
    }

    #[test]
    fn test_mongodb_config_validation_zero_health_check_interval() {
        let result = MongoDBConfig::new(vec!["127.0.0.1:27017".to_string()], true, 300, 0);

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Health check interval must be greater than 0"
        );
    }

    #[tokio::test]
    async fn test_session_affinity_manager_creation() {
        let manager = SessionAffinityManager::new();
        assert_eq!(manager.session_count().await, 0);
    }

    #[tokio::test]
    async fn test_session_affinity_manager_get_backend_for_client() {
        let manager = SessionAffinityManager::new();
        let client_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12345);
        let available_backends = vec!["mongos-0".to_string(), "mongos-1".to_string()];

        // First call should assign a backend
        let backend1 = manager
            .get_backend_for_client(client_addr, &available_backends)
            .await;
        assert!(backend1.is_some());
        let backend1_clone = backend1.clone();
        assert!(available_backends.contains(&backend1.unwrap()));

        // Second call should return the same backend (session affinity)
        let backend2 = manager
            .get_backend_for_client(client_addr, &available_backends)
            .await;
        assert_eq!(backend1_clone, backend2);

        assert_eq!(manager.session_count().await, 1);
    }

    #[tokio::test]
    async fn test_session_affinity_manager_no_available_backends() {
        let manager = SessionAffinityManager::new();
        let client_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12345);
        let available_backends = vec![];

        let backend = manager
            .get_backend_for_client(client_addr, &available_backends)
            .await;
        assert!(backend.is_none());
    }

    #[tokio::test]
    async fn test_session_affinity_manager_backend_unavailable() {
        let manager = SessionAffinityManager::new();
        let client_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12345);

        // Initially assign backend
        let initial_backends = vec!["mongos-0".to_string(), "mongos-1".to_string()];
        let backend1 = manager
            .get_backend_for_client(client_addr, &initial_backends)
            .await;
        assert!(backend1.is_some());

        // Backend becomes unavailable
        let reduced_backends = vec!["mongos-2".to_string()];
        let backend2 = manager
            .get_backend_for_client(client_addr, &reduced_backends)
            .await;

        // Should assign a new backend since the old one is unavailable
        assert!(backend2.is_some());
        assert_eq!(backend2.unwrap(), "mongos-2");
    }

    #[tokio::test]
    async fn test_session_affinity_manager_remove_client_affinity() {
        let manager = SessionAffinityManager::new();
        let client_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12345);
        let available_backends = vec!["mongos-0".to_string()];

        // Assign backend
        let _backend = manager
            .get_backend_for_client(client_addr, &available_backends)
            .await;
        assert_eq!(manager.session_count().await, 1);

        // Remove affinity
        let removed = manager.remove_client_affinity(client_addr).await;
        assert!(removed);
        assert_eq!(manager.session_count().await, 0);

        // Try to remove again (should return false)
        let removed_again = manager.remove_client_affinity(client_addr).await;
        assert!(!removed_again);
    }

    #[tokio::test]
    async fn test_session_affinity_manager_clear_all_sessions() {
        let manager = SessionAffinityManager::new();
        let available_backends = vec!["mongos-0".to_string()];

        // Add multiple sessions
        for i in 0..5 {
            let client_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12345 + i);
            let _backend = manager
                .get_backend_for_client(client_addr, &available_backends)
                .await;
        }

        assert_eq!(manager.session_count().await, 5);

        // Clear all sessions
        manager.clear_all_sessions().await;
        assert_eq!(manager.session_count().await, 0);
    }

    #[tokio::test]
    async fn test_session_affinity_manager_get_active_clients() {
        let manager = SessionAffinityManager::new();
        let available_backends = vec!["mongos-0".to_string()];

        // Add some sessions
        let client_addrs = vec![
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12345),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12346),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12347),
        ];

        for &client_addr in &client_addrs {
            let _backend = manager
                .get_backend_for_client(client_addr, &available_backends)
                .await;
        }

        let active_clients = manager.get_active_clients().await;
        assert_eq!(active_clients.len(), 3);

        for &client_addr in &client_addrs {
            assert!(active_clients.contains(&client_addr));
        }
    }

    #[tokio::test]
    async fn test_mongodb_proxy_creation() {
        let config =
            MongoDBConfig::new(vec!["127.0.0.1:27017".to_string()], true, 300, 10).unwrap();

        let proxy = MongoDBProxy::new(config);
        assert_eq!(proxy.get_config().endpoint_count(), 1);
        assert!(proxy.get_config().session_affinity_enabled);
    }

    #[tokio::test]
    async fn test_mongodb_proxy_route_request_no_healthy_backends() {
        let config =
            MongoDBConfig::new(vec!["127.0.0.1:27017".to_string()], true, 300, 10).unwrap();

        let proxy = MongoDBProxy::new(config);
        let client_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12345);

        // No backends initialized, should return error
        let result = proxy.route_request(client_addr).await;
        match result {
            RoutingDecision::Error { message } => {
                assert_eq!(message, "No healthy mongos instances available");
            }
            _ => panic!("Expected error decision"),
        }
    }

    #[tokio::test]
    async fn test_mongodb_proxy_handle_client_disconnect_affinity_enabled() {
        let config = MongoDBConfig::new(
            vec!["127.0.0.1:27017".to_string()],
            true, // Affinity enabled
            300,
            10,
        )
        .unwrap();

        let proxy = MongoDBProxy::new(config);
        let client_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12345);

        // Add a session manually
        {
            let mut affinity_map = proxy.affinity_manager.client_to_backend.write().await;
            affinity_map.insert(client_addr, "mongos-0".to_string());
        }

        // Handle disconnect
        let removed = proxy.handle_client_disconnect(client_addr).await;
        assert!(removed);
        assert_eq!(proxy.affinity_manager.session_count().await, 0);
    }

    #[tokio::test]
    async fn test_mongodb_proxy_handle_client_disconnect_affinity_disabled() {
        let config = MongoDBConfig::new(
            vec!["127.0.0.1:27017".to_string()],
            false, // Affinity disabled
            300,
            10,
        )
        .unwrap();

        let proxy = MongoDBProxy::new(config);
        let client_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12345);

        // Handle disconnect (should not remove anything since affinity is disabled)
        let removed = proxy.handle_client_disconnect(client_addr).await;
        assert!(!removed);
    }

    #[tokio::test]
    async fn test_mongodb_proxy_with_health_check() {
        let config =
            MongoDBConfig::new(vec!["127.0.0.1:27017".to_string()], true, 300, 10).unwrap();

        let proxy = MongoDBProxy::new(config).with_health_check();
        assert!(proxy.health_manager.is_some());
    }

    #[tokio::test]
    async fn test_mongodb_proxy_start_health_checks() {
        let config =
            MongoDBConfig::new(vec!["127.0.0.1:27017".to_string()], true, 300, 10).unwrap();

        let proxy = MongoDBProxy::new(config).with_health_check();

        // Initialize backends first
        let result = proxy.initialize_backends().await;
        assert!(result.is_ok());

        // Start health checks should succeed
        let result = proxy.start_health_checks().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mongodb_proxy_start_health_checks_without_health_manager() {
        let config =
            MongoDBConfig::new(vec!["127.0.0.1:27017".to_string()], true, 300, 10).unwrap();

        let proxy = MongoDBProxy::new(config); // No health manager

        // Should succeed but do nothing
        let result = proxy.start_health_checks().await;
        assert!(result.is_ok());
    }
}
