use crate::structs::auth::SessionContext;
use crate::structs::UserPublic;

pub struct LoginOutput {
    pub token: String,
    pub user: UserPublic,
    pub session: SessionContext,
}
