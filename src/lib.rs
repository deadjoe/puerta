pub mod config;
pub mod error;
/// Puerta - High-performance load balancer for MongoDB Sharded Clusters and Redis Clusters
/// Built on Cloudflare's Pingora framework and RCProxy architecture
///
/// Puerta supports two distinct operational modes:
/// 1. MongoDB Mode: Session-aware TCP load balancing across multiple mongos instances using Pingora TCP proxy
/// 2. Redis Mode: Protocol-aware proxy for Redis Cluster with MOVED/ASK handling using RCProxy
pub mod core;
pub mod health;
pub mod modes;
pub mod utils;

use async_trait::async_trait;
use std::error::Error;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

// Pingora framework imports for TCP proxy
use pingora::apps::ServerApp;
use pingora_core::connectors::TransportConnector;
use pingora_core::listeners::Listeners;
use pingora_core::protocols::Stream;
use pingora_core::server::configuration::Opt;
use pingora_core::server::Server;
use pingora_core::server::ShutdownWatch;
use pingora_core::services::listening::Service;
use pingora_core::upstreams::peer::{BasicPeer, Peer};
use pingora_load_balancing::{health_check, selection::RoundRobin, LoadBalancer};

use crate::modes::mongodb::MongoDBConfig;
use crate::modes::redis::{RedisClusterProxy, RedisConfig};

/// Main proxy mode enumeration
#[derive(Debug, Clone)]
pub enum ProxyMode {
    /// MongoDB Sharded Cluster mode: Session-aware TCP load balancing across mongos instances
    /// Uses Pingora TCP proxy with session affinity for MongoDB Wire Protocol
    MongoDB {
        mongos_endpoints: Vec<String>,
        session_affinity_enabled: bool,
    },
    /// Redis Cluster mode: Protocol-aware proxy with slot-based routing
    /// Uses RCProxy-style Redis cluster handling
    Redis {
        cluster_nodes: Vec<String>,
        slot_refresh_interval_ms: u64,
    },
}

/// Main puerta configuration
#[derive(Debug, Clone)]
pub struct PuertaConfig {
    pub listen_addr: String,
    pub proxy_mode: ProxyMode,
    pub health_check_interval_ms: u64,
    pub max_connections: usize,
}

impl PuertaConfig {
    /// Create a new Puerta configuration with validation
    pub fn new(
        listen_addr: String,
        proxy_mode: ProxyMode,
        health_check_interval_ms: u64,
        max_connections: usize,
    ) -> Result<Self, String> {
        // Validate listen address
        if listen_addr.trim().is_empty() {
            return Err("Listen address cannot be empty".to_string());
        }

        // Validate health check interval
        if health_check_interval_ms == 0 {
            return Err("Health check interval must be greater than 0".to_string());
        }

        // Validate max connections
        if max_connections == 0 {
            return Err("Max connections must be greater than 0".to_string());
        }

        // Validate proxy mode specific settings
        match &proxy_mode {
            ProxyMode::MongoDB {
                mongos_endpoints, ..
            } => {
                if mongos_endpoints.is_empty() {
                    return Err("At least one mongos endpoint is required".to_string());
                }
            }
            ProxyMode::Redis {
                cluster_nodes,
                slot_refresh_interval_ms,
            } => {
                if cluster_nodes.is_empty() {
                    return Err("At least one Redis cluster node is required".to_string());
                }
                if *slot_refresh_interval_ms == 0 {
                    return Err("Slot refresh interval must be greater than 0".to_string());
                }
            }
        }

        Ok(Self {
            listen_addr,
            proxy_mode,
            health_check_interval_ms,
            max_connections,
        })
    }

    /// Get the proxy mode as a string for logging
    pub fn mode_name(&self) -> &'static str {
        match self.proxy_mode {
            ProxyMode::MongoDB { .. } => "MongoDB",
            ProxyMode::Redis { .. } => "Redis",
        }
    }

    /// Check if the configuration is valid
    pub fn is_valid(&self) -> bool {
        !self.listen_addr.trim().is_empty()
            && self.health_check_interval_ms > 0
            && self.max_connections > 0
    }
}

/// MongoDB TCP Proxy App using Pingora framework for MongoDB Wire Protocol
/// Now uses the structured MongoDBProxy from src/modes/mongodb for routing decisions
pub struct MongoDBTcpProxy {
    connector: TransportConnector,
    load_balancer: Arc<LoadBalancer<RoundRobin>>,
    mongodb_proxy: Arc<crate::modes::mongodb::MongoDBProxy>,
}

impl MongoDBTcpProxy {
    pub async fn new(load_balancer: Arc<LoadBalancer<RoundRobin>>, config: MongoDBConfig) -> Result<Self, Box<dyn std::error::Error>> {
        // Create the structured MongoDB proxy with health checking
        let mongodb_proxy = crate::modes::mongodb::MongoDBProxy::new(config.clone())
            .with_health_check();
        
        // Initialize backends
        mongodb_proxy.initialize_backends().await?;
        
        // Start health checks
        mongodb_proxy.start_health_checks().await?;
        
        Ok(Self {
            connector: TransportConnector::new(None),
            load_balancer,
            mongodb_proxy: Arc::new(mongodb_proxy),
        })
    }

    /// Get the current session count for monitoring
    pub async fn session_count(&self) -> usize {
        self.mongodb_proxy.get_affinity_manager().session_count().await
    }

    /// Select backend mongos using Pingora's load balancer with session affinity
    async fn select_backend(
        &self,
        client_addr: &str,
    ) -> Result<BasicPeer, Box<dyn Error + Send + Sync>> {
        // Parse client address to SocketAddr
        let socket_addr: std::net::SocketAddr = client_addr.parse()
            .map_err(|e| format!("Invalid client address {client_addr}: {e}"))?;
        
        // Check session affinity first  
        if let Some(backend_id) = self.mongodb_proxy
            .get_affinity_manager()
            .get_backend_for_client(socket_addr, &[])
            .await
        {
            // If session affinity exists, try to use that backend
            let backend_pool = self.mongodb_proxy.get_backends();
            let backends = backend_pool.read().await;
            if let Some(backend) = backends.get(&backend_id) {
                if backend.healthy {
                    log::info!("Using session affinity: client {client_addr} -> backend {backend_id}");
                    return Ok(BasicPeer::new(&backend.addr.to_string()));
                }
            }
        }
        
        // No session affinity or backend unhealthy, use Pingora load balancer
        let upstream = self.load_balancer
            .select(client_addr.as_bytes(), 256) // Use client address for consistent hashing
            .ok_or("No healthy backends available")?;
        
        log::info!("Load balancer selected backend: {upstream:?} for client {client_addr}");
        
        // Create session affinity for new connection
        let backend_addr = upstream.addr.to_string();
        let backend_id = format!("mongos-{backend_addr}");
        let available_backends = vec![backend_id.clone()];
        let _ = self.mongodb_proxy
            .get_affinity_manager()
            .get_backend_for_client(socket_addr, &available_backends)
            .await;
        
        Ok(BasicPeer::new(&backend_addr))
    }

    /// Clean up session affinity when client disconnects
    /// Now uses MongoDBProxy's handle_client_disconnect
    async fn cleanup_session(&self, client_addr: &str) {
        // Parse client address to SocketAddr for MongoDBProxy
        if let Ok(socket_addr) = client_addr.parse::<std::net::SocketAddr>() {
            let removed = self.mongodb_proxy.handle_client_disconnect(socket_addr).await;
            if removed {
                log::info!("Cleaned up session affinity for: {}", client_addr);
            }
        } else {
            log::warn!("Invalid client address format for cleanup: {}", client_addr);
        }
    }

    /// Bidirectional TCP data forwarding between MongoDB client and mongos
    async fn forward_tcp_data(
        &self,
        mut client_stream: Stream,
        mut mongos_stream: Stream,
        client_addr: &str,
    ) {
        let mut client_buf = [0; 8192];
        let mut mongos_buf = [0; 8192];
        let mut bytes_transferred_to_mongos = 0u64;
        let mut bytes_transferred_to_client = 0u64;

        log::info!("Starting data forwarding for client: {}", client_addr);

        loop {
            tokio::select! {
                // Client -> Mongos
                result = client_stream.read(&mut client_buf) => {
                    match result {
                        Ok(0) => {
                            log::debug!("Client {} connection closed", client_addr);
                            break;
                        }
                        Ok(n) => {
                            bytes_transferred_to_mongos += n as u64;
                            if let Err(e) = mongos_stream.write_all(&client_buf[0..n]).await {
                                log::error!("Failed to write {n} bytes to mongos for client {client_addr}: {e}");
                                break;
                            }
                            if let Err(e) = mongos_stream.flush().await {
                                log::error!("Failed to flush to mongos for client {client_addr}: {e}");
                                break;
                            }
                            log::trace!("Forwarded {n} bytes from client {client_addr} to mongos");
                        }
                        Err(e) => {
                            log::error!("Failed to read from client {client_addr}: {e}");
                            break;
                        }
                    }
                }
                // Mongos -> Client
                result = mongos_stream.read(&mut mongos_buf) => {
                    match result {
                        Ok(0) => {
                            log::debug!("Mongos connection closed for client {}", client_addr);
                            break;
                        }
                        Ok(n) => {
                            bytes_transferred_to_client += n as u64;
                            if let Err(e) = client_stream.write_all(&mongos_buf[0..n]).await {
                                log::error!("Failed to write {n} bytes to client {client_addr}: {e}");
                                break;
                            }
                            if let Err(e) = client_stream.flush().await {
                                log::error!("Failed to flush to client {client_addr}: {e}");
                                break;
                            }
                            log::trace!("Forwarded {n} bytes from mongos to client {client_addr}");
                        }
                        Err(e) => {
                            log::error!("Failed to read from mongos for client {client_addr}: {e}");
                            break;
                        }
                    }
                }
            }
        }

        log::info!(
            "Data forwarding completed for client {client_addr}: {bytes_transferred_to_mongos} bytes to mongos, {bytes_transferred_to_client} bytes to client"
        );
    }
}

#[async_trait]
impl ServerApp for MongoDBTcpProxy {
    async fn process_new(
        self: &Arc<Self>,
        client_stream: Stream,
        _shutdown: &ShutdownWatch,
    ) -> Option<Stream> {
        // Get client address for session affinity
        let client_addr = match client_stream
            .get_socket_digest()
            .and_then(|digest| digest.peer_addr().cloned())
        {
            Some(addr) => addr.to_string(),
            None => {
                log::warn!("Could not get client address, using fallback identifier");
                // Generate a unique identifier for this connection
                format!("unknown-{:x}", std::ptr::addr_of!(client_stream) as usize)
            }
        };

        log::info!("New MongoDB client connection from: {}", client_addr);

        // Select backend mongos
        let backend_peer = match self.select_backend(&client_addr).await {
            Ok(peer) => peer,
            Err(e) => {
                log::error!("Failed to select backend: {e}");
                return None;
            }
        };

        // Connect to mongos
        let mongos_stream = match self.connector.new_stream(&backend_peer).await {
            Ok(stream) => stream,
            Err(e) => {
                log::error!(
                    "Failed to connect to mongos {}: {}",
                    backend_peer.address(),
                    e
                );
                self.cleanup_session(&client_addr).await;
                return None;
            }
        };

        log::info!(
            "Established connection to mongos: {}",
            backend_peer.address()
        );

        // Forward MongoDB Wire Protocol data bidirectionally
        self.forward_tcp_data(client_stream, mongos_stream, &client_addr)
            .await;

        // Clean up session affinity
        self.cleanup_session(&client_addr).await;

        None
    }
}

/// Main puerta instance using Pingora framework for TCP proxying
pub struct Puerta {
    config: PuertaConfig,
    server: Option<Server>,
}

impl Puerta {
    pub fn new(config: PuertaConfig) -> Self {
        Self {
            config,
            server: None,
        }
    }

    /// Get the current configuration
    pub fn config(&self) -> &PuertaConfig {
        &self.config
    }

    /// Initialize Pingora server with daemon support
    pub fn initialize(
        &mut self, 
        opt: Option<Opt>, 
        pid_file: std::path::PathBuf, 
        error_log: Option<std::path::PathBuf>,
        upgrade_sock: std::path::PathBuf
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Create server configuration if daemon mode is enabled
        let server = if let Some(opt) = opt {
            if opt.daemon {
                // Create a basic server configuration for daemon mode
                let mut server_conf = pingora_core::server::configuration::ServerConf::default();
                server_conf.daemon = true;
                server_conf.pid_file = pid_file.to_string_lossy().to_string();
                server_conf.upgrade_sock = upgrade_sock.to_string_lossy().to_string();
                if let Some(log_path) = error_log {
                    server_conf.error_log = Some(log_path.to_string_lossy().to_string());
                }
                
                // Create server with configuration
                Server::new_with_opt_and_conf(Some(opt), server_conf)
            } else {
                Server::new(Some(opt))?
            }
        } else {
            Server::new(None)?
        };
        
        self.server = Some(server);
        Ok(())
    }

    /// Check if the server is initialized
    pub fn is_initialized(&self) -> bool {
        self.server.is_some()
    }

    pub fn run(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        if self.server.is_none() {
            return Err("Server not initialized. Call initialize() first.".into());
        }

        match &self.config.proxy_mode {
            ProxyMode::MongoDB { .. } => self.run_mongodb_mode(),
            ProxyMode::Redis { .. } => self.run_redis_mode(),
        }
    }

    fn run_mongodb_mode(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        log::info!("Starting Puerta in MongoDB TCP proxy mode using Pingora framework");

        let mut server = self.server.take().unwrap();
        server.bootstrap();

        // Extract MongoDB configuration
        let (mongos_endpoints, session_affinity_enabled) = match &self.config.proxy_mode {
            ProxyMode::MongoDB {
                mongos_endpoints,
                session_affinity_enabled,
            } => (mongos_endpoints.clone(), *session_affinity_enabled),
            _ => unreachable!("run_mongodb_mode called with non-MongoDB config"),
        };

        // Create MongoDB configuration
        let mongodb_config = MongoDBConfig::new(
            mongos_endpoints.clone(),
            session_affinity_enabled,
            300,
            self.config.health_check_interval_ms / 1000,
        )
        .map_err(|e| format!("Invalid MongoDB configuration: {e}"))?;

        // Create Pingora load balancer with mongos endpoints
        let mut upstreams =
            LoadBalancer::try_from_iter(mongos_endpoints.iter().map(|s| s.as_str()))?;

        // Add health check for mongos instances using TCP health check
        let health_checker = health_check::TcpHealthCheck::new();
        upstreams.set_health_check(health_checker);
        upstreams.health_check_frequency = Some(std::time::Duration::from_millis(
            self.config.health_check_interval_ms,
        ));

        // Create background health check service
        let background = pingora_core::services::background::background_service(
            "mongodb-health-check",
            upstreams,
        );
        let load_balancer = background.task();

        // Create MongoDB TCP proxy service
        let mongodb_proxy = futures::executor::block_on(MongoDBTcpProxy::new(load_balancer, mongodb_config))
            .map_err(|e| format!("Failed to create MongoDB proxy: {e}"))?;

        // Create TCP listening service for MongoDB Wire Protocol
        let tcp_service = Service::with_listeners(
            "MongoDB TCP Proxy".to_string(),
            Listeners::tcp(&self.config.listen_addr),
            mongodb_proxy,
        );

        // Add services to server
        server.add_service(tcp_service);
        server.add_service(background);

        log::info!(
            "MongoDB TCP proxy listening on: {}",
            self.config.listen_addr
        );
        log::info!("Proxying to mongos endpoints: {mongos_endpoints:?}");

        // Run the server (consume ownership)
        server.run_forever();
    }

    fn run_redis_mode(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        log::info!("Starting Puerta in Redis mode using RCProxy architecture");

        // Extract Redis configuration
        let (cluster_nodes, slot_refresh_interval_ms) = match &self.config.proxy_mode {
            ProxyMode::Redis {
                cluster_nodes,
                slot_refresh_interval_ms,
            } => (cluster_nodes.clone(), *slot_refresh_interval_ms),
            _ => unreachable!("run_redis_mode called with non-Redis config"),
        };

        // Create Redis configuration
        let redis_config = RedisConfig {
            cluster_nodes,
            slot_refresh_interval_sec: slot_refresh_interval_ms / 1000,
            max_redirects: 3,
            connection_timeout_ms: 5000,
        };

        let server = self.server.take().unwrap();
        let redis_proxy = RedisClusterProxy::new(redis_config, server).with_health_check();
        futures::executor::block_on(redis_proxy.run_redis_proxy())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_puerta_config_creation() {
        let config = PuertaConfig::new(
            "127.0.0.1:8080".to_string(),
            ProxyMode::MongoDB {
                mongos_endpoints: vec!["127.0.0.1:27017".to_string()],
                session_affinity_enabled: true,
            },
            1000,
            1000,
        )
        .unwrap();

        assert_eq!(config.listen_addr, "127.0.0.1:8080");
        assert_eq!(config.health_check_interval_ms, 1000);
        assert_eq!(config.max_connections, 1000);
        assert_eq!(config.mode_name(), "MongoDB");
        assert!(config.is_valid());
    }

    #[test]
    fn test_puerta_config_validation_empty_listen_addr() {
        let result = PuertaConfig::new(
            "".to_string(),
            ProxyMode::MongoDB {
                mongos_endpoints: vec!["127.0.0.1:27017".to_string()],
                session_affinity_enabled: true,
            },
            1000,
            1000,
        );

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Listen address cannot be empty");
    }

    #[test]
    fn test_puerta_config_validation_zero_health_check_interval() {
        let result = PuertaConfig::new(
            "127.0.0.1:8080".to_string(),
            ProxyMode::MongoDB {
                mongos_endpoints: vec!["127.0.0.1:27017".to_string()],
                session_affinity_enabled: true,
            },
            0,
            1000,
        );

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Health check interval must be greater than 0"
        );
    }

    #[test]
    fn test_puerta_config_validation_zero_max_connections() {
        let result = PuertaConfig::new(
            "127.0.0.1:8080".to_string(),
            ProxyMode::MongoDB {
                mongos_endpoints: vec!["127.0.0.1:27017".to_string()],
                session_affinity_enabled: true,
            },
            1000,
            0,
        );

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Max connections must be greater than 0"
        );
    }

    #[test]
    fn test_puerta_config_validation_empty_mongos_endpoints() {
        let result = PuertaConfig::new(
            "127.0.0.1:8080".to_string(),
            ProxyMode::MongoDB {
                mongos_endpoints: vec![],
                session_affinity_enabled: true,
            },
            1000,
            1000,
        );

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "At least one mongos endpoint is required"
        );
    }

    #[test]
    fn test_puerta_config_validation_empty_redis_nodes() {
        let result = PuertaConfig::new(
            "127.0.0.1:8080".to_string(),
            ProxyMode::Redis {
                cluster_nodes: vec![],
                slot_refresh_interval_ms: 30000,
            },
            1000,
            1000,
        );

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "At least one Redis cluster node is required"
        );
    }

    #[test]
    fn test_puerta_config_validation_zero_slot_refresh_interval() {
        let result = PuertaConfig::new(
            "127.0.0.1:8080".to_string(),
            ProxyMode::Redis {
                cluster_nodes: vec!["127.0.0.1:6379".to_string()],
                slot_refresh_interval_ms: 0,
            },
            1000,
            1000,
        );

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Slot refresh interval must be greater than 0"
        );
    }

    #[test]
    fn test_redis_config_mode_name() {
        let config = PuertaConfig::new(
            "127.0.0.1:8080".to_string(),
            ProxyMode::Redis {
                cluster_nodes: vec!["127.0.0.1:6379".to_string()],
                slot_refresh_interval_ms: 30000,
            },
            1000,
            1000,
        )
        .unwrap();

        assert_eq!(config.mode_name(), "Redis");
    }

    #[test]
    fn test_puerta_creation() {
        let config = PuertaConfig::new(
            "127.0.0.1:8080".to_string(),
            ProxyMode::MongoDB {
                mongos_endpoints: vec!["127.0.0.1:27017".to_string()],
                session_affinity_enabled: true,
            },
            1000,
            1000,
        )
        .unwrap();

        let puerta = Puerta::new(config);
        assert!(!puerta.is_initialized());
        assert_eq!(puerta.config().listen_addr, "127.0.0.1:8080");
    }

    #[test]
    fn test_run_without_initialization() {
        let config = PuertaConfig::new(
            "127.0.0.1:8080".to_string(),
            ProxyMode::MongoDB {
                mongos_endpoints: vec!["127.0.0.1:27017".to_string()],
                session_affinity_enabled: true,
            },
            1000,
            1000,
        )
        .unwrap();

        let mut puerta = Puerta::new(config);

        // The run method is synchronous, no need for runtime
        let result = puerta.run();

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Server not initialized. Call initialize() first."
        );
    }

    #[tokio::test]
    async fn test_mongodb_tcp_proxy_session_count() {
        // Create a mock load balancer and config for testing
        let upstreams = LoadBalancer::try_from_iter(["127.0.0.1:27017"].iter()).unwrap();
        let load_balancer = Arc::new(upstreams);

        let config = MongoDBConfig {
            mongos_endpoints: vec!["127.0.0.1:27017".to_string()],
            session_affinity_enabled: true,
            session_timeout_sec: 300,
            health_check_interval_sec: 10,
        };

        let proxy = MongoDBTcpProxy::new(load_balancer, config).await.unwrap();

        // Initially should have 0 sessions
        assert_eq!(proxy.session_count().await, 0);

        // Test session creation through routing
        let client_addr: std::net::SocketAddr = "127.0.0.1:12345".parse().unwrap();
        let backends = vec!["backend1".to_string()];
        
        // Simulate session creation
        let _backend = proxy.mongodb_proxy.affinity_manager
            .get_backend_for_client(client_addr, &backends)
            .await;

        assert_eq!(proxy.session_count().await, 1);
    }

    #[tokio::test]
    async fn test_mongodb_tcp_proxy_cleanup_session() {
        let upstreams = LoadBalancer::try_from_iter(["127.0.0.1:27017"].iter()).unwrap();
        let load_balancer = Arc::new(upstreams);

        let config = MongoDBConfig {
            mongos_endpoints: vec!["127.0.0.1:27017".to_string()],
            session_affinity_enabled: true,
            session_timeout_sec: 300,
            health_check_interval_sec: 10,
        };

        let proxy = MongoDBTcpProxy::new(load_balancer, config).await.unwrap();
        let client_addr: std::net::SocketAddr = "127.0.0.1:12345".parse().unwrap();
        let backends = vec!["backend1".to_string()];

        // Create a session first
        let _backend = proxy.mongodb_proxy.affinity_manager
            .get_backend_for_client(client_addr, &backends)
            .await;

        assert_eq!(proxy.session_count().await, 1);

        // Clean up the session using the correct API
        proxy.mongodb_proxy.affinity_manager
            .remove_client_affinity(client_addr)
            .await;

        assert_eq!(proxy.session_count().await, 0);
    }

    // TODO: Fix MongoDB test - currently has compilation errors due to refactoring
    // #[tokio::test]
    // async fn test_mongodb_tcp_proxy_cleanup_session_disabled() {
    //     let upstreams = LoadBalancer::try_from_iter(["127.0.0.1:27017"].iter()).unwrap();
    //     let load_balancer = Arc::new(upstreams);
    //
    //     let config = MongoDBConfig::new(
    //         vec!["127.0.0.1:27017".to_string()],
    //         false, // Disabled
    //         300,
    //         10,
    //     )
    //     .unwrap();
    //
    //     let proxy = MongoDBTcpProxy::new(load_balancer, config);
    //     let client_addr = "127.0.0.1:12345";
    //
    //     // Add a session manually (even though affinity is disabled)
    //     {
    //         let mut sessions = proxy.session_affinity.write().await;
    //         sessions.insert(client_addr.to_string(), "127.0.0.1:27017".to_string());
    //     }
    //
    //     assert_eq!(proxy.session_count().await, 1);
    //
    //     // Cleanup should not remove session when affinity is disabled
    //     proxy.cleanup_session(client_addr).await;
    //
    //     // Session should still be there since affinity is disabled
    //     assert_eq!(proxy.session_count().await, 1);
    // }

    #[test]
    fn test_puerta_initialization() {
        let config = PuertaConfig::new(
            "127.0.0.1:8080".to_string(),
            ProxyMode::MongoDB {
                mongos_endpoints: vec!["127.0.0.1:27017".to_string()],
                session_affinity_enabled: true,
            },
            1000,
            1000,
        )
        .unwrap();

        let mut puerta = Puerta::new(config);
        assert!(!puerta.is_initialized());

        let result = puerta.initialize(None);
        assert!(result.is_ok());
        assert!(puerta.is_initialized());
    }
}
