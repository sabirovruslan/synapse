use crate::{
    redis::{client::RedisSync, subscriber::spawn_redis_subscriber},
    server::{grpc::run_grpc, uds::run_uds},
};
use synapse_core::L1Cache;
use tokio_util::sync::CancellationToken;

pub mod redis;
pub mod server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let shutdown = CancellationToken::new();
    let l1_cache = L1Cache::new(10_000);
    let redis_sync = RedisSync::from_env()?;
    if let Some(redis_sync) = redis_sync.clone() {
        spawn_redis_subscriber(l1_cache.clone(), shutdown.clone(), redis_sync);
    }

    let uds_handle = run_uds(l1_cache.clone(), shutdown.clone(), redis_sync);
    let grpc_handle = run_grpc(l1_cache.clone(), shutdown.clone());

    let servers = async { tokio::try_join!(uds_handle, grpc_handle) };
    tokio::pin!(servers);

    tokio::select! {
        res = &mut servers => {
            res?;
        }
        _ = tokio::signal::ctrl_c() => {
            shutdown.cancel();
        }
    };

    servers.await?;

    Ok(())
}
