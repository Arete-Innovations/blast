//! Codegen-time validation: every nav `Entry.route` MUST resolve to a real
//! route emitted by either Primer CRUD or Blueprint pages, and every
//! per-entry roles list MUST be a subset of the route's effective auth.

use std::collections::BTreeMap;

use crate::error::{BlastError, BlastResult};
use crate::state::NavConfig;

use super::resolve::ResolvedRoute;

pub fn validate_nav_against_routes(
    nav: Option<&NavConfig>,
    resolved: &[ResolvedRoute],
) -> BlastResult<()> {
    let nav = match nav {
        Some(n) => n,
        None => return Ok(()),
    };

    let by_name: BTreeMap<&str, &ResolvedRoute> =
        resolved.iter().map(|r| (r.name.as_str(), r)).collect();

    for section in &nav.sections {
        for entry in &section.entries {
            let route = match by_name.get(entry.route.as_str()) {
                Some(r) => r,
                None => {
                    return Err(BlastError::Invalid(format!(
                        "nav entry references unknown route '{}' (section '{}'). \
                         Add a Page entry or enable a Primer verb that emits \
                         this route.",
                        entry.route, section.key
                    )));
                }
            };

            match &entry.roles {
                Some(roles) => {
                    if !route.accepts_role_subset(roles) {
                        return Err(BlastError::Invalid(format!(
                            "nav entry roles {:?} for route '{}' (section '{}') \
                             are not a subset of the route's effective auth {:?}",
                            roles, entry.route, section.key, route.auth,
                        )));
                    }
                }
                None => continue,
            }
        }
    }

    Ok(())
}
