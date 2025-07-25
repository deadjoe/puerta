# Puerta API参考文档

## 目录

1. [核心模块](#核心模块)
2. [MongoDB代理模块](#mongodb代理模块)
3. [Redis代理模块](#redis代理模块)
4. [健康检查模块](#健康检查模块)
5. [配置模块](#配置模块)
6. [错误处理](#错误处理)

## 核心模块

### Backend

表示后端服务实例的核心数据结构。

```rust
pub struct Backend {
    pub id: String,
    pub addr: SocketAddr,
    pub weight: usize,
    pub healthy: bool,
    pub last_health_check: Option<SystemTime>,
    pub metadata: BackendMetadata,
}
```

#### 字段说明

- `id`: 后端唯一标识符
- `addr`: 后端服务地址
- `weight`: 负载均衡权重
- `healthy`: 健康状态标志
- `last_health_check`: 最后健康检查时间
- `metadata`: 后端特定元数据

### BackendMetadata

后端服务的特定元数据信息。

```rust
pub enum BackendMetadata {
    MongoDB {
        version: Option<String>,
        is_primary: bool,
        connection_count: usize,
    },
    Redis {
        node_id: String,
        slot_ranges: Vec<(u16, u16)>,
        is_master: bool,
        replication_offset: Option<u64>,
    },
}
```

### BackendManager

后端服务管理器，提供后端的增删改查功能。

```rust
pub struct BackendManager {
    backends: BackendPool,
}

impl BackendManager {
    pub fn new() -> Self
    pub async fn add_backend(&self, backend: Backend)
    pub async fn remove_backend(&self, backend_id: &str) -> Option<Backend>
    pub async fn get_backend(&self, backend_id: &str) -> Option<Backend>
    pub async fn get_healthy_backends(&self) -> Vec<Backend>
    pub fn get_pool(&self) -> BackendPool
}
```

#### 方法说明

- `new()`: 创建新的后端管理器
- `add_backend()`: 添加后端服务
- `remove_backend()`: 移除后端服务
- `get_backend()`: 获取指定后端
- `get_healthy_backends()`: 获取所有健康的后端
- `get_pool()`: 获取后端池引用

## MongoDB代理模块

### MongoDBProxy

MongoDB代理的核心结构，提供会话亲和性和负载均衡功能。

```rust
pub struct MongoDBProxy {
    pub config: MongoDBConfig,
    pub backend_manager: Arc<BackendManager>,
    pub affinity_manager: AffinityManager,
    pub health_manager: Option<Arc<HealthCheckManager>>,
}

impl MongoDBProxy {
    pub fn new(config: MongoDBConfig) -> Self
    pub fn with_health_check(mut self) -> Self
    pub async fn initialize_backends(&self) -> Result<(), PuertaError>
    pub async fn start_health_checks(&self) -> Result<(), PuertaError>
    pub async fn route_request(&self, client_addr: SocketAddr) -> RoutingDecision
}
```

#### 方法说明

- `new()`: 创建MongoDB代理实例
- `with_health_check()`: 启用健康检查功能
- `initialize_backends()`: 初始化后端服务
- `start_health_checks()`: 启动健康检查
- `route_request()`: 路由客户端请求

### AffinityManager

会话亲和性管理器，确保客户端连接的一致性路由。

```rust
pub struct AffinityManager {
    pub session_timeout: Duration,
    sessions: Arc<RwLock<HashMap<ClientIdentifier, AffinitySession>>>,
    identification_strategy: ClientIdentificationStrategy,
}

impl AffinityManager {
    pub fn new(session_timeout: Duration) -> Self
    pub async fn get_backend_for_client(
        &self,
        client_addr: SocketAddr,
        available_backends: &[String],
        selection_fn: impl FnOnce(&[String]) -> Option<String>,
        connection_data: Option<&[u8]>,
    ) -> Option<String>
    pub async fn remove_session(&self, client_addr: SocketAddr)
    pub async fn cleanup_expired_sessions(&self) -> usize
    pub async fn session_count(&self) -> usize
}
```

#### 方法说明

- `new()`: 创建亲和性管理器
- `get_backend_for_client()`: 为客户端获取或分配后端
- `remove_session()`: 移除客户端会话
- `cleanup_expired_sessions()`: 清理过期会话
- `session_count()`: 获取当前会话数量

### ClientIdentifier

客户端标识符，支持多种识别策略。

```rust
pub enum ClientIdentifier {
    SocketAddr(SocketAddr),
    Fingerprint(String),
    SessionId(String),
    Hybrid { addr: SocketAddr, fingerprint: String },
}
```

### ClientIdentificationStrategy

客户端识别策略枚举。

```rust
pub enum ClientIdentificationStrategy {
    SocketAddr,
    ConnectionFingerprint,
    SessionId,
    Hybrid,
}
```

## Redis代理模块

### RedisClusterProxy

Redis集群代理的核心结构。

```rust
pub struct RedisClusterProxy {
    config: RedisConfig,
    server: Server,
    connector: TransportConnector,
    cluster_nodes: Arc<RwLock<HashMap<String, BasicPeer>>>,
    slot_mapping: Arc<RwLock<SlotMapping>>,
    health_manager: Option<Arc<HealthCheckManager>>,
}
```

### SlotMapping

Redis集群槽位映射管理。

```rust
pub struct SlotMapping {
    slots: [Option<String>; 16384], // Redis集群有16384个槽位
}

impl SlotMapping {
    pub fn new() -> Self
    pub fn update_slot(&mut self, slot: u16, node_id: String)
    pub fn get_node_for_slot(&self, slot: u16) -> Option<&String>
    pub fn calculate_slot(key: &[u8]) -> u16
}
```

## 健康检查模块

### HealthCheckManager

健康检查管理器，统一管理后端健康状态。

```rust
pub struct HealthCheckManager {
    checker: Box<dyn HealthChecker>,
}

impl HealthCheckManager {
    pub fn new(checker: Box<dyn HealthChecker>) -> Self
    pub async fn check_backend_health(&self, backend: &mut Backend) -> HealthStatus
}
```

### HealthChecker

健康检查器trait，定义健康检查接口。

```rust
#[async_trait]
pub trait HealthChecker: Send + Sync {
    async fn check_health(&self, backend: &Backend) -> HealthStatus;
    fn check_timeout(&self) -> Duration;
    fn check_interval(&self) -> Duration;
}
```

### HealthStatus

健康状态枚举。

```rust
pub enum HealthStatus {
    Healthy,
    Unhealthy { reason: String },
    Timeout,
    Unknown,
}

impl HealthStatus {
    pub fn is_healthy(&self) -> bool
}
```

### MongoDBHealthChecker

MongoDB特定的健康检查器。

```rust
pub struct MongoDBHealthChecker {
    timeout: Duration,
    retry_count: usize,
    retry_delay: Duration,
}

impl MongoDBHealthChecker {
    pub fn new() -> Self
    pub fn with_timeout(mut self, timeout: Duration) -> Self
    pub fn with_retry(mut self, count: usize, delay: Duration) -> Self
}
```

### RedisHealthChecker

Redis特定的健康检查器。

```rust
pub struct RedisHealthChecker {
    timeout: Duration,
    retry_count: usize,
    retry_delay: Duration,
}

impl RedisHealthChecker {
    pub fn new() -> Self
    pub fn with_timeout(mut self, timeout: Duration) -> Self
    pub fn with_retry(mut self, count: usize, delay: Duration) -> Self
}
```

## 配置模块

### Config

主配置结构，包含所有子系统配置。

```rust
pub struct Config {
    pub server: ServerConfig,
    pub proxy: ProxyConfig,
}

impl Config {
    pub fn load_from_file(path: &Path) -> Result<Self, ConfigError>
    pub fn validate(&self) -> Result<(), ConfigError>
}
```

### ServerConfig

服务器配置。

```rust
pub struct ServerConfig {
    pub listen_addr: String,
    pub max_connections: usize,
    pub worker_threads: Option<usize>,
}
```

### ProxyConfig

代理配置枚举。

```rust
pub enum ProxyConfig {
    MongoDB(MongoDBConfig),
    Redis(RedisConfig),
}
```

### MongoDBConfig

MongoDB代理配置。

```rust
pub struct MongoDBConfig {
    pub mongos_endpoints: Vec<String>,
    pub session_affinity_enabled: bool,
    pub session_timeout_sec: u64,
    pub health_check_interval_sec: u64,
}

impl Default for MongoDBConfig {
    fn default() -> Self
}
```

### RedisConfig

Redis代理配置。

```rust
pub struct RedisConfig {
    pub cluster_endpoints: Vec<String>,
    pub max_redirections: usize,
    pub connection_timeout_ms: u64,
    pub health_check_interval_sec: u64,
}

impl Default for RedisConfig {
    fn default() -> Self
}
```

## 错误处理

### PuertaError

主错误类型，包含所有子系统错误。

```rust
pub enum PuertaError {
    Config(ConfigError),
    MongoDB(MongoDBError),
    Redis(RedisError),
    Health(HealthError),
    Network(NetworkError),
}

impl PuertaError {
    pub fn severity(&self) -> ErrorSeverity
    pub fn is_recoverable(&self) -> bool
}
```

### ConfigError

配置相关错误。

```rust
pub enum ConfigError {
    IoError(String),
    ParseError(String),
    ValidationError(String),
    SerializeError(String),
}
```

### MongoDBError

MongoDB代理相关错误。

```rust
pub enum MongoDBError {
    ConnectionFailed { backend_id: String, source: std::io::Error },
    BackendNotFound { backend_id: String },
    AffinityDisabled,
    SessionExpired { client_addr: SocketAddr },
}
```

### RedisError

Redis代理相关错误。

```rust
pub enum RedisError {
    ClusterNodeNotFound { node_id: String },
    SlotMappingError { slot: u16 },
    RedirectionLimitExceeded { max_redirections: usize },
    ProtocolError { message: String },
}
```

### ErrorSeverity

错误严重性级别。

```rust
pub enum ErrorSeverity {
    Critical,
    Error,
    Warning,
    Info,
}
```

## 路由决策

### RoutingDecision

路由决策枚举，表示请求路由的结果。

```rust
pub enum RoutingDecision {
    Route { backend_id: String },
    Redirect { target_addr: SocketAddr },
    Error { message: String },
    Drop,
}
```

## 使用示例

### 基本MongoDB代理设置

```rust
use puerta::modes::mongodb::{MongoDBProxy, MongoDBConfig};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = MongoDBConfig {
        mongos_endpoints: vec![
            "localhost:27017".to_string(),
            "localhost:27018".to_string(),
        ],
        session_affinity_enabled: true,
        session_timeout_sec: 300,
        health_check_interval_sec: 10,
    };
    
    let proxy = MongoDBProxy::new(config)
        .with_health_check();
    
    proxy.initialize_backends().await?;
    proxy.start_health_checks().await?;
    
    // 处理客户端请求
    let client_addr = "192.168.1.100:12345".parse()?;
    let decision = proxy.route_request(client_addr).await;
    
    match decision {
        RoutingDecision::Route { backend_id } => {
            println!("路由到后端: {}", backend_id);
        }
        RoutingDecision::Error { message } => {
            eprintln!("路由错误: {}", message);
        }
        _ => {}
    }
    
    Ok(())
}
```

### 自定义健康检查

```rust
use puerta::health::{HealthChecker, HealthStatus, HealthCheckManager};
use puerta::core::Backend;
use async_trait::async_trait;
use std::time::Duration;

struct CustomHealthChecker;

#[async_trait]
impl HealthChecker for CustomHealthChecker {
    async fn check_health(&self, backend: &Backend) -> HealthStatus {
        // 自定义健康检查逻辑
        match tokio::net::TcpStream::connect(backend.addr).await {
            Ok(_) => HealthStatus::Healthy,
            Err(e) => HealthStatus::Unhealthy {
                reason: format!("连接失败: {}", e),
            },
        }
    }
    
    fn check_timeout(&self) -> Duration {
        Duration::from_secs(5)
    }
    
    fn check_interval(&self) -> Duration {
        Duration::from_secs(30)
    }
}

// 使用自定义健康检查器
let checker = Box::new(CustomHealthChecker);
let health_manager = HealthCheckManager::new(checker);
```

这份API参考文档提供了Puerta项目所有公共API的详细说明，包括数据结构、方法签名、参数说明和使用示例，为开发者使用和扩展Puerta提供了完整的参考。
