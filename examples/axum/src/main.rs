use std::{env, sync::Arc};

use axum::{Router, routing::get};
use dotenv::dotenv;
use synapse_rust::SynapseClient;

use crate::state::AppState;

mod handler;
mod response;
mod state;

#[tokio::main]
async fn main() {
    dotenv().ok();
    let socket_path =
        env::var("SYNAPSE_SOCKET_PATH").unwrap_or_else(|_| "/tmp/synapse.sock".to_string());
    let synapse_client = SynapseClient::new(socket_path).await.unwrap();
    let state = AppState {
        synapse_client: Arc::new(synapse_client),
    };

    let app = Router::new()
        .route("/", get(|| async { "example synapse" }))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();

    println!(
        "Server listening on http://{}",
        listener.local_addr().unwrap()
    );

    axum::serve(listener, app).await.unwrap();
}
