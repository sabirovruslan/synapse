use std::error::Error;

use synapse_core::L1Cache;
use tokio_util::sync::CancellationToken;

pub async fn run_grpc(
    _l1_cache: L1Cache,
    shutdown: CancellationToken,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    tokio::select! {
        _ = shutdown.cancelled() => {
            println!("gRPC server shutdown requested");
        }
    }

    Ok(())
}
