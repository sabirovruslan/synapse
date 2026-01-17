use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, header::CONTENT_TYPE},
    response::IntoResponse,
};
use serde_json::json;
use std::time::Instant;
use tokio::fs;

use crate::{response::SynapseResponse, state::AppState};

pub async fn get_data_from_synapse(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> impl IntoResponse {
    let start = Instant::now();
    let bytes = state.synapse_client.get(&key).await.unwrap();
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    if let Some(bytes) = bytes {
        let duration_ms = start.elapsed().as_secs_f64() * 1000.0;
        println!("[timing] synapse get hit {:.2}ms key={}", duration_ms, key);
        return (headers, bytes).into_response();
    }

    let load_start = Instant::now();
    let file_bytes = fs::read(&state.big_file_path).await.unwrap();
    let load_ms = load_start.elapsed().as_secs_f64() * 1000.0;
    let _ = state.synapse_client.set(&key, file_bytes, None).await;
    let duration_ms = start.elapsed().as_secs_f64() * 1000.0;
    println!(
        "[timing] synapse get miss {:.2}ms (load {:.2}ms) key={}",
        duration_ms, load_ms, key
    );
    (headers, "Set synapse".as_bytes().to_vec()).into_response()
}
