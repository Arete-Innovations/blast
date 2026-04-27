
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::ctx::Ctx;
use crate::meltdown::MeltDown;
use crate::structs::fuses::schedule::Schedule;

pub type FuseFuture = Pin<Box<dyn Future<Output = Result<(), MeltDown>> + Send>>;

pub type FuseFn = Arc<dyn Fn(&Ctx) -> FuseFuture + Send + Sync>;

pub struct Fuse {
    pub name: String,
    pub flow_name: String,
    pub schedule: Schedule,
    pub run_fn: FuseFn,
}

pub struct FuseBuilder {
    pub name: String,
    pub(crate) schedule: Option<Schedule>,
    pub(crate) run_fn: Option<FuseFn>,
    pub(crate) flow_name: Option<String>,
}

#[derive(Default)]
pub struct FuseRegistry {
    pub(crate) fuses: Vec<Fuse>,
}
