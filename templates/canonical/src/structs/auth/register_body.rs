use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct RegisterBody {
    pub email: String,
    pub password: String,
}
