# Puerta MongoDB Load Balancer Test Scripts

本目录包含用于测试 Puerta MongoDB 负载均衡器的完整测试套件。

## 测试脚本说明

### 1. 快速测试脚本 (`test_mongodb_lb_quick.sh`)
用于快速验证负载均衡器基本功能的轻量级测试。

**用途:**
- 开发过程中的快速验证
- CI/CD 流水线中的基本健康检查
- 部署后的功能验证

**测试内容:**
- 基本连通性测试
- 路由器功能验证
- 负载均衡基本验证（5个连接）
- 基本数据库操作测试

**运行时间:** ~10-30秒

### 2. 综合测试脚本 (`test_mongodb_lb_comprehensive.sh`)
全面的负载均衡器测试套件，包含深入的功能和性能测试。

**用途:**
- 完整的功能验证
- 性能基准测试
- 负载均衡效率分析
- 故障恢复能力测试
- 生产环境部署前验证

**测试内容:**
1. **基础连通性测试** - 验证负载均衡器和所有后端路由器的连通性
2. **路由器节点验证** - 确认所有节点都正确响应为mongos路由器
3. **集群拓扑分析** - 获取并分析MongoDB分片集群的拓扑信息
4. **负载均衡验证** - 测试20个连接的负载分配情况
5. **并发连接测试** - 验证20个并发连接的处理能力
6. **数据库操作测试** - CRUD操作的功能验证
7. **批量操作测试** - 100个文档的批量插入、查询、更新、删除
8. **性能基准测试** - 读取和写入操作的性能指标
9. **连接弹性测试** - 连接重试和故障恢复能力
10. **压力测试** - 30秒持续压力测试（5个并发客户端）
11. **错误处理测试** - 异常命令和错误情况的处理
12. **资源使用测试** - 内存和CPU使用情况监控

**运行时间:** ~2-3分钟

## 使用方法

### 前置条件
- MongoDB 集群正在运行
- Puerta 负载均衡器已启动并监听 27016 端口
- 已安装 `mongosh` MongoDB Shell
- 已安装 `bc` 计算器工具

### 快速测试
```bash
./test_mongodb_lb_quick.sh
```

### 综合测试
```bash
./test_mongodb_lb_comprehensive.sh
```

## 测试结果

### 快速测试输出示例
```
=== Quick Puerta Load Balancer Test ===
Testing: 127.0.0.1:27016
Backends: 127.0.0.1:27017 127.0.0.1:27018 127.0.0.1:27019

1. Testing basic connectivity...
✅ Load balancer connectivity: SUCCESS

2. Verifying router functionality...
✅ Router verification: SUCCESS

3. Testing load balancing (5 connections)...
   Connection 1: connectionId 3869
   Connection 2: connectionId 3662
   Connection 3: connectionId 4002
   Connection 4: connectionId 3665
   Connection 5: connectionId 3667
✅ Load balancing: SUCCESS (5/5 unique connections)

4. Testing basic operations...
✅ Basic operations: SUCCESS

=== Quick Test Summary ===
✅ Load balancer is working correctly
✅ All basic tests passed
```

### 综合测试输出
综合测试会生成：
- 控制台实时输出
- 详细日志文件 (`/tmp/puerta_test_YYYYMMDD_HHMMSS.log`)
- JSON格式报告文件 (`/tmp/puerta_test_report_YYYYMMDD_HHMMSS.json`)

**关键指标:**
- 负载均衡效率（连接ID唯一性比例）
- 性能指标（读取/写入操作每秒）
- 压力测试结果
- 资源使用情况
- 整体成功率

## 测试配置

### 环境变量
可以通过修改脚本开头的配置变量来调整测试参数：

```bash
# 负载均衡器配置
LOAD_BALANCER_HOST="127.0.0.1"
LOAD_BALANCER_PORT="27016"
BACKEND_ROUTERS=("127.0.0.1:27017" "127.0.0.1:27018" "127.0.0.1:27019")

# 测试配置
CONCURRENT_CONNECTIONS=20
STRESS_TEST_DURATION=30
BATCH_SIZE=100
MAX_RETRIES=3
```

### 自定义测试
1. 修改 `BACKEND_ROUTERS` 数组以匹配你的实际后端路由器
2. 调整 `CONCURRENT_CONNECTIONS` 来改变并发测试强度
3. 修改 `STRESS_TEST_DURATION` 来调整压力测试时长
4. 更改 `BATCH_SIZE` 来调整批量操作的大小

## 故障排除

### 常见问题
1. **连接失败** - 检查MongoDB集群和负载均衡器是否正在运行
2. **权限错误** - 确保有足够的权限执行测试操作
3. **端口冲突** - 确认端口配置正确且未被占用
4. **性能问题** - 检查系统资源使用情况

### 调试技巧
- 查看详细的日志文件了解具体错误信息
- 使用快速测试脚本先验证基本功能
- 逐步增加测试强度来定位问题
- 检查MongoDB集群的健康状态

## 生产环境建议

### 监控指标
基于测试结果，建议监控以下指标：
- 连接池使用率
- 请求响应时间
- 错误率
- 后端节点健康状态
- 资源使用情况

### 性能优化
- 根据负载均衡效率调整连接池配置
- 优化后端路由器的负载分配策略
- 实施适当的缓存机制
- 定期进行性能基准测试

## 版本兼容性

- **操作系统:** Linux, macOS (已测试)
- **MongoDB:** 4.4+ (推荐 5.0+)
- **依赖工具:** mongosh, bc, standard Unix utilities

## 贡献指南

如需添加新的测试用例或改进现有测试：
1. 遵循现有的测试结构
2. 添加适当的错误处理
3. 更新文档说明
4. 确保在所有支持的平台上运行正常