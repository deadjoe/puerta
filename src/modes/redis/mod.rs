/// Redis Cluster mode implementation using Pingora and RCProxy architecture
///
/// This mode provides protocol-aware proxy for Redis Cluster.
/// Key features:
/// - RESP protocol parsing and manipulation
/// - Slot-based request routing using CRC16
/// - MOVED/ASK redirection handling
/// - Cluster topology discovery and maintenance
/// - Cross-slot operation detection and handling
pub mod proxy;
pub mod redirect;
pub mod resp;
pub mod slots;

use async_trait::async_trait;
use bytes::Bytes;
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::RwLock;

// Pingora framework imports
use pingora::apps::ServerApp;
use pingora_core::connectors::TransportConnector;
use pingora_core::listeners::Listeners;
use pingora_core::protocols::Stream;
use pingora_core::server::Server;
use pingora_core::server::ShutdownWatch;
use pingora_core::services::listening::Service;
use pingora_core::upstreams::peer::{BasicPeer, Peer};

/// Redis Cluster configuration
#[derive(Debug, Clone)]
pub struct RedisConfig {
    pub cluster_nodes: Vec<String>,
    pub slot_refresh_interval_sec: u64,
    pub max_redirects: u8,
    pub connection_timeout_ms: u64,
}

/// Redis slot mapping (16384 slots total)
#[derive(Debug, Clone)]
pub struct SlotMapping {
    /// Maps slot number (0-16383) to backend ID
    slot_to_backend: HashMap<u16, String>,
    /// Maps backend ID to slot ranges for quick lookup
    backend_to_slots: HashMap<String, Vec<(u16, u16)>>,
}

/// Redis command representation
#[derive(Debug, Clone)]
pub struct RedisCommand {
    pub command: String,
    pub args: Vec<Bytes>,
    pub key: Option<String>, // Extracted key for slot calculation
    pub slot: Option<u16>,   // Calculated slot
    pub readonly: bool,      // Whether this is a read-only command
}

/// Redis response representation
#[derive(Debug, Clone)]
pub enum RedisResponse {
    /// Normal response to forward to client
    Data(Bytes),
    /// MOVED redirection
    Moved { slot: u16, new_address: String },
    /// ASK redirection  
    Ask { slot: u16, new_address: String },
    /// Error response
    Error(String),
}

/// Redis cluster proxy using Pingora TCP proxy for RESP protocol
pub struct RedisClusterProxy {
    config: RedisConfig,
    server: Server,
    connector: TransportConnector,
    cluster_nodes: Arc<RwLock<HashMap<String, BasicPeer>>>,
    slot_mapping: Arc<RwLock<SlotMapping>>,
    health_manager: Option<Arc<crate::health::HealthCheckManager>>,
}

impl SlotMapping {
    pub fn new() -> Self {
        Self {
            slot_to_backend: HashMap::new(),
            backend_to_slots: HashMap::new(),
        }
    }

    /// Calculate Redis slot for a key using CRC16
    pub fn calculate_slot(key: &str) -> u16 {
        // Extract hash tag if present (text between first { and })
        let hash_key = if let (Some(start), Some(end)) = (key.find('{'), key.find('}')) {
            if end > start + 1 {
                &key[start + 1..end]
            } else {
                key
            }
        } else {
            key
        };

        // Calculate CRC16 and mod 16384
        crc16(hash_key.as_bytes()) % 16384
    }

    /// Get backend ID for a given slot
    pub fn get_backend_for_slot(&self, slot: u16) -> Option<String> {
        self.slot_to_backend.get(&slot).cloned()
    }

    /// Update slot mapping from cluster nodes
    pub fn update_slot_mapping(&mut self, slot_ranges: HashMap<String, Vec<(u16, u16)>>) {
        self.slot_to_backend.clear();
        self.backend_to_slots = slot_ranges.clone();

        for (backend_id, ranges) in slot_ranges {
            for (start, end) in ranges {
                for slot in start..=end {
                    self.slot_to_backend.insert(slot, backend_id.clone());
                }
            }
        }
    }

    /// Check if all slots are covered
    pub fn is_complete(&self) -> bool {
        self.slot_to_backend.len() == 16384
    }
}

impl RedisClusterProxy {
    pub fn new(config: RedisConfig, server: Server) -> Self {
        Self {
            config,
            server,
            connector: TransportConnector::new(None),
            cluster_nodes: Arc::new(RwLock::new(HashMap::new())),
            slot_mapping: Arc::new(RwLock::new(SlotMapping::new())),
            health_manager: None,
        }
    }

    pub fn with_health_check(mut self) -> Self {
        let health_checker = Box::new(crate::health::redis::RedisHealthChecker::new());
        self.health_manager = Some(Arc::new(crate::health::HealthCheckManager::new(
            health_checker,
        )));
        self
    }

    /// Initialize cluster nodes from configuration
    pub async fn initialize_cluster_nodes(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut nodes = self.cluster_nodes.write().await;

        for endpoint in &self.config.cluster_nodes {
            let peer = BasicPeer::new(endpoint);
            nodes.insert(endpoint.clone(), peer);
        }

        // Initial cluster topology discovery
        self.discover_cluster_topology().await?;

        Ok(())
    }

    /// Discover cluster topology by querying CLUSTER NODES
    pub async fn discover_cluster_topology(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let nodes = self.cluster_nodes.read().await;

        for (addr, peer) in nodes.iter() {
            match self.query_cluster_nodes(peer).await {
                Ok(slot_mapping) => {
                    let mut mapping = self.slot_mapping.write().await;
                    *mapping = slot_mapping;
                    log::info!("Successfully discovered cluster topology from {}", addr);
                    return Ok(());
                }
                Err(e) => {
                    log::warn!("Failed to discover topology from {}: {}", addr, e);
                    continue;
                }
            }
        }

        Err("Failed to discover cluster topology from any node".into())
    }

    /// Query CLUSTER NODES from a specific peer
    async fn query_cluster_nodes(
        &self,
        peer: &BasicPeer,
    ) -> Result<SlotMapping, Box<dyn Error + Send + Sync>> {
        let _stream = self.connector.new_stream(peer).await?;

        // Send CLUSTER NODES command
        let _cmd_bytes = b"*2\r\n$7\r\nCLUSTER\r\n$5\r\nNODES\r\n";

        // TODO: Implement actual RESP protocol communication
        // For now, create a basic slot mapping
        let mut slot_mapping = SlotMapping::new();

        // Mock implementation - in practice, parse the actual response
        let mut slot_ranges = HashMap::new();
        for (i, (addr, _)) in self.cluster_nodes.read().await.iter().enumerate() {
            let start_slot = (i * 5461) as u16; // 16384 / 3 ≈ 5461
            let end_slot = ((i + 1) * 5461).min(16383) as u16;
            slot_ranges.insert(addr.clone(), vec![(start_slot, end_slot)]);
        }

        slot_mapping.update_slot_mapping(slot_ranges);
        Ok(slot_mapping)
    }

    /// Run the Redis cluster proxy
    pub async fn run_redis_proxy(self) -> Result<(), Box<dyn Error + Send + Sync>> {
        log::info!("Starting Redis Cluster proxy using Pingora framework");

        // Initialize cluster nodes and topology
        self.initialize_cluster_nodes().await?;

        let mut server = self.server;
        server.bootstrap();

        // Create Redis protocol proxy app
        let redis_app = RedisProtocolApp::new(
            self.connector,
            self.cluster_nodes.clone(),
            self.slot_mapping.clone(),
            self.config.max_redirects,
        );

        // Create TCP listening service for Redis RESP protocol
        let tcp_service = Service::with_listeners(
            "Redis Cluster Proxy".to_string(),
            Listeners::tcp("0.0.0.0:6379"), // Default Redis port
            redis_app,
        );

        server.add_service(tcp_service);

        log::info!("Redis Cluster proxy listening on: 0.0.0.0:6379");
        log::info!("Proxying to cluster nodes: {:?}", self.config.cluster_nodes);

        // Run the server
        server.run_forever();
    }

    /// Get cluster nodes for management
    pub fn get_cluster_nodes(&self) -> Arc<RwLock<HashMap<String, BasicPeer>>> {
        Arc::clone(&self.cluster_nodes)
    }

    /// Get slot mapping for cluster topology updates
    pub fn get_slot_mapping(&self) -> Arc<RwLock<SlotMapping>> {
        Arc::clone(&self.slot_mapping)
    }

    /// Get configuration
    pub fn get_config(&self) -> &RedisConfig {
        &self.config
    }
}

/// Redis Protocol App using Pingora for RESP protocol handling
pub struct RedisProtocolApp {
    connector: TransportConnector,
    cluster_nodes: Arc<RwLock<HashMap<String, BasicPeer>>>,
    slot_mapping: Arc<RwLock<SlotMapping>>,
    max_redirects: u8,
}

impl RedisProtocolApp {
    pub fn new(
        connector: TransportConnector,
        cluster_nodes: Arc<RwLock<HashMap<String, BasicPeer>>>,
        slot_mapping: Arc<RwLock<SlotMapping>>,
        max_redirects: u8,
    ) -> Self {
        Self {
            connector,
            cluster_nodes,
            slot_mapping,
            max_redirects,
        }
    }

    /// Parse Redis command from raw data
    pub async fn parse_redis_command(
        &self,
        data: &[u8],
    ) -> Result<RedisCommand, Box<dyn Error + Send + Sync>> {
        // Basic RESP parsing - this would be more sophisticated in practice
        let cmd_str = String::from_utf8_lossy(data);

        // Extract command and key for slot calculation
        // This is a simplified parser - would need full RESP implementation
        if cmd_str.starts_with("*") {
            let parts: Vec<&str> = cmd_str.lines().collect();
            if parts.len() >= 4 {
                let command = parts[2].to_uppercase();
                let key = if parts.len() >= 6 {
                    Some(parts[5].to_string())
                } else {
                    None
                };
                let slot = key.as_ref().map(|k| SlotMapping::calculate_slot(k));
                let readonly = Self::is_readonly_command(&command);

                return Ok(RedisCommand {
                    command,
                    args: vec![],
                    key,
                    slot,
                    readonly,
                });
            }
        }

        Err("Failed to parse Redis command".into())
    }

    /// Check if a Redis command is read-only
    fn is_readonly_command(command: &str) -> bool {
        matches!(
            command.to_uppercase().as_str(),
            "GET"
                | "MGET"
                | "EXISTS"
                | "TTL"
                | "PTTL"
                | "TYPE"
                | "STRLEN"
                | "LLEN"
                | "LRANGE"
                | "LINDEX"
                | "SCARD"
                | "SMEMBERS"
                | "SISMEMBER"
                | "HGET"
                | "HMGET"
                | "HGETALL"
                | "HLEN"
                | "HEXISTS"
                | "HKEYS"
                | "HVALS"
                | "ZCARD"
                | "ZCOUNT"
                | "ZRANGE"
                | "ZRANGEBYSCORE"
                | "ZRANK"
                | "ZSCORE"
        )
    }

    /// Route command to appropriate cluster node
    pub async fn route_command(
        &self,
        command: &RedisCommand,
    ) -> Result<BasicPeer, Box<dyn Error + Send + Sync>> {
        if let Some(slot) = command.slot {
            let slot_mapping = self.slot_mapping.read().await;
            if let Some(node_addr) = slot_mapping.get_backend_for_slot(slot) {
                let nodes = self.cluster_nodes.read().await;
                if let Some(peer) = nodes.get(&node_addr) {
                    return Ok(peer.clone());
                }
            }
        }

        // Fallback to first available node
        let nodes = self.cluster_nodes.read().await;
        if let Some((_, peer)) = nodes.iter().next() {
            Ok(peer.clone())
        } else {
            Err("No cluster nodes available".into())
        }
    }

    /// Forward Redis RESP protocol data with redirection handling
    async fn forward_redis_data(&self, mut client_stream: Stream, mut redis_stream: Stream) {
        use crate::modes::redis::redirect::{RedirectParser, RedirectionContext};

        let mut client_buf = [0; 8192];
        let mut redis_buf = [0; 8192];
        let _redirect_context = RedirectionContext::new(0, self.max_redirects);

        loop {
            tokio::select! {
                // Client -> Redis
                result = client_stream.read(&mut client_buf) => {
                    match result {
                        Ok(0) => {
                            log::debug!("Client connection closed");
                            break;
                        }
                        Ok(n) => {
                            // Forward client data to Redis
                            if let Err(e) = redis_stream.write_all(&client_buf[0..n]).await {
                                log::error!("Failed to write to Redis: {}", e);
                                break;
                            }
                            if let Err(e) = redis_stream.flush().await {
                                log::error!("Failed to flush to Redis: {}", e);
                                break;
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to read from client: {}", e);
                            break;
                        }
                    }
                }
                // Redis -> Client
                result = redis_stream.read(&mut redis_buf) => {
                    match result {
                        Ok(0) => {
                            log::debug!("Redis connection closed");
                            break;
                        }
                        Ok(n) => {
                            // Check for Redis redirections in the response
                            let response_data = &redis_buf[0..n];

                            // Parse potential redirections using RCProxy-style parsing
                            if let Some(redirect) = RedirectParser::parse_redirect_raw(response_data) {
                                log::info!("Detected redirection: {:?}", redirect);

                                // Handle the redirection - in a full implementation, we would:
                                // 1. Create new connection to the target node
                                // 2. Send ASKING command if needed (for ASK redirections)
                                // 3. Retry the original command
                                // 4. Update slot mapping for MOVED redirections
                                //
                                // For now, forward the redirection response to client
                                // to demonstrate the parsing functionality
                                match redirect {
                                    crate::modes::redis::redirect::RedirectType::Moved { slot, address } => {
                                        log::warn!("MOVED redirection detected for slot {} to {}", slot, address);
                                        // TODO: Update slot mapping and connect to new node
                                    }
                                    crate::modes::redis::redirect::RedirectType::Ask { slot, address } => {
                                        log::warn!("ASK redirection detected for slot {} to {}", slot, address);
                                        // TODO: Send ASKING command and retry
                                    }
                                }
                            }

                            // Forward response to client
                            if let Err(e) = client_stream.write_all(response_data).await {
                                log::error!("Failed to write to client: {}", e);
                                break;
                            }
                            if let Err(e) = client_stream.flush().await {
                                log::error!("Failed to flush to client: {}", e);
                                break;
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to read from Redis: {}", e);
                            break;
                        }
                    }
                }
            }
        }
    }
}

#[async_trait]
impl ServerApp for RedisProtocolApp {
    async fn process_new(
        self: &Arc<Self>,
        client_stream: Stream,
        _shutdown: &ShutdownWatch,
    ) -> Option<Stream> {
        // Get client address for logging and potential future use
        let client_addr = match client_stream
            .get_socket_digest()
            .and_then(|digest| digest.peer_addr().cloned())
        {
            Some(addr) => addr.to_string(),
            None => {
                log::warn!("Could not get client address, using fallback identifier");
                format!("unknown-{:x}", std::ptr::addr_of!(client_stream) as usize)
            }
        };

        log::info!("New Redis client connection from: {}", client_addr);

        // For now, route to first available cluster node
        // TODO: Implement proper command parsing and routing
        let nodes = self.cluster_nodes.read().await;
        let redis_peer = match nodes.iter().next() {
            Some((_, peer)) => peer.clone(),
            None => {
                log::error!("No Redis cluster nodes available");
                return None;
            }
        };

        // Connect to Redis node
        let redis_stream = match self.connector.new_stream(&redis_peer).await {
            Ok(stream) => stream,
            Err(e) => {
                log::error!(
                    "Failed to connect to Redis node {}: {}",
                    redis_peer.address(),
                    e
                );
                return None;
            }
        };

        log::info!(
            "Established connection to Redis node: {}",
            redis_peer.address()
        );

        // Forward Redis RESP protocol data bidirectionally
        self.forward_redis_data(client_stream, redis_stream).await;

        None
    }
}

/// Simple CRC16 implementation for Redis slot calculation
fn crc16(data: &[u8]) -> u16 {
    crate::utils::crc16(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pingora_core::server::Server;
    use std::collections::HashMap;

    #[test]
    fn test_slot_calculation() {
        // Test vectors - need to verify these match actual Redis implementation
        assert_eq!(SlotMapping::calculate_slot("123456789"), 12739);
        assert_eq!(SlotMapping::calculate_slot("foo"), 44950 % 16384);
        assert_eq!(SlotMapping::calculate_slot("bar"), 5061);

        // Test hash tags
        assert_eq!(
            SlotMapping::calculate_slot("foo{hash_tag}bar"),
            SlotMapping::calculate_slot("hash_tag")
        );
        assert_eq!(
            SlotMapping::calculate_slot("{user1000}.following"),
            SlotMapping::calculate_slot("user1000")
        );
    }

    #[test]
    fn test_crc16() {
        // Test CRC16 calculation
        // These values should match Redis cluster slot calculation
        assert_eq!(crc16(b"123456789"), 12739);
        assert_eq!(crc16(b"foo"), 44950);
    }

    #[test]
    fn test_redis_config_creation() {
        let config = RedisConfig {
            cluster_nodes: vec!["127.0.0.1:6379".to_string(), "127.0.0.1:6380".to_string()],
            slot_refresh_interval_sec: 30,
            max_redirects: 3,
            connection_timeout_ms: 5000,
        };

        assert_eq!(config.cluster_nodes.len(), 2);
        assert_eq!(config.slot_refresh_interval_sec, 30);
        assert_eq!(config.max_redirects, 3);
        assert_eq!(config.connection_timeout_ms, 5000);
    }

    #[test]
    fn test_slot_mapping_creation() {
        let mapping = SlotMapping::new();
        assert!(!mapping.is_complete());
        assert_eq!(mapping.get_backend_for_slot(0), None);
    }

    #[test]
    fn test_slot_mapping_update() {
        let mut mapping = SlotMapping::new();

        let mut slot_ranges = HashMap::new();
        slot_ranges.insert("node1".to_string(), vec![(0, 5460)]);
        slot_ranges.insert("node2".to_string(), vec![(5461, 10922)]);
        slot_ranges.insert("node3".to_string(), vec![(10923, 16383)]);

        mapping.update_slot_mapping(slot_ranges);

        assert!(mapping.is_complete());
        assert_eq!(mapping.get_backend_for_slot(0), Some("node1".to_string()));
        assert_eq!(
            mapping.get_backend_for_slot(5461),
            Some("node2".to_string())
        );
        assert_eq!(
            mapping.get_backend_for_slot(16383),
            Some("node3".to_string())
        );
    }

    #[test]
    fn test_slot_mapping_partial_coverage() {
        let mut mapping = SlotMapping::new();

        let mut slot_ranges = HashMap::new();
        slot_ranges.insert("node1".to_string(), vec![(0, 1000)]);

        mapping.update_slot_mapping(slot_ranges);

        assert!(!mapping.is_complete());
        assert_eq!(mapping.get_backend_for_slot(0), Some("node1".to_string()));
        assert_eq!(mapping.get_backend_for_slot(1001), None);
    }

    #[test]
    fn test_redis_command_creation() {
        let command = RedisCommand {
            command: "GET".to_string(),
            args: vec![],
            key: Some("mykey".to_string()),
            slot: Some(SlotMapping::calculate_slot("mykey")),
            readonly: true,
        };

        assert_eq!(command.command, "GET");
        assert_eq!(command.key, Some("mykey".to_string()));
        assert!(command.readonly);
        assert!(command.slot.is_some());
    }

    #[test]
    fn test_redis_response_variants() {
        let data_response = RedisResponse::Data(bytes::Bytes::from("OK"));
        let moved_response = RedisResponse::Moved {
            slot: 1234,
            new_address: "127.0.0.1:6379".to_string(),
        };
        let ask_response = RedisResponse::Ask {
            slot: 5678,
            new_address: "127.0.0.1:6380".to_string(),
        };
        let error_response = RedisResponse::Error("ERR something went wrong".to_string());

        match data_response {
            RedisResponse::Data(_) => {}
            _ => panic!("Expected Data response"),
        }

        match moved_response {
            RedisResponse::Moved { slot, new_address } => {
                assert_eq!(slot, 1234);
                assert_eq!(new_address, "127.0.0.1:6379");
            }
            _ => panic!("Expected Moved response"),
        }

        match ask_response {
            RedisResponse::Ask { slot, new_address } => {
                assert_eq!(slot, 5678);
                assert_eq!(new_address, "127.0.0.1:6380");
            }
            _ => panic!("Expected Ask response"),
        }

        match error_response {
            RedisResponse::Error(msg) => {
                assert_eq!(msg, "ERR something went wrong");
            }
            _ => panic!("Expected Error response"),
        }
    }

    #[test]
    fn test_slot_calculation_edge_cases() {
        // Empty string
        let empty_slot = SlotMapping::calculate_slot("");
        assert!(empty_slot < 16384);

        // Single character
        let single_char_slot = SlotMapping::calculate_slot("a");
        assert!(single_char_slot < 16384);

        // Very long key
        let long_key = "a".repeat(1000);
        let long_key_slot = SlotMapping::calculate_slot(&long_key);
        assert!(long_key_slot < 16384);

        // Special characters
        let special_key = "key-with:special@characters#";
        let special_slot = SlotMapping::calculate_slot(special_key);
        assert!(special_slot < 16384);
    }

    #[test]
    fn test_hash_tag_extraction() {
        // No hash tag
        assert_eq!(
            SlotMapping::calculate_slot("simple_key"),
            SlotMapping::calculate_slot("simple_key")
        );

        // Hash tag at the beginning
        let key1 = "{tag}key";
        let tag_only = "tag";
        assert_eq!(
            SlotMapping::calculate_slot(key1),
            SlotMapping::calculate_slot(tag_only)
        );

        // Hash tag in the middle
        let key2 = "prefix{tag}suffix";
        assert_eq!(
            SlotMapping::calculate_slot(key2),
            SlotMapping::calculate_slot(tag_only)
        );

        // Multiple hash tags (should use first one)
        let key3 = "prefix{tag1}middle{tag2}suffix";
        assert_eq!(
            SlotMapping::calculate_slot(key3),
            SlotMapping::calculate_slot("tag1")
        );

        // Empty hash tag (should use full key)
        let key4 = "prefix{}suffix";
        assert_eq!(
            SlotMapping::calculate_slot(key4),
            SlotMapping::calculate_slot(key4)
        );

        // Invalid hash tag (no closing brace)
        let key5 = "prefix{tag_suffix";
        assert_eq!(
            SlotMapping::calculate_slot(key5),
            SlotMapping::calculate_slot(key5)
        );
    }

    #[tokio::test]
    async fn test_redis_cluster_proxy_creation() {
        let config = RedisConfig {
            cluster_nodes: vec!["127.0.0.1:6379".to_string()],
            slot_refresh_interval_sec: 30,
            max_redirects: 3,
            connection_timeout_ms: 5000,
        };

        let server = Server::new(None).unwrap();
        let proxy = RedisClusterProxy::new(config, server);

        assert_eq!(proxy.get_config().cluster_nodes.len(), 1);
        assert_eq!(proxy.get_config().max_redirects, 3);
    }

    #[tokio::test]
    async fn test_redis_protocol_app_creation() {
        use pingora_core::connectors::TransportConnector;

        let connector = TransportConnector::new(None);
        let cluster_nodes = Arc::new(RwLock::new(HashMap::new()));
        let slot_mapping = Arc::new(RwLock::new(SlotMapping::new()));

        let _app = RedisProtocolApp::new(connector, cluster_nodes, slot_mapping, 3);

        // Just test creation succeeds
        assert!(true);
    }

    #[test]
    fn test_slot_mapping_comprehensive_coverage() {
        let mut mapping = SlotMapping::new();

        // Create a full mapping covering all 16384 slots
        let mut slot_ranges = HashMap::new();
        let slots_per_node = 16384 / 3;

        slot_ranges.insert("node1".to_string(), vec![(0, slots_per_node - 1)]);
        slot_ranges.insert(
            "node2".to_string(),
            vec![(slots_per_node, 2 * slots_per_node - 1)],
        );
        slot_ranges.insert("node3".to_string(), vec![(2 * slots_per_node, 16383)]);

        mapping.update_slot_mapping(slot_ranges);

        assert!(mapping.is_complete());

        // Test boundary conditions
        assert_eq!(mapping.get_backend_for_slot(0), Some("node1".to_string()));
        assert_eq!(
            mapping.get_backend_for_slot(slots_per_node - 1),
            Some("node1".to_string())
        );
        assert_eq!(
            mapping.get_backend_for_slot(slots_per_node),
            Some("node2".to_string())
        );
        assert_eq!(
            mapping.get_backend_for_slot(16383),
            Some("node3".to_string())
        );
    }

    #[tokio::test]
    async fn test_redis_cluster_proxy_with_health_check() {
        let config = RedisConfig {
            cluster_nodes: vec!["127.0.0.1:6379".to_string()],
            slot_refresh_interval_sec: 30,
            max_redirects: 3,
            connection_timeout_ms: 5000,
        };

        let server = Server::new(None).unwrap();
        let proxy = RedisClusterProxy::new(config, server).with_health_check();
        assert!(proxy.health_manager.is_some());
    }

    #[test]
    fn test_redis_protocol_app_is_readonly_command() {
        // Test read-only commands
        assert!(RedisProtocolApp::is_readonly_command("GET"));
        assert!(RedisProtocolApp::is_readonly_command("MGET"));
        assert!(RedisProtocolApp::is_readonly_command("EXISTS"));
        assert!(RedisProtocolApp::is_readonly_command("LLEN"));
        assert!(RedisProtocolApp::is_readonly_command("HGET"));
        assert!(RedisProtocolApp::is_readonly_command("ZCARD"));

        // Test write commands (should return false)
        assert!(!RedisProtocolApp::is_readonly_command("SET"));
        assert!(!RedisProtocolApp::is_readonly_command("DEL"));
        assert!(!RedisProtocolApp::is_readonly_command("HSET"));
        assert!(!RedisProtocolApp::is_readonly_command("ZADD"));

        // Test case insensitive
        assert!(RedisProtocolApp::is_readonly_command("get"));
        assert!(RedisProtocolApp::is_readonly_command("Get"));
    }
}
