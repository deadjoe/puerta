use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use puerta::core::{Backend, BackendManager, BackendMetadata};
use puerta::health::{HealthCheckManager, create_health_checker};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::runtime::Runtime;

/// 创建测试后端
fn create_test_backend(id: &str, addr: &str) -> Backend {
    Backend {
        id: id.to_string(),
        addr: addr.parse().unwrap(),
        weight: 1,
        healthy: true,
        last_health_check: Some(SystemTime::now()),
        metadata: BackendMetadata::MongoDB {
            version: Some("4.4.0".to_string()),
            is_primary: true,
            connection_count: 0,
        },
    }
}

/// 后端管理器性能基准测试
fn bench_backend_manager(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("backend_manager");
    
    // 测试不同数量的后端
    for backend_count in [10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::new("add_backends", backend_count),
            backend_count,
            |b, &backend_count| {
                b.to_async(&rt).iter(|| async {
                    let manager = BackendManager::new();
                    
                    for i in 0..backend_count {
                        let backend = create_test_backend(
                            &format!("backend_{}", i),
                            &format!("127.0.0.1:{}", 8000 + i)
                        );
                        manager.add_backend(backend).await;
                    }
                    
                    black_box(manager);
                });
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("get_backend", backend_count),
            backend_count,
            |b, &backend_count| {
                b.to_async(&rt).iter(|| async {
                    let manager = BackendManager::new();
                    
                    // 预填充后端
                    for i in 0..backend_count {
                        let backend = create_test_backend(
                            &format!("backend_{}", i),
                            &format!("127.0.0.1:{}", 8000 + i)
                        );
                        manager.add_backend(backend).await;
                    }
                    
                    // 基准测试查询操作
                    let backend_id = format!("backend_{}", backend_count / 2);
                    let result = manager.get_backend(&backend_id).await;
                    black_box(result);
                });
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("get_healthy_backends", backend_count),
            backend_count,
            |b, &backend_count| {
                b.to_async(&rt).iter(|| async {
                    let manager = BackendManager::new();
                    
                    // 预填充后端（一半健康，一半不健康）
                    for i in 0..backend_count {
                        let mut backend = create_test_backend(
                            &format!("backend_{}", i),
                            &format!("127.0.0.1:{}", 8000 + i)
                        );
                        backend.healthy = i % 2 == 0; // 一半健康
                        manager.add_backend(backend).await;
                    }
                    
                    // 基准测试健康后端过滤
                    let healthy_backends = manager.get_healthy_backends().await;
                    black_box(healthy_backends);
                });
            },
        );
    }
    
    group.finish();
}

/// 数据结构操作性能基准测试
fn bench_data_structures(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("data_structures");
    
    // 测试HashMap查找性能
    for size in [100, 1000, 10000].iter() {
        group.bench_with_input(
            BenchmarkId::new("hashmap_lookup", size),
            size,
            |b, &size| {
                b.iter(|| {
                    use std::collections::HashMap;
                    let mut map = HashMap::new();
                    
                    // 填充数据
                    for i in 0..size {
                        map.insert(format!("key_{}", i), format!("value_{}", i));
                    }
                    
                    // 查找操作
                    let key = format!("key_{}", size / 2);
                    let result = map.get(&key);
                    black_box(result);
                });
            },
        );
    }
    
    group.finish();
}

/// 字符串操作性能基准测试
fn bench_string_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("string_operations");
    
    // 测试字符串格式化性能
    group.bench_function("string_formatting", |b| {
        b.iter(|| {
            let mut results = Vec::new();
            for i in 0..1000 {
                let formatted = format!("backend_{}_{}", i, "test");
                results.push(formatted);
            }
            black_box(results);
        });
    });
    
    // 测试字符串解析性能
    group.bench_function("socket_addr_parsing", |b| {
        b.iter(|| {
            let mut results = Vec::new();
            for i in 0..1000 {
                let addr_str = format!("127.0.0.1:{}", 8000 + i);
                if let Ok(addr) = addr_str.parse::<SocketAddr>() {
                    results.push(addr);
                }
            }
            black_box(results);
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_backend_manager,
    bench_data_structures,
    bench_string_operations
);

criterion_main!(benches);
