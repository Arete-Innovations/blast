use std::{future::Future, sync::Arc};

use crate::{
    ctx::Ctx,
    meltdown::MeltDown,
    structs::fuses::{
        registry::{Fuse, FuseBuilder, FuseFn, FuseFuture, FuseRegistry},
        schedule::Schedule,
    },
};

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
        let Some(schedule) = self.schedule else {
            panic!("Fuse '{}' is missing .schedule(...)", self.name);
        };
        let Some(run_fn) = self.run_fn else {
            panic!("Fuse '{}' is missing .run(...)", self.name);
        };
        let Some(flow_name) = self.flow_name else {
            panic!("Fuse '{}' is missing .run(...)", self.name);
        };
        Fuse {
            name: self.name,
            flow_name,
            schedule,
            run_fn,
        }
    }
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
