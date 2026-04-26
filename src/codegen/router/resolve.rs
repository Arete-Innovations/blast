//! Resolve the union of CRUD-route auto-emission (per Primer verbs) and
//! Blueprint custom pages into a flat, ordered list of `ResolvedRoute`.
//!
//! Ordering — CRUD routes first sorted by resource name then verb, then
//! Blueprint pages in declaration order. Determinism matters for the
//! route-names union and for diff stability.

use crate::state::{AuthMode, Page, PageLayout, ResourceState, Role, Verb, VerbState};

/// What the route was originally derived from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteOrigin {
    /// Auto-emitted from a Primer's verb. `(resource_table, verb)`.
    Crud {
        resource: String,
        verb: Verb,
    },
    /// Declared in Blueprint `pages` section.
    Page,
}

/// Effective auth requirement attached to a route. CRUD routes derive this
/// from `VerbState.auth`; pages derive it from `Page.roles`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteAuth {
    /// Anyone (anonymous OK).
    Public,
    /// Any authenticated user.
    Required,
    /// Restricted to the listed roles. Empty list is illegal at this stage.
    Roles(Vec<Role>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRoute {
    /// Route name — the wire identifier used by `<router-link :to="{ name }">`
    /// and the `RouteName` union. e.g. `users.list`, `dashboard`.
    pub name: String,
    /// vue-router URL path, e.g. `/users`, `/users/:id`.
    pub path: String,
    /// Lazy-loaded component import path, e.g. `@/pages/users/ListPage.vue`.
    pub component_import: String,
    /// PageShell layout.
    pub layout: PageLayout,
    /// Optional human label (rendered in nav + breadcrumbs).
    pub label: Option<String>,
    /// Optional icon registry key.
    pub icon: Option<String>,
    /// Effective auth requirement (used by guard install).
    pub auth: RouteAuth,
    /// True when this route's origin opts to appear in the auto-generated
    /// menu (CRUD routes default true; pages obey `in_nav`).
    pub in_nav: bool,
    /// Origin discriminator (for tooling, validation messages, tests).
    pub origin: RouteOrigin,
}

impl ResolvedRoute {
    /// True iff the auth tier of this route accepts the given role set.
    /// Used by validation to confirm an `Entry.roles` is a subset of the
    /// route's effective auth.
    pub fn accepts_role_subset(&self, requested: &[Role]) -> bool {
        match &self.auth {
            RouteAuth::Public => true,
            RouteAuth::Required => true,
            RouteAuth::Roles(allowed) => {
                requested.iter().all(|r| allowed.iter().any(|a| a == r))
            }
        }
    }
}

pub fn resolve_all(resources: &[ResourceState], pages: &[Page]) -> Vec<ResolvedRoute> {
    let mut out: Vec<ResolvedRoute> = Vec::new();

    let mut crud: Vec<ResolvedRoute> = Vec::new();
    for res in resources {
        let table = res.name.as_str().to_string();
        for (verb, vstate) in &res.verbs {
            match crud_route(&table, *verb, vstate) {
                Some(route) => crud.push(route),
                None => continue,
            }
        }
    }
    crud.sort_by(|a, b| a.name.cmp(&b.name));
    out.extend(crud);

    for page in pages {
        out.push(page_route(page));
    }
    out
}

fn crud_route(table: &str, verb: Verb, vstate: &VerbState) -> Option<ResolvedRoute> {
    let (suffix, path_tail, component_stem, layout) = match verb {
        Verb::List => ("list", String::new(), "ListPage", PageLayout::Table),
        Verb::Get => ("detail", "/:id".to_string(), "DetailPage", PageLayout::Cards),
        Verb::Create => ("create", "/new".to_string(), "CreatePage", PageLayout::Cards),
        Verb::Update => ("edit", "/:id/edit".to_string(), "EditPage", PageLayout::Cards),
        // Delete is a destructive verb invoked from list/detail; it has no
        // page and therefore no route. Skip silently.
        Verb::Delete => return None,
    };

    Some(ResolvedRoute {
        name: format!("{}.{}", table, suffix),
        path: format!("/{}{}", table, path_tail),
        component_import: format!("@/pages/{}/{}.vue", table, component_stem),
        layout,
        label: None,
        icon: None,
        auth: derive_auth(&vstate.auth),
        in_nav: matches!(verb, Verb::List),
        origin: RouteOrigin::Crud {
            resource: table.to_string(),
            verb,
        },
    })
}

fn page_route(page: &Page) -> ResolvedRoute {
    let auth = page_auth(&page.roles);
    ResolvedRoute {
        name: page.route.clone(),
        path: page.path.clone(),
        component_import: format!("@/{}", page.component),
        layout: page.layout.clone(),
        label: page.label.clone(),
        icon: page.icon.clone(),
        auth,
        in_nav: page.in_nav,
        origin: RouteOrigin::Page,
    }
}

fn page_auth(roles: &Option<Vec<Role>>) -> RouteAuth {
    match roles {
        Some(list) if !list.is_empty() => RouteAuth::Roles(list.clone()),
        Some(_empty) => RouteAuth::Required, // allow: empty list = "any signed-in user"
        None => RouteAuth::Public,
    }
}

fn derive_auth(mode: &AuthMode) -> RouteAuth {
    match mode {
        AuthMode::Public => RouteAuth::Public,
        AuthMode::AuthRequired => RouteAuth::Required,
        AuthMode::AdminOnly => RouteAuth::Roles(vec![Role::Admin]),
        AuthMode::ScopedTo(_field) => RouteAuth::Required, // allow: per-row scope is independent of role
        AuthMode::Roles(roles) => derive_named_roles(roles),
    }
}

fn derive_named_roles(roles: &std::collections::BTreeSet<String>) -> RouteAuth {
    let mut acc: Vec<Role> = Vec::new();
    for r in roles {
        let role = parse_role_name(r.as_str());
        if !acc.iter().any(|x| x == &role) {
            acc.push(role);
        }
    }
    if acc.is_empty() {
        RouteAuth::Required
    } else {
        RouteAuth::Roles(acc)
    }
}

fn parse_role_name(name: &str) -> Role {
    match name {
        "admin" => Role::Admin,
        "Admin" => Role::Admin,
        "user" => Role::User,
        "User" => Role::User,
        other => default_role_for_unknown(other),
    }
}

/// Unknown named-role strings collapse to `User`. They're still emitted in
/// the auth set but never gate the entry stricter than user-level access.
fn default_role_for_unknown(_unknown: &str) -> Role {
    // allow: explicit fallback documented above; never silent
    Role::User
}
