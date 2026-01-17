use std::sync::Arc;

use synapse_rust::SynapseClient;

#[derive(Clone)]
pub struct AppState {
    pub synapse_client: Arc<SynapseClient>,
}
