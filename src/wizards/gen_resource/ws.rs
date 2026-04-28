use std::collections::BTreeSet;

use dialoguer::{theme::ColorfulTheme, Confirm, FuzzySelect, MultiSelect};

use crate::{
    error::{BlastError, BlastResult},
    schema_parser::ParsedTable,
    state::{
        names::{AuthScopeField, FieldName},
        resource::{PayloadShape, ResourceState, TopicScope, WsEventsState},
    },
};

const PAYLOAD_LABELS: &[(&str, PayloadShape)] = &[("FullPublicRow", PayloadShape::Public), ("Admin", PayloadShape::Admin), ("IdOnly", PayloadShape::IdOnly)];

const TOPIC_LABELS: &[&str] = &["Global", "PerRow", "ScopedTo(<field>)"];

pub fn collect_ws_events(table: &ParsedTable, resource: &mut ResourceState) -> BlastResult<()> {
    let theme = ColorfulTheme::default();
    let prev = resource.ws_events.clone();
    let prev_enabled = prev.is_some();

    let enable = Confirm::with_theme(&theme).with_prompt("Emit WebSocket events for this resource?").default(prev_enabled).interact()?;

    if !enable {
        resource.ws_events = None;
        return Ok(());
    }

    let column_names: Vec<&str> = table.columns.iter().map(|c| c.name.as_str()).collect();
    let prev_triggers = previous_triggers(prev.as_ref());
    let pre_triggers: Vec<bool> = column_names.iter().map(|n| prev_triggers.contains(*n)).collect();

    let trigger_picks = MultiSelect::with_theme(&theme)
        .with_prompt("Trigger columns (write to any of these emits an event)")
        .items(&column_names)
        .defaults(&pre_triggers)
        .interact()?;

    let trigger_columns = collect_field_names(&column_names, &trigger_picks);

    let prev_payload_idx = previous_payload_index(prev.as_ref());
    let payload_label_strs: Vec<&str> = PAYLOAD_LABELS.iter().map(|(l, _)| *l).collect();
    let payload_idx = FuzzySelect::with_theme(&theme).with_prompt("Payload shape").items(&payload_label_strs).default(prev_payload_idx).interact()?;
    let payload_entry = PAYLOAD_LABELS.get(payload_idx);
    let payload_shape = match payload_entry {
        Some((_, shape)) => *shape,
        None => return Err(BlastError::Invalid(format!("payload FuzzySelect returned out-of-range index {payload_idx}"))),
    };

    let prev_topic_idx = previous_topic_index(prev.as_ref());
    let topic_idx = FuzzySelect::with_theme(&theme).with_prompt("Topic scope").items(TOPIC_LABELS).default(prev_topic_idx).interact()?;
    let topic_scope = match topic_idx {
        0 => TopicScope::Global,
        1 => TopicScope::PerRow,
        2 => prompt_scoped_topic(&theme, &column_names, prev.as_ref())?,
        n => return Err(BlastError::Invalid(format!("topic FuzzySelect returned out-of-range index {n}"))),
    };

    resource.ws_events = Some(WsEventsState {
        trigger_columns,
        payload_shape,
        topic_scope,
    });
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

fn previous_triggers(prev: Option<&WsEventsState>) -> BTreeSet<String> {
    let Some(state) = prev else {
        return BTreeSet::new();
    };
    state.trigger_columns.iter().map(|f| f.as_str().to_string()).collect()
}

fn previous_payload_index(prev: Option<&WsEventsState>) -> usize {
    let Some(state) = prev else {
        return 0;
    };
    payload_index(state.payload_shape)
}

fn previous_topic_index(prev: Option<&WsEventsState>) -> usize {
    let Some(state) = prev else {
        return 0;
    };
    topic_index(&state.topic_scope)
}

fn previous_topic_field(prev: Option<&WsEventsState>) -> Option<String> {
    let Some(state) = prev else {
        return None;
    };
    match &state.topic_scope {
        TopicScope::ScopedTo(f) => Some(f.as_str().to_string()),
        _other => None,
    }
}

fn prompt_scoped_topic(theme: &ColorfulTheme, column_names: &[&str], prev: Option<&WsEventsState>) -> BlastResult<TopicScope> {
    if column_names.is_empty() {
        return Err(BlastError::Invalid("no columns available to scope topic to".to_string()));
    }
    let prev_field = previous_topic_field(prev);
    let target = match prev_field.as_deref() {
        Some(f) => f,
        None => "user_id",
    };
    let default_idx = find_position(column_names, target);
    let idx = FuzzySelect::with_theme(theme).with_prompt("Scope topic to which field?").items(column_names).default(default_idx).interact()?;
    let chosen = column_names.get(idx);
    match chosen {
        Some(name) => Ok(TopicScope::ScopedTo(AuthScopeField::new(name.to_string()))),
        None => Err(BlastError::Invalid(format!("topic-scope FuzzySelect returned out-of-range index {idx}"))),
    }
}

fn find_position(names: &[&str], target: &str) -> usize {
    let mut idx: usize = 0;
    for (i, name) in names.iter().enumerate() {
        if *name == target {
            idx = i;
            break;
        }
    }
    idx
}

fn payload_index(shape: PayloadShape) -> usize {
    match shape {
        PayloadShape::Public => 0,
        PayloadShape::Admin => 1,
        PayloadShape::IdOnly => 2,
    }
}

fn topic_index(scope: &TopicScope) -> usize {
    match scope {
        TopicScope::Global => 0,
        TopicScope::PerRow => 1,
        TopicScope::ScopedTo(_) => 2,
    }
}
