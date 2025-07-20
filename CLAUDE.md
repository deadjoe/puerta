# Puerta - MongoDB & Redis 负载均衡器项目

## 项目目标
基于Rust开发一个能够同时支持 MongoDB Shared Cluster 和 Redis Cluster 的负载均衡器，部署在kubernetes之外的网络边缘，为其他网络中的client applications提供到后端数据库集群的负载均衡服务。

## 核心设计目标

1. **基于Rust** - 充分利用Rust的极致安全性和稳定性
2. **轻量化** - 极小的代码体积和资源消耗footprint  
3. **极致性能** - 最优的网络吞吐性能和延迟
4. **基于优秀开源框架** - 基于以下两个开源项目构建：
   - Cloudflare的Pingora框架
   - RCProxy项目
   - 两个参考项目都在examples/目录下可本地访问完整源码
5. **高质量开发** - 使用Rust生态中的主流高质量测试工具和质量保证工具
6. **高测试覆盖率** - 包含高覆盖率的测试用例和代码内文档

## 技术要求

### 后端数据库支持
- MongoDB Shared Cluster
- Redis Cluster
- 后端数据库可能部署在物理机/虚拟机/k8s中

### 信息获取原则
对于MongoDB和Redis相关技术信息，必须第一时间从官方网站获取最新官方信息，不能主观猜测或使用过时知识。

### 开发环境配置
- Git环境已初始化
- Rust/Cargo项目环境已配置
- 需要配置完整的测试和质量保证工具链

## 项目结构
- `examples/pingora/` - Cloudflare Pingora框架参考代码
- `examples/rcproxy/` - RCProxy项目参考代码

## 开发阶段
1. ✅ 环境初始化
2. 🔄 代码阅读和技术调研
3. 架构设计
4. 核心功能实现
5. 测试和优化

## 参考命令
- 测试: `cargo test`
- 代码检查: `cargo clippy`
- 格式化: `cargo fmt`
- 构建: `cargo build --release`