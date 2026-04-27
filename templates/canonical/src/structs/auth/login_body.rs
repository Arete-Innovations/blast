use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct LoginBody {
    pub email: String,
    pub password: String,
}
