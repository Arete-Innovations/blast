use crate::error::{BlastError, BlastResult};
use crate::state::app::{AppState, APP_SCHEMA_VERSION};
use crate::state::resource::{ResourceState, RESOURCE_SCHEMA_VERSION};

type AppUpgrader = fn(&mut AppState) -> BlastResult<()>;
type ResourceUpgrader = fn(&mut ResourceState) -> BlastResult<()>;

const APP_UPGRADERS: &[(u32, AppUpgrader)] = &[];

const RESOURCE_UPGRADERS: &[(u32, ResourceUpgrader)] = &[];

pub fn upgrade_app(state: &mut AppState) -> BlastResult<()> {
    while state.schema_version < APP_SCHEMA_VERSION {
        let from = state.schema_version;
        let entry = APP_UPGRADERS.iter().find(|(v, _)| *v == from);
        let upgrader = match entry {
            Some((_, f)) => f,
            None => {
                return Err(BlastError::Invalid(format!(
                    "no app upgrader registered for schema_version={from}"
                )))
            }
        };
        upgrader(state)?;
        if state.schema_version <= from {
            return Err(BlastError::Invalid(format!(
                "app upgrader for v{from} did not bump schema_version"
            )));
        }
    }
    if state.schema_version > APP_SCHEMA_VERSION {
        return Err(BlastError::Invalid(format!(
            "app schema_version={} newer than supported {}",
            state.schema_version, APP_SCHEMA_VERSION
        )));
    }
    Ok(())
}

pub fn upgrade_resource(state: &mut ResourceState) -> BlastResult<()> {
    while state.schema_version < RESOURCE_SCHEMA_VERSION {
        let from = state.schema_version;
        let entry = RESOURCE_UPGRADERS.iter().find(|(v, _)| *v == from);
        let upgrader = match entry {
            Some((_, f)) => f,
            None => {
                return Err(BlastError::Invalid(format!(
                    "no resource upgrader registered for schema_version={from}"
                )))
            }
        };
        upgrader(state)?;
        if state.schema_version <= from {
            return Err(BlastError::Invalid(format!(
                "resource upgrader for v{from} did not bump schema_version"
            )));
        }
    }
    if state.schema_version > RESOURCE_SCHEMA_VERSION {
        return Err(BlastError::Invalid(format!(
            "resource schema_version={} newer than supported {}",
            state.schema_version, RESOURCE_SCHEMA_VERSION
        )));
    }
    Ok(())
}
