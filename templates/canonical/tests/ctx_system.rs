use canonical::{
    ctx::{Ctx, CtxPool},
    structs::auth::Role,
};
use diesel_async::{
    pooled_connection::{deadpool::Pool, AsyncDieselConnectionManager},
    AsyncPgConnection,
};

fn dummy_pool() -> CtxPool {
    let cfg = AsyncDieselConnectionManager::<AsyncPgConnection>::new("postgres://localhost/__catablast_unused__");
    Pool::builder(cfg).max_size(1).build().expect("pool build")
}

#[test]
fn system_ctx_has_admin_role() {
    let ctx = Ctx::system(dummy_pool());
    assert_eq!(ctx.role(), Some(Role::Admin));
    assert!(ctx.is_admin(), "system ctx must be admin-equivalent");
}

#[test]
fn system_ctx_passes_require_session() {
    let ctx = Ctx::system(dummy_pool());
    let session = ctx.require_session().expect("system ctx has a session");
    assert_eq!(session.user_id, 0, "sentinel user_id");
    assert_eq!(session.session_id, 0, "sentinel session_id");
    assert_eq!(session.role, Role::Admin);
    assert_eq!(session.token, "", "no token leak surface");
}

#[test]
fn system_ctx_passes_require_admin() {
    let ctx = Ctx::system(dummy_pool());
    ctx.require_admin().expect("system ctx must clear admin gate");
}

#[test]
fn system_ctx_passes_require_any() {
    let ctx = Ctx::system(dummy_pool());
    ctx.require_any(&[Role::Member]).expect_err("Member-only gate must NOT auto-pass for Admin sentinel");
    ctx.require_any(&[Role::Admin]).expect("Admin gate clears");
    ctx.require_any(&[Role::Admin, Role::Member]).expect("multi-role gate clears when Admin in set");
}

#[test]
fn system_ctx_session_user_id_is_zero_sentinel() {
    let ctx = Ctx::system(dummy_pool());
    assert_eq!(ctx.session_user_id(), Some(0));
}

#[test]
fn anonymous_ctx_fails_admin_and_session_gates() {
    let ctx = Ctx::anonymous(dummy_pool());
    assert!(!ctx.is_admin());
    assert_eq!(ctx.role(), None);
    ctx.require_session().expect_err("anonymous must fail require_session");
    ctx.require_admin().expect_err("anonymous must fail require_admin");
}
