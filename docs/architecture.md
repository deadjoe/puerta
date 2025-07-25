# Puerta 架构设计文档

## 概述

Puerta是一个高性能的数据库集群代理，专为MongoDB分片集群和Redis集群设计。基于Cloudflare的Pingora框架构建，采用异步I/O和零拷贝技术。

## 整体架构

```
┌─────────────────────────────────────────────────────────────┐
│                        Client Layer                         │
├─────────────────────────────────────────────────────────────┤
│                     Puerta Proxy                            │
│  ┌─────────────────┐  ┌─────────────────┐                  │
│  │  MongoDB Mode   │  │   Redis Mode    │                  │
│  │ ┌─────────────┐ │  │ ┌─────────────┐ │                  │
│  │ │Session      │ │  │ │Slot         │ │                  │
│  │ │Affinity     │ │  │ │Mapping      │ │                  │
│  │ └─────────────┘ │  │ └─────────────┘ │                  │
│  │ ┌─────────────┐ │  │ ┌─────────────┐ │                  │
│  │ │Load         │ │  │ │Redirection  │ │                  │
│  │ │Balancing    │ │  │ │Handling     │ │                  │
│  │ └─────────────┘ │  │ └─────────────┘ │                  │
│  └─────────────────┘  └─────────────────┘                  │
│  ┌─────────────────────────────────────────────────────────┤
│  │                Core Components                          │
│  │ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐        │
│  │ │Backend      │ │Health       │ │Error        │        │
│  │ │Manager      │ │Checker      │ │Handler      │        │
│  │ └─────────────┘ └─────────────┘ └─────────────┘        │
│  └─────────────────────────────────────────────────────────┤
├─────────────────────────────────────────────────────────────┤
│                    Pingora Framework                        │
├─────────────────────────────────────────────────────────────┤
│                   Backend Services                          │
│  ┌─────────────────┐              ┌─────────────────┐      │
│  │MongoDB Cluster  │              │Redis Cluster    │      │
│  └─────────────────┘              └─────────────────┘      │
└─────────────────────────────────────────────────────────────┘
```

## 核心组件

### 1. MongoDB代理架构

```
Client Request → MongoDBProxy.route_request()
                      ↓
              AffinityManager ←→ BackendManager
                      ↓                ↓
              Session Selection   Health Check
                      ↓
              Routing Decision
```

**关键特性**:
- 会话亲和性：确保客户端连接一致性
- 多种标识策略：SocketAddr、指纹、会话ID、混合模式
- 负载均衡：加权轮询算法
- 健康检查：MongoDB Wire Protocol检查

### 2. Redis代理架构

```
Client Command → RESP Parser → Key Extraction
                                    ↓
                Slot Calculation → Slot Mapping
                                    ↓
                Node Selection → Command Forwarding
```

**关键特性**:
- RESP协议解析：完整的Redis协议支持
- 槽位路由：CRC16算法计算键槽位
- 重定向处理：MOVED/ASK重定向支持
- 集群发现：自动发现Redis集群拓扑

### 3. 会话亲和性系统

```rust
pub struct AffinityManager {
    sessions: Arc<RwLock<HashMap<ClientIdentifier, AffinitySession>>>,
    identification_strategy: ClientIdentificationStrategy,
    session_timeout: Duration,
}
```

**客户端标识策略**:
- `SocketAddr`: 基于客户端地址
- `ConnectionFingerprint`: SHA-256连接指纹
- `SessionId`: MongoDB会话ID
- `Hybrid`: 混合多种策略

### 4. 健康检查系统

```rust
pub trait HealthChecker: Send + Sync {
    async fn check_health(&self, backend: &Backend) -> HealthStatus;
    fn check_timeout(&self) -> Duration;
    fn check_interval(&self) -> Duration;
}
```

**检查类型**:
- MongoDB: `ismaster`命令 + Wire Protocol
- Redis: `PING` + `CLUSTER NODES`命令
- 支持重试和超时机制

### 5. 后端管理

```rust
pub struct BackendManager {
    backends: Arc<RwLock<HashMap<String, Backend>>>,
}
```

**功能**:
- 后端服务注册和发现
- 健康状态管理
- 负载均衡权重调整
- 连接池管理

## 数据流

### MongoDB数据流

```
1. 客户端连接 → TCP Accept
2. 客户端识别 → 生成ClientIdentifier
3. 会话查找 → 检查现有会话
4. 后端选择 → 负载均衡或会话路由
5. 数据转发 → 双向TCP转发
6. 连接维护 → 会话超时管理
```

### Redis数据流

```
1. 命令接收 → RESP协议解析
2. 键提取 → 从命令中提取键
3. 槽位计算 → CRC16算法
4. 节点映射 → 槽位到节点映射
5. 命令转发 → 发送到目标节点
6. 响应处理 → 处理重定向响应
```

## 并发模型

基于Tokio异步运行时：

```
Tokio Runtime
├── Worker Thread Pool
├── Task Scheduler
├── Async I/O
└── Timer System
```

**并发特性**:
- 异步I/O：非阻塞网络操作
- 任务调度：公平调度和工作窃取
- 共享状态：Arc + RwLock模式
- 无锁优化：关键路径使用原子操作

## 内存管理

### 数据结构设计

```rust
// 共享状态
Arc<RwLock<HashMap<String, Backend>>>        // 后端池
Arc<RwLock<HashMap<ClientId, Session>>>      // 会话存储
Arc<RwLock<SlotMapping>>                     // Redis槽位映射

// 线程本地
Vec<u8>                                      // I/O缓冲区
ObjectPool<Connection>                       // 连接池
```

### 优化策略

1. **零拷贝I/O**: 使用`tokio::io::copy`
2. **对象池**: 复用缓冲区和连接对象
3. **引用计数**: `Arc`共享不可变数据
4. **写时复制**: 配置数据COW模式

## 错误处理

### 错误层次

```rust
pub enum PuertaError {
    Config(ConfigError),
    MongoDB(MongoDBError),
    Redis(RedisError),
    Health(HealthError),
    Network(NetworkError),
}
```

### 错误处理策略

- **分级处理**: Critical/Error/Warning/Info
- **恢复机制**: 可恢复错误自动重试
- **日志集成**: 结构化日志记录
- **监控集成**: 错误指标收集

## 配置管理

### 配置结构

```rust
pub struct Config {
    pub server: ServerConfig,      // 服务器配置
    pub proxy: ProxyConfig,        // 代理配置
}

pub enum ProxyConfig {
    MongoDB(MongoDBConfig),
    Redis(RedisConfig),
}
```

### 配置源

- TOML配置文件
- 环境变量
- 命令行参数
- 配置验证和热重载

## 性能特征

### 关键指标

- **吞吐量**: >10K连接/秒
- **延迟**: P99 < 10ms
- **内存**: 线性增长，支持10K+并发连接
- **CPU**: 多核高效利用

### 优化点

1. **I/O优化**: 64KB缓冲区，批量处理
2. **并发优化**: 分片锁，无锁数据结构
3. **内存优化**: 对象池，引用计数
4. **网络优化**: 连接复用，流水线处理

## 可扩展性

### 水平扩展

- 多实例部署
- 负载均衡器前置
- 状态无关设计
- 配置共享

### 垂直扩展

- 多核CPU利用
- 内存池化管理
- I/O并发优化
- 缓存层次优化

## 监控和观测

### 指标收集

```rust
struct Metrics {
    connections_total: AtomicU64,
    requests_per_second: AtomicU64,
    error_rate: AtomicU64,
    latency_histogram: Histogram,
}
```

### 日志系统

- 结构化日志（JSON格式）
- 分级日志（Debug/Info/Warn/Error）
- 性能日志（慢查询、连接统计）
- 审计日志（配置变更、管理操作）

### 健康检查端点

- `/health`: 服务健康状态
- `/metrics`: Prometheus指标
- `/status`: 详细状态信息
- `/config`: 配置信息（脱敏）

## 安全考虑

### 网络安全

- TLS/SSL支持
- 客户端认证
- IP白名单
- 连接限制

### 数据安全

- 敏感信息脱敏
- 审计日志
- 权限控制
- 安全配置

这份架构文档为Puerta项目提供了全面的技术架构说明，涵盖了系统设计、组件交互、性能特征和扩展性考虑。
