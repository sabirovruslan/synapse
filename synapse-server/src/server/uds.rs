use std::{
    env,
    error::Error,
    fs::{create_dir_all, remove_file},
    path::Path,
};

use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use synapse_core::{CacheCommand, CacheResponce, L1Cache};
use tokio::net::UnixListener;
use tokio_util::{
    codec::{Framed, LengthDelimitedCodec},
    sync::CancellationToken,
};

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
                    let mut framed = Framed::new(stream, LengthDelimitedCodec::new());

                    while let Some(Ok(packet)) = framed.next().await {
                        if let Ok(cmd) = bincode::deserialize::<CacheCommand>(&packet) {
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

                            let reply = bincode::serialize(&response).unwrap();
                            let _ = framed.send(Bytes::from(reply)).await;
                        } else {
                            let response = CacheResponce::Error("Not implemented".into());
                            let reply = bincode::serialize(&response).unwrap();
                            let _ = framed.send(Bytes::from(reply)).await;
                        }
                    }
                });
            }
        }
    }

    Ok(())
}
