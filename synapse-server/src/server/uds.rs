use std::{
    env,
    error::Error,
    fs::{create_dir_all, remove_file},
    path::Path,
};

use bytes::{Buf, BufMut, Bytes, BytesMut};
use futures::{SinkExt, StreamExt};
use synapse_core::{
    CacheCommand, CacheResponce, L1Cache, MAX_FRAME_LENGTH, OP_GET, OP_SET, RES_ERR, RES_HIT,
    RES_MISS, RES_OK,
};
use tokio::net::UnixListener;
use tokio_util::{
    codec::{Framed, LengthDelimitedCodec},
    sync::CancellationToken,
};

fn decode_command(mut buf: &[u8]) -> Result<CacheCommand, String> {
    if !buf.has_remaining() {
        return Err("Buf is empty".into());
    }

    let op = buf.get_u8();
    match op {
        OP_GET => {
            let key_len = buf.get_u32_le() as usize;
            if buf.remaining() < key_len {
                return Err("Bad key_len".into());
            }
            let key = String::from_utf8(buf.copy_to_bytes(key_len).to_vec())
                .map_err(|e| format!("Bad key utf-8: {}", e))?;
            Ok(CacheCommand::Get { key })
        }
        OP_SET => {
            let key_len = buf.get_u32_le() as usize;
            let value_len = buf.get_u32_le() as usize;
            let ttl_raw = buf.get_u64_le();
            if buf.remaining() < key_len + value_len {
                return Err("Bad lenghts".into());
            }
            let key = String::from_utf8(buf.copy_to_bytes(key_len).to_vec())
                .map_err(|e| format!("Bad key utf-8: {}", e))?;
            let value = buf.copy_to_bytes(value_len).to_vec();
            let ttl_secs = if ttl_raw == 0 { None } else { Some(ttl_raw) };
            Ok(CacheCommand::Set {
                key,
                value,
                ttl_secs,
            })
        }
        _ => Err("Unknown op".into()),
    }
}

fn encode_response(response: CacheResponce) -> Bytes {
    let mut out = BytesMut::new();
    match response {
        CacheResponce::Ok => out.put_u8(RES_OK),
        CacheResponce::Miss => out.put_u8(RES_MISS),
        CacheResponce::Hit(val) => {
            out.put_u8(RES_HIT);
            out.put_u32_le(val.len() as u32);
            out.extend_from_slice(&val);
        }
        CacheResponce::Error(e) => {
            out.put_u8(RES_ERR);
            out.put_u32_le(e.len() as u32);
            out.extend_from_slice(e.as_bytes());
        }
    }
    out.freeze()
}

pub async fn run_uds(l1_cache: L1Cache, shutdown: CancellationToken) -> Result<(), Box<dyn Error>> {
    let socket_path =
        env::var("SYNAPSE_SOCKET_PATH").unwrap_or_else(|_| "/tmp/synapse.sock".to_string());
    if let Some(parent) = Path::new(&socket_path).parent() {
        create_dir_all(parent)?;
    }
    if Path::new(&socket_path).exists() {
        remove_file(&socket_path)?;
    }

    let listener = UnixListener::bind(&socket_path)?;

    println!("Synapse Server started on UDS: {}", socket_path);

    loop {
        tokio::select! {
            _ = shutdown.cancelled() => {
                println!("UDS server shutdown requested");
                break;
            }
            accept_res = listener.accept() => {
                let (stream, _) = accept_res?;
                let l1_cache_clone = l1_cache.clone();

                tokio::spawn(async move {
                    let mut framed = Framed::new(
                        stream,
                        LengthDelimitedCodec::builder()
                            .max_frame_length(MAX_FRAME_LENGTH)
                            .new_codec(),
                    );

                    while let Some(Ok(packet)) = framed.next().await {
                        if let Ok(cmd) = decode_command(&packet) {
                            let response = match cmd {
                                CacheCommand::Get { key } => l1_cache_clone.get(&key).await,
                                CacheCommand::Set {
                                    key,
                                    value,
                                    ttl_secs,
                                } => {
                                    l1_cache_clone.set(key, value, ttl_secs).await;
                                    CacheResponce::Ok
                                }
                            };

                            let _ = framed.send(encode_response(response)).await;
                        } else {
                            let response = CacheResponce::Error("Not implemented".into());
                            let _ = framed.send(encode_response(response)).await;
                        }
                    }
                });
            }
        }
    }

    Ok(())
}
