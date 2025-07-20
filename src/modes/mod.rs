/// Different operational modes for puerta

pub mod mongodb;
pub mod redis;

use crate::core::Backend;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Shared backend pool management
pub type BackendPool = Arc<RwLock<HashMap<String, Backend>>>;

/// Result type for routing decisions
#[derive(Debug)]
pub enum RoutingDecision {
    /// Route to specific backend
    Route { backend_id: String },
    /// Return error to client
    Error { message: String },
    /// Redirect client (Redis MOVED/ASK)
    Redirect { new_address: String, command: String },
}