use criterion::{Criterion, criterion_group, criterion_main};
use std::{
    hint::black_box,
    sync::atomic::{AtomicUsize, Ordering},
};
use synapse_core::L1Cache;
use tokio::runtime::Builder;

fn bench_l1cache_get_hit(c: &mut Criterion) {
    let rt = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("create runtime");
    let cache = L1Cache::new(1024);
    let key = "bench_key".to_string();

    rt.block_on(async {
        cache.set(key.clone(), vec![1u8; 64], None).await;
    });

    c.bench_function("l1cache_get_hit", |b| {
        b.to_async(&rt).iter(|| async {
            let res = cache.get(black_box("bench_key")).await;
            black_box(res);
        });
    });
}

fn bench_l1cache_set(c: &mut Criterion) {
    let rt = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("create runtime");
    let cache = L1Cache::new(1024);
    let keys: Vec<String> = (0..1024).map(|i| format!("key_{:04}", i)).collect();
    let value = vec![1u8; 64];
    let cursor = AtomicUsize::new(0);

    c.bench_function("l1cache_set", |b| {
        b.to_async(&rt).iter(|| async {
            let idx = cursor.fetch_add(1, Ordering::Relaxed) % keys.len();
            cache.set(keys[idx].clone(), value.clone(), None).await;
        });
    });
}

criterion_group!(benches, bench_l1cache_get_hit, bench_l1cache_set);
criterion_main!(benches);
