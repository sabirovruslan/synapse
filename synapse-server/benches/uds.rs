use bytes::{BufMut, BytesMut};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use synapse_core::{CacheResponce, OP_GET, OP_SET};
use synapse_server::server::uds::{decode_command, encode_response};

fn build_get_frame() -> BytesMut {
    let key = "alpha";
    let mut buf = BytesMut::new();
    buf.put_u8(OP_GET);
    buf.put_u32_le(key.len() as u32);
    buf.extend_from_slice(key.as_bytes());
    buf
}

fn build_set_frame(value: &[u8]) -> BytesMut {
    let key = "alpha";
    let mut buf = BytesMut::new();
    buf.put_u8(OP_SET);
    buf.put_u32_le(key.len() as u32);
    buf.put_u32_le(value.len() as u32);
    buf.put_u64_le(0);
    buf.extend_from_slice(key.as_bytes());
    buf.extend_from_slice(value);
    buf
}

fn uds_benches(c: &mut Criterion) {
    let get_frame = build_get_frame();
    c.bench_function("uds_decode_get", |b| {
        b.iter(|| {
            let _ = decode_command(black_box(get_frame.as_ref())).unwrap();
        });
    });

    let set_frame = build_set_frame(b"payload");
    c.bench_function("uds_decode_set", |b| {
        b.iter(|| {
            let _ = decode_command(black_box(set_frame.as_ref())).unwrap();
        });
    });

    let hit = CacheResponce::Hit(b"payload".to_vec());
    c.bench_function("uds_encode_hit", |b| {
        b.iter(|| {
            let _ = encode_response(black_box(hit.clone()));
        });
    });

    let err = CacheResponce::Error("err".into());
    c.bench_function("uds_encode_error", |b| {
        b.iter(|| {
            let _ = encode_response(black_box(err.clone()));
        });
    });

    for size in [1024_usize, 4 * 1024, 16 * 1024, 64 * 1024, 256 * 1024] {
        let value = vec![0xAB; size];
        let set_frame = build_set_frame(&value);
        c.bench_function(&format!("uds_decode_set_{}b", size), |b| {
            b.iter(|| {
                let _ = decode_command(black_box(set_frame.as_ref())).unwrap();
            });
        });

        let hit = CacheResponce::Hit(value.clone());
        c.bench_function(&format!("uds_encode_hit_{}b", size), |b| {
            b.iter(|| {
                let _ = encode_response(black_box(hit.clone()));
            });
        });
    }
}

criterion_group! {
    name = uds;
    config = Criterion::default().sample_size(200).measurement_time(std::time::Duration::from_secs(5));
    targets = uds_benches
}
criterion_main!(uds);
