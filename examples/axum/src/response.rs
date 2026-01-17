use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynapseResponse {
    pub data: Option<Vec<u8>>,
}
