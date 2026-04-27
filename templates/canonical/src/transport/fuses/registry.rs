
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::ctx::Ctx;
use crate::transport::fuses::schedule::Schedule;
use crate::meltdown::MeltDown;

pub type FuseFuture = Pin<Box<dyn Future<Output = Result<(), MeltDown>> + Send>>;

pub type FuseFn = Arc<dyn Fn(&Ctx) -> FuseFuture + Send + Sync>;

pub struct Fuse {
    pub name: String,
    pub flow_name: String,
    pub schedule: Schedule,
    pub run_fn: FuseFn,
}

impl Fuse {
    pub fn named(name: impl Into<String>) -> FuseBuilder {
        FuseBuilder {
            name: name.into(),
            schedule: None,
            run_fn: None,
            flow_name: None,
        }
    }
}

pub struct FuseBuilder {
    name: String,
    schedule: Option<Schedule>,
    run_fn: Option<FuseFn>,
    flow_name: Option<String>,
}

impl FuseBuilder {
    pub fn schedule(mut self, schedule: Schedule) -> Self {
        self.schedule = Some(schedule);
        self
    }

    pub fn run<F, Fut>(mut self, f: F) -> Fuse
    where
        F: Fn(&Ctx) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), MeltDown>> + Send + 'static,
    {
        let flow_name = std::any::type_name::<F>().to_string();
        let wrapped: FuseFn = Arc::new(move |ctx: &Ctx| {
            let fut = f(ctx);
            Box::pin(fut) as FuseFuture
        });
        self.run_fn = Some(wrapped);
        self.flow_name = Some(flow_name);
        self.build()
    }

    fn build(self) -> Fuse {
        let schedule = self
            .schedule
            .unwrap_or_else(|| panic!("Fuse '{}' is missing .schedule(...)", self.name));
        let run_fn = self
            .run_fn
            .unwrap_or_else(|| panic!("Fuse '{}' is missing .run(...)", self.name));
        let flow_name = self
            .flow_name
            .unwrap_or_else(|| panic!("Fuse '{}' is missing .run(...)", self.name));
        Fuse {
            name: self.name,
            flow_name,
            schedule,
            run_fn,
        }
    }
}

#[derive(Default)]
pub struct FuseRegistry {
    fuses: Vec<Fuse>,
}

impl FuseRegistry {
    pub fn new() -> Self {
        Self { fuses: Vec::new() }
    }

    pub fn add(&mut self, fuse: Fuse) -> &mut Self {
        if self.fuses.iter().any(|f| f.name == fuse.name) {
            panic!("Fuse '{}' registered twice; names must be unique", fuse.name);
        }
        self.fuses.push(fuse);
        self
    }

    pub fn len(&self) -> usize {
        self.fuses.len()
    }

    pub fn is_empty(&self) -> bool {
        self.fuses.is_empty()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Fuse> {
        self.fuses.iter()
    }

    pub fn into_inner(self) -> Vec<Fuse> {
        self.fuses
    }
}
