use std::collections::{BTreeMap, BTreeSet};

use dialoguer::{theme::ColorfulTheme, Confirm, MultiSelect};

use crate::{
    error::BlastResult,
    schema_parser::ParsedTable,
    state::{
        names::FieldName,
        resource::{FilterKind, ListOptions, ResourceState, Verb},
    },
};

pub fn collect_list_options(table: &ParsedTable, resource: &mut ResourceState) -> BlastResult<()> {
    let Some(list_state) = resource.verbs.get(&Verb::List).cloned() else {
        return Ok(());
    };

    let theme = ColorfulTheme::default();
    let prev = list_state.list_options.clone();
    let prev_paginated = pagination_default(prev.as_ref());

    let paginated = Confirm::with_theme(&theme).with_prompt("List endpoint paginated?").default(prev_paginated).interact()?;

    let column_names: Vec<&str> = table.columns.iter().map(|c| c.name.as_str()).collect();
    let prev_filterable = previous_filterable(prev.as_ref());
    let pre_filterable: Vec<bool> = column_names.iter().map(|n| prev_filterable.contains(*n)).collect();

    let filter_picks = MultiSelect::with_theme(&theme)
        .with_prompt("Filterable columns (?filter[col]=val)")
        .items(&column_names)
        .defaults(&pre_filterable)
        .interact()?;

    let filterable_columns = collect_filterable_map(&column_names, &filter_picks);

    let prev_sortable = previous_sortable(prev.as_ref());
    let pre_sortable: Vec<bool> = column_names.iter().map(|n| prev_sortable.contains(*n)).collect();

    let sort_picks = MultiSelect::with_theme(&theme)
        .with_prompt("Sortable columns (?sort=-col)")
        .items(&column_names)
        .defaults(&pre_sortable)
        .interact()?;

    let sortable_columns = collect_field_names(&column_names, &sort_picks);

    let default_sort = previous_default_sort(prev.as_ref());
    let max_page_size = previous_max_page_size(prev.as_ref());

    let updated = ListOptions {
        paginated,
        filterable_columns,
        sortable_columns,
        default_sort,
        max_page_size,
    };

    let mut new_state = list_state;
    new_state.list_options = Some(updated);
    resource.verbs.insert(Verb::List, new_state);
    Ok(())
}

fn collect_field_names(column_names: &[&str], picks: &[usize]) -> BTreeSet<FieldName> {
    let mut out: BTreeSet<FieldName> = BTreeSet::new();
    for idx in picks {
        let name = column_names.get(*idx);
        match name {
            Some(n) => {
                out.insert(FieldName::new(n.to_string()));
            }
            None => {}
        }
    }
    out
}

/// Collect filterable picks into the v2 `BTreeMap<FieldName, FilterKind>`
/// shape, defaulting every entry to `FilterKind::Eq`. The TUI does not yet
/// prompt for per-column FilterKind — operators get tuned by hand or via a
/// later wizard pass.
fn collect_filterable_map(column_names: &[&str], picks: &[usize]) -> BTreeMap<FieldName, FilterKind> {
    let mut out: BTreeMap<FieldName, FilterKind> = BTreeMap::new();
    for idx in picks {
        let name = column_names.get(*idx);
        match name {
            Some(n) => {
                out.insert(FieldName::new(n.to_string()), FilterKind::Eq);
            }
            None => {}
        }
    }
    out
}

fn pagination_default(prev: Option<&ListOptions>) -> bool {
    let Some(opts) = prev else {
        return true;
    };
    opts.paginated
}

fn previous_filterable(prev: Option<&ListOptions>) -> BTreeSet<String> {
    let Some(opts) = prev else {
        return BTreeSet::new();
    };
    opts.filterable_columns.keys().map(|f| f.as_str().to_string()).collect()
}

fn previous_sortable(prev: Option<&ListOptions>) -> BTreeSet<String> {
    let Some(opts) = prev else {
        return BTreeSet::new();
    };
    opts.sortable_columns.iter().map(|f| f.as_str().to_string()).collect()
}

fn previous_default_sort(prev: Option<&ListOptions>) -> Option<FieldName> {
    let Some(opts) = prev else {
        return None;
    };
    opts.default_sort.clone()
}

fn previous_max_page_size(prev: Option<&ListOptions>) -> Option<u32> {
    let Some(opts) = prev else {
        return None;
    };
    opts.max_page_size
}
