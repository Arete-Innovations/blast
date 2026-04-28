use std::collections::BTreeSet;

use dialoguer::{theme::ColorfulTheme, Confirm, FuzzySelect, Input, MultiSelect};
use indexmap::IndexMap;

use crate::{
    error::{BlastError, BlastResult},
    schema_parser::ParsedTable,
    state::{
        names::AuthScopeField,
        resource::{AuthMode, ResourceState, Verb, VerbState},
    },
};

const ALL_VERBS: &[Verb] = &[Verb::List, Verb::Get, Verb::Create, Verb::Update, Verb::Delete];

const AUTH_LABELS: &[&str] = &["Public", "AuthRequired", "AdminOnly", "ScopedTo(<field>)", "Roles([..])"];

pub fn collect_verbs(table: &ParsedTable, resource: &mut ResourceState) -> BlastResult<()> {
    let theme = ColorfulTheme::default();
    let mut new_verbs: IndexMap<Verb, VerbState> = IndexMap::new();

    for verb in ALL_VERBS {
        let previous = resource.verbs.get(verb).cloned();
        let was_enabled = previous.is_some();

        let enable = Confirm::with_theme(&theme).with_prompt(format!("Enable verb `{}`?", verb_label(verb))).default(was_enabled).interact()?;
        if !enable {
            continue;
        }

        let prev_state = previous;
        let auth = prompt_auth_mode(&theme, verb, table, prev_state.as_ref())?;
        let list_options = carry_list_options(prev_state);

        new_verbs.insert(*verb, VerbState { auth, list_options });
    }

    resource.verbs = new_verbs;
    Ok(())
}

fn prompt_auth_mode(theme: &ColorfulTheme, verb: &Verb, table: &ParsedTable, previous: Option<&VerbState>) -> BlastResult<AuthMode> {
    let default_idx = previous_auth_default(previous);
    let idx = FuzzySelect::with_theme(theme)
        .with_prompt(format!("Auth for `{}`", verb_label(verb)))
        .items(AUTH_LABELS)
        .default(default_idx)
        .interact()?;

    match idx {
        0 => Ok(AuthMode::Public),
        1 => Ok(AuthMode::AuthRequired),
        2 => Ok(AuthMode::AdminOnly),
        3 => prompt_scoped_to(theme, table, previous),
        4 => prompt_roles(theme, previous),
        n => Err(BlastError::Invalid(format!("auth FuzzySelect returned out-of-range index {n}"))),
    }
}

fn auth_label_index(auth: &AuthMode) -> usize {
    match auth {
        AuthMode::Public => 0,
        AuthMode::AuthRequired => 1,
        AuthMode::AdminOnly => 2,
        AuthMode::ScopedTo(_) => 3,
        AuthMode::Roles(_) => 4,
    }
}

fn prompt_scoped_to(theme: &ColorfulTheme, table: &ParsedTable, previous: Option<&VerbState>) -> BlastResult<AuthMode> {
    let names: Vec<&str> = table.columns.iter().map(|c| c.name.as_str()).collect();
    if names.is_empty() {
        return Err(BlastError::Invalid("no columns available to scope auth to".to_string()));
    }
    let prev_field = previous_scoped_field(previous);
    let target = match prev_field.as_deref() {
        Some(f) => f,
        None => "user_id",
    };
    let default_idx = position_or_zero(&names, target);

    let idx = FuzzySelect::with_theme(theme).with_prompt("Scope auth to which field?").items(&names).default(default_idx).interact()?;
    let chosen = names.get(idx);
    match chosen {
        Some(name) => Ok(AuthMode::ScopedTo(AuthScopeField::new(name.to_string()))),
        None => Err(BlastError::Invalid(format!("scope FuzzySelect returned out-of-range index {idx}"))),
    }
}

fn prompt_roles(theme: &ColorfulTheme, previous: Option<&VerbState>) -> BlastResult<AuthMode> {
    let prev_roles = previous_roles(previous);

    let known = known_role_palette(&prev_roles);
    let pre_selected: Vec<bool> = known.iter().map(|r| prev_roles.contains(r)).collect();

    let picks = MultiSelect::with_theme(theme)
        .with_prompt("Select roles allowed (space toggles, enter confirms)")
        .items(&known)
        .defaults(&pre_selected)
        .interact()?;

    let mut chosen: BTreeSet<String> = BTreeSet::new();
    for idx in picks {
        let role = known.get(idx);
        match role {
            Some(name) => {
                chosen.insert(name.clone());
            }
            None => {}
        }
    }

    let extra: String = Input::with_theme(theme).with_prompt("Additional roles (comma-separated, leave empty to skip)").allow_empty(true).interact_text()?;
    for raw in extra.split(',') {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            chosen.insert(trimmed.to_string());
        }
    }

    if chosen.is_empty() {
        return Err(BlastError::Invalid("Roles auth mode requires at least one role".to_string()));
    }

    Ok(AuthMode::Roles(chosen))
}

fn known_role_palette(prev_roles: &BTreeSet<String>) -> Vec<String> {
    let mut roles: BTreeSet<String> = ["admin", "staff", "moderator", "user"].iter().map(|s| s.to_string()).collect();
    for role in prev_roles {
        roles.insert(role.clone());
    }
    roles.into_iter().collect()
}

fn position_or_zero(names: &[&str], target: &str) -> usize {
    let mut idx: usize = 0;
    for (i, name) in names.iter().enumerate() {
        if *name == target {
            idx = i;
            break;
        }
    }
    idx
}

fn carry_list_options(prev: Option<VerbState>) -> Option<crate::state::resource::ListOptions> {
    let Some(state) = prev else {
        return None;
    };
    state.list_options
}

fn previous_auth_default(previous: Option<&VerbState>) -> usize {
    let Some(state) = previous else {
        return 0;
    };
    auth_label_index(&state.auth)
}

fn previous_scoped_field(previous: Option<&VerbState>) -> Option<String> {
    let Some(state) = previous else {
        return None;
    };
    match &state.auth {
        AuthMode::ScopedTo(f) => Some(f.as_str().to_string()),
        _other => None,
    }
}

fn previous_roles(previous: Option<&VerbState>) -> BTreeSet<String> {
    let Some(state) = previous else {
        return BTreeSet::new();
    };
    match &state.auth {
        AuthMode::Roles(roles) => roles.clone(),
        _other => BTreeSet::new(),
    }
}

pub fn verb_label(verb: &Verb) -> &'static str {
    match verb {
        Verb::List => "List",
        Verb::Get => "Get",
        Verb::Create => "Create",
        Verb::Update => "Update",
        Verb::Delete => "Delete",
    }
}
