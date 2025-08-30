# Puerta 测试套件

本目录包含 Puerta 负载均衡器的完整测试套件，支持 MongoDB 和 Redis 集群的负载均衡测试。

## 目录结构

```
tests/
├── mongodb/              # MongoDB 负载均衡器测试
│   ├── README.md        # MongoDB 测试文档
│   ├── test_mongodb_lb_basic.sh       # 基础功能测试
│   ├── test_mongodb_lb_quick.sh       # 快速验证测试
│   └── test_mongodb_lb_comprehensive.sh # 综合测试套件
├── redis/               # Redis 集群负载均衡器测试
│   ├── README.md        # Redis 测试文档
│   ├── test_redis_lb_basic.sh         # 基础功能测试
│   ├── test_redis_lb_quick.sh         # 快速验证测试
│   └── test_redis_lb_comprehensive.sh # 综合测试套件
└── test.sh              # 统一测试运行器
```

## 快速开始

### 使用便捷测试脚本
```bash
# 从 tests 目录运行
cd tests
./test.sh <database> <test_type>

# MongoDB 测试
./test.sh mongo basic     # MongoDB 基础测试
./test.sh mongo quick     # MongoDB 快速测试  
./test.sh mongo full      # MongoDB 综合测试

# Redis 测试
./test.sh redis basic     # Redis 基础测试
./test.sh redis quick     # Redis 快速测试
./test.sh redis full      # Redis 综合测试
./test.sh redis routing   # Redis 路由逻辑验证

# 注意：由于 Puerta 在任何时刻只能运行一种模式（MongoDB 或 Redis），
# 需要根据当前运行的模式选择相应的测试
```

### 直接运行测试脚本
```bash
# MongoDB 测试
./tests/mongodb/test_mongodb_lb_basic.sh       # 基础功能测试 (~15秒)
./tests/mongodb/test_mongodb_lb_quick.sh       # 快速验证测试 (~30秒)
./tests/mongodb/test_mongodb_lb_comprehensive.sh # 综合测试套件 (~2-3分钟)

# Redis 测试
./tests/redis/test_redis_lb_basic.sh           # 基础功能测试 (~15秒)
./tests/redis/test_redis_lb_quick.sh           # 快速验证测试 (~30秒)
./tests/redis/test_redis_lb_comprehensive.sh   # 综合测试套件 (~2-3分钟)
./tests/redis/test_redis_routing_logic.sh      # 路由逻辑验证 (~10秒)
```

## 测试类型

### MongoDB 测试套件
验证 MongoDB 负载均衡器的功能：
- 基本连通性和路由验证
- 会话亲和性 (Session Affinity) 测试
- 负载均衡效率测试
- 并发连接处理
- 数据库操作验证
- 性能基准测试
- 错误处理和恢复能力

### Redis 测试套件
验证 Redis 集群负载均衡器的功能：
- 基本连通性和 RESP 协议支持
- Slot 路由和 Hash Tag 功能
- CRC16 槽位计算验证
- Redis 集群拓扑感知
- 多种 Redis 数据类型支持
- 并发连接和性能测试
- MOVED/ASK 重定向处理
- 路由一致性验证
- 错误处理和连接弹性

## 使用建议

### 开发阶段
```bash
# MongoDB 快速验证
./test.sh mongo basic

# Redis 快速验证  
./test.sh redis basic

# Redis 路由逻辑验证
./test.sh redis routing
```

### 提交前验证
```bash
# MongoDB 功能验证
./test.sh mongo quick

# Redis 功能验证
./test.sh redis quick
```

### 部署前验证
```bash
# 全面的功能和性能测试
./test.sh mongo full    # MongoDB 综合测试
./test.sh redis full    # Redis 综合测试
```

## 测试配置

### MongoDB 测试配置
```bash
# 负载均衡器配置
LOAD_BALANCER_HOST="127.0.0.1"
LOAD_BALANCER_PORT="27016"
BACKEND_ROUTERS=("127.0.0.1:27017" "127.0.0.1:27018" "127.0.0.1:27019")
```

### Redis 测试配置
```bash
# 负载均衡器配置
LOAD_BALANCER_HOST="127.0.0.1"
LOAD_BALANCER_PORT="6379"
REDIS_NODES=("127.0.0.1:7001" "127.0.0.1:7002" "127.0.0.1:7003" "127.0.0.1:7004" "127.0.0.1:7005" "127.0.0.1:7006")
```

## 添加新测试

1. 在相应子目录创建测试脚本
2. 遵循现有的命名约定：`test_<component>_<type>.sh`
3. 添加适当的文档和注释
4. 更新相关 README 文件

## 输出文件

测试过程中会生成以下文件：
- 控制台实时输出
- 详细日志文件 (`/tmp/puerta_test_*.log`)
- JSON格式报告文件 (`/tmp/puerta_test_report_*.json`)

## 前置条件

### MongoDB 测试前置条件
- MongoDB Sharded Cluster 正在运行
- Puerta 负载均衡器已启动 (MongoDB mode, 端口 27016)
- 已安装 `mongosh` MongoDB Shell
- 已安装 `bc` 计算器工具

### Redis 测试前置条件  
- Redis Cluster 正在运行 (端口 7001-7006)
- Puerta 负载均衡器已启动 (Redis mode, 端口 6379)
- 已安装 `redis-cli` Redis 命令行工具
- 已安装 `bc` 计算器工具

## 性能指标参考

### MongoDB 测试
- 基础测试：~15秒
- 快速测试：~30秒  
- 综合测试：~2-3分钟
- 期望性能：50-100 ops/sec

### Redis 测试
- 基础测试：~15秒
- 快速测试：~30秒
- 综合测试：~2-3分钟
- 路由逻辑验证：~10秒
- 期望性能：100-150 ops/sec
- 压力测试：30秒内处理4000+操作
- 槽位计算性能：~200 ops/sec

## 故障排除

如遇问题请查看：
1. 各测试套件的详细 README 文件 (`tests/mongodb/README.md`, `tests/redis/README.md`)
2. 测试日志文件中的错误信息 (`/tmp/puerta_*_test_*.log`)
3. 确认所有前置条件已满足
4. 验证相应的后端数据库集群状态