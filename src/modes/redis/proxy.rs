/// Redis Cluster proxy implementation
use super::{RedisCommand, RedisResponse, SlotMapping};
use crate::core::Backend;
use crate::modes::RoutingDecision;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Redis cluster proxy handler
pub struct RedisClusterProxy {
    backends: Arc<RwLock<HashMap<String, Backend>>>,
    slot_mapping: Arc<RwLock<SlotMapping>>,
    max_redirects: u8,
}

impl RedisClusterProxy {
    pub fn new(max_redirects: u8) -> Self {
        Self {
            backends: Arc::new(RwLock::new(HashMap::new())),
            slot_mapping: Arc::new(RwLock::new(SlotMapping::new())),
            max_redirects,
        }
    }

    /// Route a Redis command to the appropriate node
    pub async fn route_command(&self, command: &RedisCommand) -> RoutingDecision {
        // Handle keyless commands
        if command.key.is_none() {
            return self.route_keyless_command(command).await;
        }

        // Calculate slot for the key
        let slot = command.slot.unwrap_or_else(|| {
            let key = command.key.as_ref().unwrap();
            SlotMapping::calculate_slot(key)
        });

        // Find backend for this slot
        let slot_mapping = self.slot_mapping.read().await;
        match slot_mapping.get_backend_for_slot(slot) {
            Some(backend_id) => {
                // Verify backend is healthy
                let backends = self.backends.read().await;
                if let Some(backend) = backends.get(&backend_id) {
                    if backend.healthy {
                        RoutingDecision::Route { backend_id }
                    } else {
                        RoutingDecision::Error {
                            message: format!(
                                "Backend {} for slot {} is unhealthy",
                                backend_id, slot
                            ),
                        }
                    }
                } else {
                    RoutingDecision::Error {
                        message: format!("Backend {} for slot {} not found", backend_id, slot),
                    }
                }
            }
            None => RoutingDecision::Error {
                message: format!("No backend found for slot {}", slot),
            },
        }
    }

    /// Route commands that don't have keys (like PING, INFO, etc.)
    async fn route_keyless_command(&self, _command: &RedisCommand) -> RoutingDecision {
        let backends = self.backends.read().await;
        let healthy_backends: Vec<String> = backends
            .values()
            .filter(|b| b.healthy)
            .map(|b| b.id.clone())
            .collect();

        if healthy_backends.is_empty() {
            RoutingDecision::Error {
                message: "No healthy Redis nodes available".to_string(),
            }
        } else {
            // Route to first healthy backend
            // TODO: Implement better selection algorithm
            RoutingDecision::Route {
                backend_id: healthy_backends[0].clone(),
            }
        }
    }

    /// Handle MOVED/ASK redirection responses
    pub async fn handle_redirection(
        &self,
        _original_command: &RedisCommand,
        response: &RedisResponse,
        redirect_count: u8,
    ) -> RoutingDecision {
        if redirect_count >= self.max_redirects {
            return RoutingDecision::Error {
                message: format!("Too many redirects ({})", redirect_count),
            };
        }

        match response {
            RedisResponse::Moved { slot, new_address } => {
                // MOVED means the slot has permanently moved
                // Update our slot mapping
                self.update_slot_mapping(*slot, new_address.clone()).await;

                // Find backend for the new address
                if let Some(backend_id) = self.find_backend_by_address(new_address).await {
                    RoutingDecision::Route { backend_id }
                } else {
                    RoutingDecision::Error {
                        message: format!(
                            "Backend not found for redirected address: {}",
                            new_address
                        ),
                    }
                }
            }
            RedisResponse::Ask {
                slot: _,
                new_address,
            } => {
                // ASK is temporary - don't update slot mapping
                if let Some(backend_id) = self.find_backend_by_address(new_address).await {
                    RoutingDecision::Route { backend_id }
                } else {
                    RoutingDecision::Error {
                        message: format!("Backend not found for ASK address: {}", new_address),
                    }
                }
            }
            _ => RoutingDecision::Error {
                message: "Invalid redirection response".to_string(),
            },
        }
    }

    /// Update slot mapping for a MOVED response
    async fn update_slot_mapping(&self, slot: u16, new_address: String) {
        // Find the backend ID for this address
        if let Some(backend_id) = self.find_backend_by_address(&new_address).await {
            let _slot_mapping = self.slot_mapping.write().await;
            // Update the slot mapping
            // TODO: Implement proper slot mapping update
            tracing::info!(
                "Slot {} moved to backend {} ({})",
                slot,
                backend_id,
                new_address
            );
        }
    }

    /// Find backend by address
    async fn find_backend_by_address(&self, address: &str) -> Option<String> {
        let backends = self.backends.read().await;
        for (backend_id, backend) in backends.iter() {
            if backend.addr.to_string() == address {
                return Some(backend_id.clone());
            }
        }
        None
    }

    /// Add a backend to the proxy
    pub async fn add_backend(&self, backend: Backend) {
        let mut backends = self.backends.write().await;
        backends.insert(backend.id.clone(), backend);
    }

    /// Remove a backend from the proxy
    pub async fn remove_backend(&self, backend_id: &str) -> Option<Backend> {
        let mut backends = self.backends.write().await;
        backends.remove(backend_id)
    }

    /// Update the cluster topology from CLUSTER NODES response
    pub async fn update_cluster_topology(&self, cluster_nodes_response: &str) {
        // Parse CLUSTER NODES response and update slot mapping
        // TODO: Implement cluster nodes parsing
        tracing::debug!("Updating cluster topology: {}", cluster_nodes_response);
    }

    /// Get backend pool reference
    pub fn get_backends(&self) -> Arc<RwLock<HashMap<String, Backend>>> {
        Arc::clone(&self.backends)
    }

    /// Get slot mapping reference
    pub fn get_slot_mapping(&self) -> Arc<RwLock<SlotMapping>> {
        Arc::clone(&self.slot_mapping)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Backend;
    use std::net::SocketAddr;

    #[tokio::test]
    async fn test_keyless_command_routing() {
        let proxy = RedisClusterProxy::new(3);

        // Add a healthy backend
        let backend = Backend::new_redis(
            "redis-1".to_string(),
            "127.0.0.1:6379".parse::<SocketAddr>().unwrap(),
            "node-1".to_string(),
        );
        proxy.add_backend(backend).await;

        // Mark backend as healthy
        {
            let mut backends = proxy.backends.write().await;
            if let Some(backend) = backends.get_mut("redis-1") {
                backend.healthy = true;
            }
        }

        let command = RedisCommand {
            command: "PING".to_string(),
            args: vec![],
            key: None,
            slot: None,
            readonly: true,
        };

        let result = proxy.route_command(&command).await;
        match result {
            RoutingDecision::Route { backend_id } => {
                assert_eq!(backend_id, "redis-1");
            }
            _ => panic!("Expected route decision"),
        }
    }

    #[tokio::test]
    async fn test_no_healthy_backends() {
        let proxy = RedisClusterProxy::new(3);

        let command = RedisCommand {
            command: "PING".to_string(),
            args: vec![],
            key: None,
            slot: None,
            readonly: true,
        };

        let result = proxy.route_command(&command).await;
        match result {
            RoutingDecision::Error { message } => {
                assert!(message.contains("No healthy Redis nodes"));
            }
            _ => panic!("Expected error decision"),
        }
    }
}
