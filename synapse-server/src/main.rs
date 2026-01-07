use std::path::Path;

use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use synapse_core::{CacheCommand, CacheResponce, L1Cache};
use tokio::net::UnixListener;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let socket_path = "/tmp/synapse.sock";

    if Path::new(socket_path).exists() {
        std::fs::remove_file(socket_path)?;
    }

    let listener = UnixListener::bind(socket_path)?;

    println!("🚀 Synapse Server started on UDS: {}", socket_path);

    let l1_cache = L1Cache::new(10_000);

    loop {
        let (stream, _) = listener.accept().await?;
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
