# Puerta 开发规范与指南

## 目录

1. [代码风格与规范](#代码风格与规范)
2. [架构设计原则](#架构设计原则)
3. [错误处理规范](#错误处理规范)
4. [测试规范](#测试规范)
5. [性能优化指南](#性能优化指南)
6. [文档规范](#文档规范)
7. [Git工作流](#git工作流)
8. [代码审查清单](#代码审查清单)

## 代码风格与规范

### Rust代码风格

遵循标准的Rust代码风格，使用`rustfmt`进行格式化：

```bash
cargo fmt
```

#### 命名规范

- **模块名**: 使用snake_case，如`mongodb_proxy`
- **结构体**: 使用PascalCase，如`AffinityManager`
- **函数名**: 使用snake_case，如`get_backend_for_client`
- **常量**: 使用SCREAMING_SNAKE_CASE，如`DEFAULT_TIMEOUT`
- **枚举**: 使用PascalCase，如`HealthStatus`

#### 代码组织

```rust
// 1. 外部crate导入
use std::collections::HashMap;
use tokio::sync::RwLock;

// 2. 内部模块导入
use crate::core::Backend;
use crate::error::PuertaError;

// 3. 类型定义
pub type BackendPool = Arc<RwLock<HashMap<String, Backend>>>;

// 4. 常量定义
const DEFAULT_SESSION_TIMEOUT: Duration = Duration::from_secs(300);

// 5. 结构体定义
pub struct AffinityManager {
    // 公共字段在前
    pub session_timeout: Duration,
    // 私有字段在后
    sessions: Arc<RwLock<HashMap<ClientIdentifier, AffinitySession>>>,
}

// 6. 实现块
impl AffinityManager {
    // 构造函数在前
    pub fn new(session_timeout: Duration) -> Self { }
    
    // 公共方法
    pub async fn get_backend_for_client(&self) -> Option<String> { }
    
    // 私有方法在后
    async fn cleanup_expired_sessions(&self) -> usize { }
}
```

### 注释规范

#### 文档注释

所有公共API必须有文档注释：

```rust
/// MongoDB会话亲和性管理器
/// 
/// 负责管理客户端到后端的会话映射，确保同一客户端的连接
/// 总是路由到相同的MongoDB mongos实例。
/// 
/// # Examples
/// 
/// ```rust
/// use std::time::Duration;
/// let manager = AffinityManager::new(Duration::from_secs(300));
/// ```
pub struct AffinityManager {
    /// 会话超时时间
    pub session_timeout: Duration,
    sessions: Arc<RwLock<HashMap<ClientIdentifier, AffinitySession>>>,
}

impl AffinityManager {
    /// 为客户端获取或分配后端
    /// 
    /// # Arguments
    /// 
    /// * `client_addr` - 客户端地址
    /// * `available_backends` - 可用后端列表
    /// * `selection_fn` - 后端选择函数
    /// * `connection_data` - 连接数据用于指纹生成
    /// 
    /// # Returns
    /// 
    /// 返回分配的后端ID，如果没有可用后端则返回None
    pub async fn get_backend_for_client(
        &self,
        client_addr: SocketAddr,
        available_backends: &[String],
        selection_fn: impl FnOnce(&[String]) -> Option<String>,
        connection_data: Option<&[u8]>,
    ) -> Option<String> {
        // 实现...
    }
}
```

#### 内联注释

使用内联注释解释复杂逻辑：

```rust
// 生成客户端标识符，支持多种策略
let client_id = match self.identification_strategy {
    ClientIdentificationStrategy::SocketAddr => {
        ClientIdentifier::SocketAddr(client_addr)
    }
    ClientIdentificationStrategy::ConnectionFingerprint => {
        // 使用SHA-256生成连接指纹
        if let Some(data) = connection_data {
            let mut hasher = Sha256::new();
            hasher.update(data);
            let fingerprint = hex::encode(hasher.finalize());
            ClientIdentifier::Fingerprint(fingerprint)
        } else {
            // 回退到地址标识
            ClientIdentifier::SocketAddr(client_addr)
        }
    }
};
```

## 架构设计原则

### 1. 单一职责原则

每个模块和结构体应该有明确的单一职责：

```rust
// ✅ 好的设计 - 职责明确
pub struct AffinityManager {
    // 只负责会话亲和性管理
}

pub struct HealthCheckManager {
    // 只负责健康检查
}

// ❌ 避免的设计 - 职责混乱
pub struct MongoDBManager {
    // 既管理连接又管理健康检查又管理负载均衡
}
```

### 2. 依赖注入

使用依赖注入提高可测试性：

```rust
// ✅ 好的设计 - 依赖注入
pub struct HealthCheckManager {
    checker: Box<dyn HealthChecker>,
}

impl HealthCheckManager {
    pub fn new(checker: Box<dyn HealthChecker>) -> Self {
        Self { checker }
    }
}

// ❌ 避免的设计 - 硬编码依赖
pub struct HealthCheckManager {
    // 硬编码特定的检查器类型
}
```

### 3. 错误处理

使用统一的错误处理系统：

```rust
use crate::error::{PuertaError, Result};

pub async fn connect_to_backend(&self, backend_id: &str) -> Result<Connection> {
    let backend = self.get_backend(backend_id)
        .ok_or_else(|| PuertaError::backend_not_found(backend_id))?;
    
    let connection = TcpStream::connect(backend.addr).await
        .map_err(|e| PuertaError::connection_failed(backend_id, e))?;
    
    Ok(Connection::new(connection))
}
```

## 错误处理规范

### 错误类型定义

使用`thiserror`定义错误类型：

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MongoDBError {
    #[error("Connection to backend {backend_id} failed: {source}")]
    ConnectionFailed {
        backend_id: String,
        #[source]
        source: std::io::Error,
    },
    
    #[error("Backend {backend_id} not found")]
    BackendNotFound { backend_id: String },
    
    #[error("Session affinity disabled")]
    AffinityDisabled,
}
```

### 错误传播

使用`?`操作符进行错误传播：

```rust
pub async fn process_request(&self, request: Request) -> Result<Response> {
    let backend_id = self.select_backend(&request).await?;
    let connection = self.connect_to_backend(&backend_id).await?;
    let response = connection.send_request(request).await?;
    Ok(response)
}
```

### 错误日志

记录适当级别的错误日志：

```rust
match result {
    Ok(response) => {
        log::debug!("Request processed successfully");
        Ok(response)
    }
    Err(e) => {
        if e.is_recoverable() {
            log::warn!("Recoverable error occurred: {}", e);
        } else {
            log::error!("Critical error occurred: {}", e);
        }
        Err(e)
    }
}
```

## 测试规范

### 单元测试

每个公共函数都应该有对应的单元测试：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test;
    
    #[tokio::test]
    async fn test_affinity_manager_session_creation() {
        // Arrange
        let manager = AffinityManager::new(Duration::from_secs(300));
        let client_addr = "127.0.0.1:12345".parse().unwrap();
        let backends = vec!["backend1".to_string()];
        
        // Act
        let result = manager.get_backend_for_client(
            client_addr,
            &backends,
            |backends| backends.first().cloned(),
            None
        ).await;
        
        // Assert
        assert_eq!(result, Some("backend1".to_string()));
        assert_eq!(manager.session_count().await, 1);
    }
    
    #[tokio::test]
    async fn test_affinity_manager_session_reuse() {
        // 测试会话重用逻辑
    }
    
    #[tokio::test]
    async fn test_affinity_manager_session_cleanup() {
        // 测试会话清理逻辑
    }
}
```

### 集成测试

在`tests/`目录下创建集成测试：

```rust
// tests/mongodb_integration.rs
use puerta::modes::mongodb::MongoDBProxy;
use puerta::config::MongoDBConfig;

#[tokio::test]
async fn test_mongodb_proxy_end_to_end() {
    // 端到端集成测试
}
```

### Mock和测试工具

使用Mock对象进行测试：

```rust
// 创建Mock健康检查器
struct MockHealthChecker {
    should_pass: bool,
}

#[async_trait]
impl HealthChecker for MockHealthChecker {
    async fn check_health(&self, _backend: &Backend) -> HealthStatus {
        if self.should_pass {
            HealthStatus::Healthy
        } else {
            HealthStatus::Unhealthy {
                reason: "Mock failure".to_string(),
            }
        }
    }
    
    fn check_timeout(&self) -> Duration {
        Duration::from_secs(1)
    }
    
    fn check_interval(&self) -> Duration {
        Duration::from_secs(5)
    }
}
```

## 性能优化指南

### 内存管理

1. **避免不必要的克隆**：
```rust
// ✅ 使用引用
fn process_backend(backend: &Backend) -> Result<()> { }

// ❌ 避免不必要的克隆
fn process_backend(backend: Backend) -> Result<()> { }
```

2. **使用合适的数据结构**：
```rust
// ✅ 对于频繁查找使用HashMap
use std::collections::HashMap;
let backends: HashMap<String, Backend> = HashMap::new();

// ✅ 对于有序数据使用BTreeMap
use std::collections::BTreeMap;
let ordered_backends: BTreeMap<String, Backend> = BTreeMap::new();
```

### 异步编程

1. **避免阻塞操作**：
```rust
// ✅ 使用异步I/O
let data = tokio::fs::read("config.toml").await?;

// ❌ 避免阻塞I/O
let data = std::fs::read("config.toml")?;
```

2. **合理使用并发**：
```rust
// ✅ 并行处理独立任务
let checks: Vec<_> = backends.iter()
    .map(|backend| check_health(backend))
    .collect();
let results = futures::future::join_all(checks).await;

// ❌ 串行处理可并行的任务
for backend in backends {
    let result = check_health(backend).await;
}
```

## 文档规范

### API文档

所有公共API必须有完整的文档：

```rust
/// MongoDB代理配置
/// 
/// 包含MongoDB集群的连接配置、会话亲和性设置和健康检查参数。
/// 
/// # Examples
/// 
/// ```rust
/// use puerta::config::MongoDBConfig;
/// 
/// let config = MongoDBConfig {
///     mongos_endpoints: vec!["localhost:27017".to_string()],
///     session_affinity_enabled: true,
///     session_timeout_sec: 300,
///     health_check_interval_sec: 10,
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MongoDBConfig {
    /// MongoDB mongos实例端点列表
    pub mongos_endpoints: Vec<String>,
    /// 是否启用会话亲和性
    pub session_affinity_enabled: bool,
    /// 会话超时时间（秒）
    pub session_timeout_sec: u64,
    /// 健康检查间隔（秒）
    pub health_check_interval_sec: u64,
}
```

### 模块文档

每个模块都应该有模块级文档：

```rust
//! MongoDB代理模块
//! 
//! 此模块提供MongoDB集群的代理功能，包括：
//! 
//! - 会话亲和性管理
//! - 负载均衡
//! - 健康检查
//! - 连接管理
//! 
//! # Architecture
//! 
//! ```text
//! Client -> MongoDBProxy -> AffinityManager -> Backend Selection
//!                       -> HealthChecker   -> Backend Health
//! ```
//! 
//! # Examples
//! 
//! ```rust
//! use puerta::modes::mongodb::MongoDBProxy;
//! use puerta::config::MongoDBConfig;
//! 
//! let config = MongoDBConfig::default();
//! let proxy = MongoDBProxy::new(config).with_health_check();
//! ```

pub mod affinity;
pub mod health;
```

## Git工作流

### 分支命名规范

- `feature/功能名称` - 新功能开发
- `bugfix/问题描述` - 错误修复
- `hotfix/紧急修复` - 紧急修复
- `refactor/重构描述` - 代码重构
- `docs/文档更新` - 文档更新

### 提交信息规范

使用约定式提交格式：

```
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

类型：
- `feat`: 新功能
- `fix`: 错误修复
- `docs`: 文档更新
- `style`: 代码格式化
- `refactor`: 代码重构
- `test`: 测试相关
- `chore`: 构建过程或辅助工具的变动

示例：
```
feat(mongodb): add multi-strategy client identification

- Support SocketAddr, fingerprint, session ID, and hybrid strategies
- Improve NAT-friendly session affinity
- Add comprehensive logging for session management

Closes #123
```

### Pull Request规范

PR标题应该清晰描述变更内容：
- `feat: 添加Redis集群支持`
- `fix: 修复会话亲和性内存泄漏`
- `refactor: 重构健康检查模块`

PR描述应该包含：
1. 变更摘要
2. 测试说明
3. 相关Issue链接
4. 破坏性变更说明（如有）

## 代码审查清单

### 功能性检查

- [ ] 代码实现符合需求规格
- [ ] 边界条件处理正确
- [ ] 错误处理完整
- [ ] 日志记录适当

### 代码质量检查

- [ ] 代码风格符合规范
- [ ] 命名清晰有意义
- [ ] 函数职责单一
- [ ] 避免代码重复

### 性能检查

- [ ] 避免不必要的内存分配
- [ ] 合理使用数据结构
- [ ] 异步操作正确使用
- [ ] 避免阻塞操作

### 安全检查

- [ ] 输入验证充分
- [ ] 避免资源泄漏
- [ ] 并发安全
- [ ] 错误信息不泄露敏感信息

### 测试检查

- [ ] 单元测试覆盖充分
- [ ] 测试用例有意义
- [ ] 集成测试通过
- [ ] 性能测试通过

### 文档检查

- [ ] API文档完整
- [ ] 示例代码正确
- [ ] 变更日志更新
- [ ] README更新（如需要）

## 最佳实践

### 1. 使用类型系统

充分利用Rust的类型系统来防止错误：

```rust
// ✅ 使用新类型模式
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BackendId(String);

impl BackendId {
    pub fn new(id: String) -> Result<Self, ValidationError> {
        if id.is_empty() {
            return Err(ValidationError::EmptyBackendId);
        }
        Ok(BackendId(id))
    }
}

// ❌ 使用原始字符串类型
pub fn get_backend(backend_id: String) -> Option<Backend> { }
```

### 2. 资源管理

正确管理资源生命周期：

```rust
// ✅ 使用RAII模式
pub struct Connection {
    stream: TcpStream,
}

impl Drop for Connection {
    fn drop(&mut self) {
        log::debug!("Closing connection");
        // 清理资源
    }
}
```

### 3. 配置管理

使用结构化配置：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub proxy: ProxyConfig,
    pub logging: LoggingConfig,
}

impl Config {
    pub fn load_from_file(path: &Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)
            .map_err(ConfigError::IoError)?;
        
        toml::from_str(&content)
            .map_err(ConfigError::ParseError)
    }
    
    pub fn validate(&self) -> Result<(), ValidationError> {
        // 配置验证逻辑
        Ok(())
    }
}
```

这份开发规范为Puerta项目的长期维护和团队协作提供了全面的指导。遵循这些规范可以确保代码质量、提高开发效率，并降低维护成本。
