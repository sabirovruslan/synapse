use axum::{
    Json,
    extract::{Path, State},
};
use tokio::fs;

use crate::{response::SynapseResponse, state::AppState};

pub async fn get_data_from_synapse(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Json<SynapseResponse> {
    let bytes = state.synapse_client.get(&key).await.unwrap();
    if bytes.is_none() {
        let file_bytes = fs::read(&state.big_file_path).await.unwrap();
        let _ = state.synapse_client.set(&key, file_bytes, None).await;
        return Json(SynapseResponse {
            data: Some("set to synapse".as_bytes().to_vec()),
        });
    }

    Json(SynapseResponse { data: bytes })
}
