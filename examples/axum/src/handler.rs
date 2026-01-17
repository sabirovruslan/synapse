use axum::{
    Json,
    extract::{Path, State},
};

use crate::{response::SynapseResponse, state::AppState};

pub async fn get_data_from_synapse(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Json<SynapseResponse> {
    let bytes = state.synapse_client.get(&key).await.unwrap();
    if bytes.is_none() {}

    Json(SynapseResponse { data: bytes })
}
