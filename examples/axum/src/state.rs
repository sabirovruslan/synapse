use std::sync::Arc;

use synapse_rust::SynapseClient;

#[derive(Clone)]
pub struct AppState {
    pub big_file_path: String,
    pub synapse_client: Arc<SynapseClient>,
}
