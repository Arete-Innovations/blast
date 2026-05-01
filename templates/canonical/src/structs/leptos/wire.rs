use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ErrorEnvelope {
    pub error: ErrorBody,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ErrorBody {
    #[serde(rename = "type")]
    pub melt_type: String,
    pub message: String,
}
