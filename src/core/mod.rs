/// Core abstractions shared between MongoDB and Redis modes
pub mod backend;
pub mod frontend;
pub mod session;

use std::net::SocketAddr;
use std::time::SystemTime;

/// Represents a backend endpoint (mongos or Redis node)
#[derive(Debug, Clone)]
pub struct Backend {
    pub id: String,
    pub addr: SocketAddr,
    pub weight: usize,
    pub healthy: bool,
    pub last_health_check: Option<SystemTime>,
    pub metadata: BackendMetadata,
}

/// Backend-specific metadata
#[derive(Debug, Clone)]
pub enum BackendMetadata {
    /// MongoDB mongos instance metadata
    MongoDB {
        version: Option<String>,
        is_primary: bool,
        connection_count: usize,
    },
    /// Redis node metadata
    Redis {
        node_id: String,
        slot_ranges: Vec<(u16, u16)>, // (start, end) inclusive
        is_master: bool,
        replication_offset: Option<u64>,
    },
}

/// Frontend connection representation
#[derive(Debug, Clone)]
pub struct Frontend {
    pub id: String,
    pub client_addr: SocketAddr,
    pub connected_at: SystemTime,
    pub backend_affinity: Option<String>, // Backend ID for session affinity
}

impl Backend {
    pub fn new_mongodb(id: String, addr: SocketAddr) -> Self {
        Self {
            id,
            addr,
            weight: 1,
            healthy: false,
            last_health_check: None,
            metadata: BackendMetadata::MongoDB {
                version: None,
                is_primary: false,
                connection_count: 0,
            },
        }
    }

    pub fn new_redis(id: String, addr: SocketAddr, node_id: String) -> Self {
        Self {
            id,
            addr,
            weight: 1,
            healthy: false,
            last_health_check: None,
            metadata: BackendMetadata::Redis {
                node_id,
                slot_ranges: Vec::new(),
                is_master: false,
                replication_offset: None,
            },
        }
    }
}
