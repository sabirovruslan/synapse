use std::{
    env,
    path::PathBuf,
    sync::{Mutex, OnceLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use bytes::{Buf, BufMut, BytesMut};
use futures::{SinkExt, StreamExt};
use synapse_core::{L1Cache, OP_GET, OP_SET, RES_HIT, RES_OK};
use tokio::net::UnixStream;
use tokio::time::sleep;
use tokio_util::{
    codec::{Framed, LengthDelimitedCodec},
    sync::CancellationToken,
};

static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn unique_socket_path() -> PathBuf {
    let mut path = env::temp_dir();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_nanos(0))
        .as_nanos();
    path.push(format!("synapse-uds-{}-{}.sock", std::process::id(), nanos));
    path
}

fn encode_get(key: &str) -> BytesMut {
    let mut buf = BytesMut::new();
    buf.put_u8(OP_GET);
    buf.put_u32_le(key.len() as u32);
    buf.extend_from_slice(key.as_bytes());
    buf
}

fn encode_set(key: &str, value: &[u8]) -> BytesMut {
    let mut buf = BytesMut::new();
    buf.put_u8(OP_SET);
    buf.put_u32_le(key.len() as u32);
    buf.put_u32_le(value.len() as u32);
    buf.put_u64_le(0);
    buf.extend_from_slice(key.as_bytes());
    buf.extend_from_slice(value);
    buf
}

#[tokio::test]
async fn run_uds_set_get_roundtrip() {
    let _guard = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();

    let socket_path = unique_socket_path();
    let socket_str = socket_path.to_string_lossy().to_string();
    let _ = std::fs::remove_file(&socket_path);
    unsafe {
        env::set_var("SYNAPSE_SOCKET_PATH", &socket_str);
    }

    let cache = L1Cache::new(10);
    let shutdown = CancellationToken::new();
    let mut server_task = tokio::spawn(synapse_server::server::uds::run_uds(
        cache,
        shutdown.clone(),
    ));

    let mut framed = None;
    for _ in 0..100 {
        match UnixStream::connect(&socket_str).await {
            Ok(stream) => {
                framed = Some(Framed::new(
                    stream,
                    LengthDelimitedCodec::builder()
                        .max_frame_length(1024)
                        .new_codec(),
                ));
                break;
            }
            Err(_) => sleep(Duration::from_millis(10)).await,
        }
    }
    let mut framed = framed.expect("server did not accept UDS connection");

    framed
        .send(encode_set("alpha", b"v1").freeze())
        .await
        .unwrap();
    let response = framed.next().await.unwrap().unwrap();
    assert_eq!(response.as_ref(), &[RES_OK]);

    framed.send(encode_get("alpha").freeze()).await.unwrap();
    let response = framed.next().await.unwrap().unwrap();
    let mut buf = &response[..];
    assert_eq!(buf.get_u8(), RES_HIT);
    let len = buf.get_u32_le() as usize;
    let value = &buf[..len];
    assert_eq!(value, b"v1");

    shutdown.cancel();
    if tokio::time::timeout(Duration::from_secs(1), &mut server_task)
        .await
        .is_err()
    {
        server_task.abort();
    }
    let _ = std::fs::remove_file(&socket_path);
    unsafe {
        env::remove_var("SYNAPSE_SOCKET_PATH");
    }
}
