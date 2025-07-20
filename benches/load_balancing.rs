use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("hash_consistent", |b| {
        b.iter(|| {
            // TODO: Implement consistent hashing benchmark
            black_box(());
        })
    });

    c.bench_function("round_robin", |b| {
        b.iter(|| {
            // TODO: Implement round robin benchmark
            black_box(());
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
