use serde::Deserialize;

#[derive(Clone)]
pub struct RegisterInput {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RegisterBody {
    pub email: String,
    pub password: String,
}
