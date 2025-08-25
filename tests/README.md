# Puerta 测试套件

本目录包含 Puerta 负载均衡器的完整测试套件。

## 目录结构

```
tests/
├── mongodb/              # MongoDB 负载均衡器测试
│   ├── README.md        # MongoDB 测试文档
│   ├── test_mongodb_lb_basic.sh       # 基础功能测试
│   ├── test_mongodb_lb_quick.sh       # 快速验证测试
│   └── test_mongodb_lb_comprehensive.sh # 综合测试套件
└── ...                  # 未来其他测试套件
```

## 快速开始

### 运行所有测试
```bash
# 从项目根目录运行
./tests/run_tests.sh
```

### 运行特定测试
```bash
# 基础功能测试 (~15秒)
./tests/mongodb/test_mongodb_lb_basic.sh

# 快速验证测试 (~30秒)
./tests/mongodb/test_mongodb_lb_quick.sh

# 综合测试套件 (~2-3分钟)
./tests/mongodb/test_mongodb_lb_comprehensive.sh
```

## 测试类型

### MongoDB 测试套件
验证 MongoDB 负载均衡器的功能：
- 基本连通性和路由器验证
- 负载均衡效率测试
- 并发连接处理
- 数据库操作验证
- 性能基准测试
- 错误处理和恢复能力

## 使用建议

### 开发阶段
```bash
# 频繁修改时的快速验证
./tests/mongodb/test_mongodb_lb_basic.sh
```

### 提交前验证
```bash
# 完整的功能验证
./tests/mongodb/test_mongodb_lb_quick.sh
```

### 部署前验证
```bash
# 全面的功能和性能测试
./tests/mongodb/test_mongodb_lb_comprehensive.sh
```

## 测试配置

所有测试脚本都支持通过修改配置变量来调整测试参数：

```bash
# 负载均衡器配置
LOAD_BALANCER_HOST="127.0.0.1"
LOAD_BALANCER_PORT="27016"
BACKEND_ROUTERS=("127.0.0.1:27017" "127.0.0.1:27018" "127.0.0.1:27019")
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

- MongoDB 集群正在运行
- Puerta 负载均衡器已启动
- 已安装 `mongosh` MongoDB Shell
- 已安装 `bc` 计算器工具

## 故障排除

如遇问题请查看：
1. 各测试套件的详细 README 文件
2. 测试日志文件中的错误信息
3. 确认所有前置条件已满足