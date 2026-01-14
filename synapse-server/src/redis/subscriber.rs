use std::{error::Error, time::Duration};

use redis::AsyncCommands;
use synapse_core::L1Cache;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

use crate::redis::client::{CacheUpdate, RedisSync};
use futures::StreamExt;

async fn run_subscriber(
    l1_cache: L1Cache,
    shutdown: CancellationToken,
    redis_sync: &RedisSync,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut pubsub = redis_sync.client.get_async_pubsub().await?;
    pubsub.subscribe(&redis_sync.channel).await?;
    let mut stream = pubsub.on_message();

    let mut conn = redis_sync.client.get_multiplexed_async_connection().await?;

    loop {
        tokio::select! {
            _ = shutdown.cancelled() => break,
            msg = stream.next() => {
                let Some(msg) = msg else { break };
                let payload = msg.get_payload_bytes();
                let cache_update: CacheUpdate = match bincode::decode_from_slice::<CacheUpdate, _>(payload, bincode::config::standard()) {
                    Ok((cache_update, _)) => cache_update,
                    Err(err) => {
                        eprintln!("Redis pub/sub decode error: {err}");
                        continue;
                    }
                };

                let prefix_key = redis_sync.prefixed_key(&cache_update.key);
                let value: Option<Vec<u8>> = conn.get(&prefix_key).await?;

                if let Some(bytes) = value {
                    l1_cache.set(cache_update.key.clone(), bytes, cache_update.ttl_secs).await;
                };
            }
        }
    }

    Ok(())
}

pub fn spawn_redis_subscriber(
    l1_cache: L1Cache,
    shutdown: CancellationToken,
    redis_sync: RedisSync,
) {
    tokio::spawn(async move {
        let mut backoff = Duration::from_millis(200);
        loop {
            if shutdown.is_cancelled() {
                break;
            }

            let res = run_subscriber(l1_cache.clone(), shutdown.clone(), &redis_sync).await;

            let failed = match res {
                Ok(()) => {
                    backoff = Duration::from_millis(200);
                    false
                }
                Err(err) => {
                    eprintln!("Redis pub/sub error: {}", err);
                    true
                }
            };

            if failed {
                backoff = (backoff * 2).min(Duration::from_secs(30));
            };

            tokio::select! {
                _ = shutdown.cancelled() => break,
                _ = sleep(backoff) => {},
            }
        }
    });
}
