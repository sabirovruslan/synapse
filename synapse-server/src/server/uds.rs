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
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::UnixListener;
use tokio_util::{
    codec::{Framed, LengthDelimitedCodec},
    sync::CancellationToken,
};

use crate::redis::client::RedisSync;

pub fn decode_command(mut buf: &[u8]) -> Result<CacheCommand, String> {
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

pub fn encode_response(response: CacheResponce) -> Bytes {
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

async fn handle_uds_stream<S>(stream: S, l1_cache: L1Cache, redis_sync: Option<RedisSync>)
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let mut framed = Framed::new(
        stream,
        LengthDelimitedCodec::builder()
            .max_frame_length(MAX_FRAME_LENGTH)
            .new_codec(),
    );

    while let Some(Ok(packet)) = framed.next().await {
        if let Ok(cmd) = decode_command(&packet) {
            let response = match cmd {
                CacheCommand::Get { key } => l1_cache.get(&key).await,
                CacheCommand::Set {
                    key,
                    value,
                    ttl_secs,
                } => {
                    if let Some(redis_sync) = redis_sync.as_ref() {
                        l1_cache.set(key.clone(), value.clone(), ttl_secs).await;
                        if let Err(err) = redis_sync.set(&key, &value, ttl_secs).await {
                            eprintln!("Redis write failed: {}", err);
                        }
                    } else {
                        l1_cache.set(key, value, ttl_secs).await;
                    }
                    CacheResponce::Ok
                }
            };

            let _ = framed.send(encode_response(response)).await;
        } else {
            let response = CacheResponce::Error("Not implemented".into());
            let _ = framed.send(encode_response(response)).await;
        }
    }
}

pub async fn run_uds(
    l1_cache: L1Cache,
    shutdown: CancellationToken,
    redis_sync: Option<RedisSync>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
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
                let redis_sync_clone = redis_sync.clone();

                tokio::spawn(async move {
                    handle_uds_stream(stream, l1_cache_clone, redis_sync_clone).await;
                });
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{decode_command, encode_response, handle_uds_stream};
    use bytes::{Buf, BufMut, BytesMut};
    use futures::{SinkExt, StreamExt};
    use synapse_core::{
        CacheCommand, CacheResponce, L1Cache, OP_GET, OP_SET, RES_ERR, RES_HIT, RES_MISS, RES_OK,
    };
    use tokio::io::duplex;
    use tokio_util::codec::{Framed, LengthDelimitedCodec};

    #[test]
    fn decode_command_get_ok() {
        let key = "alpha";
        let mut buf = BytesMut::new();
        buf.put_u8(OP_GET);
        buf.put_u32_le(key.len() as u32);
        buf.extend_from_slice(key.as_bytes());

        let cmd = decode_command(&buf).expect("decode get");
        assert!(matches!(cmd, CacheCommand::Get { key: k } if k == key));
    }

    #[test]
    fn decode_command_get_ok_wit_utf_8() {
        let key = "@alpha#1!&";
        let mut buf = BytesMut::new();
        buf.put_u8(OP_GET);
        buf.put_u32_le(key.len() as u32);
        buf.extend_from_slice(key.as_bytes());

        let cmd = decode_command(&buf).expect("decode get");
        assert!(matches!(cmd, CacheCommand::Get { key: k } if k == key));
    }

    #[test]
    fn decode_command_set_ok_without_ttl() {
        let key = "k1";
        let value = b"value";
        let mut buf = BytesMut::new();
        buf.put_u8(OP_SET);
        buf.put_u32_le(key.len() as u32);
        buf.put_u32_le(value.len() as u32);
        buf.put_u64_le(0);
        buf.extend_from_slice(key.as_bytes());
        buf.extend_from_slice(value);

        let cmd = decode_command(&buf).expect("decode set");
        assert!(matches!(
            cmd,
            CacheCommand::Set {
                key: k,
                value: v,
                ttl_secs: None,
            } if k == key && v == value
        ));
    }

    #[test]
    fn decode_command_set_ok_with_ttl() {
        let key = "k2";
        let value = b"payload";
        let ttl = 42_u64;
        let mut buf = BytesMut::new();
        buf.put_u8(OP_SET);
        buf.put_u32_le(key.len() as u32);
        buf.put_u32_le(value.len() as u32);
        buf.put_u64_le(ttl);
        buf.extend_from_slice(key.as_bytes());
        buf.extend_from_slice(value);

        let cmd = decode_command(&buf).expect("decode set ttl");
        assert!(matches!(
            cmd,
            CacheCommand::Set {
                key: k,
                value: v,
                ttl_secs: Some(t),
            } if k == key && v == value && t == ttl
        ));
    }

    #[test]
    fn decode_command_empty_buf() {
        let err = decode_command(&[]).expect_err("empty buf should error");
        assert_eq!(err, "Buf is empty");
    }

    #[test]
    fn decode_command_bad_key_len() {
        let mut buf = BytesMut::new();
        buf.put_u8(OP_GET);
        buf.put_u32_le(10);
        buf.extend_from_slice(b"abc");

        let err = decode_command(&buf).expect_err("bad key_len should error");
        assert_eq!(err, "Bad key_len");
    }

    #[test]
    fn decode_command_bad_lengths() {
        let mut buf = BytesMut::new();
        buf.put_u8(OP_SET);
        buf.put_u32_le(1);
        buf.put_u32_le(5);
        buf.put_u64_le(0);
        buf.extend_from_slice(b"k");
        buf.extend_from_slice(b"v");

        let err = decode_command(&buf).expect_err("bad lengths should error");
        assert_eq!(err, "Bad lenghts");
    }

    #[test]
    fn decode_command_unknown_op() {
        let buf = [0xFF];
        let err = decode_command(&buf).expect_err("unknown op should error");
        assert_eq!(err, "Unknown op");
    }

    #[test]
    fn encode_response_ok() {
        let out = encode_response(CacheResponce::Ok);
        assert_eq!(out.as_ref(), &[RES_OK]);
    }

    #[test]
    fn encode_response_miss() {
        let out = encode_response(CacheResponce::Miss);
        assert_eq!(out.as_ref(), &[RES_MISS]);
    }

    #[test]
    fn encode_response_hit() {
        let val = b"data".to_vec();
        let out = encode_response(CacheResponce::Hit(val.clone()));

        let mut expected = BytesMut::new();
        expected.put_u8(RES_HIT);
        expected.put_u32_le(val.len() as u32);
        expected.extend_from_slice(&val);

        assert_eq!(out.as_ref(), expected.as_ref());
    }

    #[test]
    fn encode_response_error() {
        let msg = "oops";
        let out = encode_response(CacheResponce::Error(msg.into()));

        let mut expected = BytesMut::new();
        expected.put_u8(RES_ERR);
        expected.put_u32_le(msg.len() as u32);
        expected.extend_from_slice(msg.as_bytes());

        assert_eq!(out.as_ref(), expected.as_ref());
    }

    #[tokio::test]
    async fn handle_uds_stream_decode_error_sends_error() {
        let cache = L1Cache::new(10);
        let (client, server) = duplex(1024);

        tokio::spawn(handle_uds_stream(server, cache, None));

        let mut framed = Framed::new(
            client,
            LengthDelimitedCodec::builder()
                .max_frame_length(1024)
                .new_codec(),
        );

        framed
            .send(BytesMut::from(&[0xFF][..]).freeze())
            .await
            .unwrap();
        let response = framed.next().await.unwrap().unwrap();

        let mut buf = &response[..];
        assert_eq!(buf.get_u8(), RES_ERR);
        let msg_len = buf.get_u32_le() as usize;
        let msg = std::str::from_utf8(&buf[..msg_len]).unwrap();
        assert_eq!(msg, "Not implemented");
    }
}
