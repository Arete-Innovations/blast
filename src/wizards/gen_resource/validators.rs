use std::collections::BTreeSet;

use dialoguer::{theme::ColorfulTheme, Input, MultiSelect};

use crate::{
    error::{BlastError, BlastResult},
    state::{FieldName, ResourceState, ValidatorRule},
};

const RULE_LABELS: &[&str] = &["Required", "MinLen(n)", "MaxLen(n)", "MinValue(n)", "MaxValue(n)", "Pattern(re)", "OneOf([..])", "Email", "Url"];

pub fn collect_validators(resource: &mut ResourceState) -> BlastResult<()> {
    let theme = ColorfulTheme::default();
    let names: Vec<FieldName> = resource.fields.keys().cloned().collect();
    for name in names {
        let field = match resource.fields.get(&name) {
            Some(f) => f.clone(),
            None => continue,
        };
        if field.primary_key {
            continue;
        }

        let rules = prompt_rules_for_field(&theme, name.as_str(), &field.validators)?;

        match resource.fields.get_mut(&name) {
            Some(slot) => {
                slot.validators = rules;
            }
            None => continue,
        }
    }
    Ok(())
}

fn prompt_rules_for_field(theme: &ColorfulTheme, field: &str, previous: &BTreeSet<ValidatorRule>) -> BlastResult<BTreeSet<ValidatorRule>> {
    let pre_selected: Vec<bool> = RULE_LABELS
        .iter()
        .map(|label| match *label {
            "Required" => previous.contains(&ValidatorRule::Required),
            "MinLen(n)" => previous.iter().any(|r| matches!(r, ValidatorRule::MinLen(_))),
            "MaxLen(n)" => previous.iter().any(|r| matches!(r, ValidatorRule::MaxLen(_))),
            "MinValue(n)" => previous.iter().any(|r| matches!(r, ValidatorRule::MinValue(_))),
            "MaxValue(n)" => previous.iter().any(|r| matches!(r, ValidatorRule::MaxValue(_))),
            "Pattern(re)" => previous.iter().any(|r| matches!(r, ValidatorRule::Pattern(_))),
            "OneOf([..])" => previous.iter().any(|r| matches!(r, ValidatorRule::OneOf(_))),
            "Email" => previous.contains(&ValidatorRule::Email),
            "Url" => previous.contains(&ValidatorRule::Url),
            _other => false,
        })
        .collect();

    let prompt = format!("Validators for field `{field}`");
    let picks = MultiSelect::with_theme(theme).with_prompt(prompt).items(RULE_LABELS).defaults(&pre_selected).interact()?;

    let mut chosen: BTreeSet<ValidatorRule> = BTreeSet::new();
    for idx in picks {
        let label = match RULE_LABELS.get(idx) {
            Some(l) => *l,
            None => continue,
        };
        let rule = build_rule_from_label(theme, field, label, previous)?;
        chosen.insert(rule);
    }
    Ok(chosen)
}

fn build_rule_from_label(theme: &ColorfulTheme, field: &str, label: &str, previous: &BTreeSet<ValidatorRule>) -> BlastResult<ValidatorRule> {
    match label {
        "Required" => Ok(ValidatorRule::Required),
        "Email" => Ok(ValidatorRule::Email),
        "Url" => Ok(ValidatorRule::Url),
        "MinLen(n)" => {
            let default = previous.iter().find_map(|r| match r {
                ValidatorRule::MinLen(n) => Some(*n),
                _other => None,
            });
            let n = prompt_usize(theme, &format!("MinLen for `{field}`"), default)?;
            Ok(ValidatorRule::MinLen(n))
        }
        "MaxLen(n)" => {
            let default = previous.iter().find_map(|r| match r {
                ValidatorRule::MaxLen(n) => Some(*n),
                _other => None,
            });
            let n = prompt_usize(theme, &format!("MaxLen for `{field}`"), default)?;
            Ok(ValidatorRule::MaxLen(n))
        }
        "MinValue(n)" => {
            let default = previous.iter().find_map(|r| match r {
                ValidatorRule::MinValue(n) => Some(*n),
                _other => None,
            });
            let n = prompt_i64(theme, &format!("MinValue for `{field}`"), default)?;
            Ok(ValidatorRule::MinValue(n))
        }
        "MaxValue(n)" => {
            let default = previous.iter().find_map(|r| match r {
                ValidatorRule::MaxValue(n) => Some(*n),
                _other => None,
            });
            let n = prompt_i64(theme, &format!("MaxValue for `{field}`"), default)?;
            Ok(ValidatorRule::MaxValue(n))
        }
        "Pattern(re)" => {
            let default = previous.iter().find_map(|r| match r {
                ValidatorRule::Pattern(s) => Some(s.clone()),
                _other => None,
            });
            let pat = prompt_pattern(theme, &format!("Pattern regex for `{field}`"), default)?;
            Ok(ValidatorRule::Pattern(pat))
        }
        "OneOf([..])" => {
            let default = previous.iter().find_map(|r| match r {
                ValidatorRule::OneOf(values) => Some(values.join(", ")),
                _other => None,
            });
            let raw = prompt_string(theme, &format!("OneOf values (comma-separated) for `{field}`"), default)?;
            let values: Vec<String> = raw.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
            if values.is_empty() {
                return Err(BlastError::Invalid("OneOf requires at least one value".to_string()));
            }
            Ok(ValidatorRule::OneOf(values))
        }
        other => Err(BlastError::Invalid(format!("unrecognized validator label: {other}"))),
    }
}

fn prompt_usize(theme: &ColorfulTheme, prompt: &str, default: Option<usize>) -> BlastResult<usize> {
    let mut input = Input::<String>::with_theme(theme).with_prompt(prompt);
    match default {
        Some(d) => input = input.default(d.to_string()),
        None => {}
    }
    let raw = input.interact_text()?;
    let parsed: usize = raw.trim().parse().map_err(|e| BlastError::Invalid(format!("expected non-negative integer, got `{}`: {}", raw, e)))?;
    Ok(parsed)
}

fn prompt_i64(theme: &ColorfulTheme, prompt: &str, default: Option<i64>) -> BlastResult<i64> {
    let mut input = Input::<String>::with_theme(theme).with_prompt(prompt);
    match default {
        Some(d) => input = input.default(d.to_string()),
        None => {}
    }
    let raw = input.interact_text()?;
    let parsed: i64 = raw.trim().parse().map_err(|e| BlastError::Invalid(format!("expected signed integer, got `{}`: {}", raw, e)))?;
    Ok(parsed)
}

fn prompt_pattern(theme: &ColorfulTheme, prompt: &str, default: Option<String>) -> BlastResult<String> {
    loop {
        let mut input = Input::<String>::with_theme(theme).with_prompt(prompt);
        match default.clone() {
            Some(d) => input = input.default(d),
            None => {}
        }
        let raw: String = input.interact_text()?;
        match regex::Regex::new(&raw) {
            Ok(_re) => return Ok(raw),
            Err(e) => {
                eprintln!("invalid regex: {e}; please retry");
                continue;
            }
        }
    }
}

fn prompt_string(theme: &ColorfulTheme, prompt: &str, default: Option<String>) -> BlastResult<String> {
    let mut input = Input::<String>::with_theme(theme).with_prompt(prompt);
    match default {
        Some(d) => input = input.default(d),
        None => {}
    }
    let raw: String = input.interact_text()?;
    Ok(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_labels_cover_all_variants() {
        assert_eq!(RULE_LABELS.len(), 9);
    }

    #[test]
    fn build_rule_from_label_parses_required() {
        let theme = ColorfulTheme::default();
        let prev: BTreeSet<ValidatorRule> = BTreeSet::new();
        let result = build_rule_from_label(&theme, "title", "Required", &prev).expect("required");
        assert_eq!(result, ValidatorRule::Required);
    }

    #[test]
    fn build_rule_from_label_parses_email() {
        let theme = ColorfulTheme::default();
        let prev: BTreeSet<ValidatorRule> = BTreeSet::new();
        let result = build_rule_from_label(&theme, "email", "Email", &prev).expect("email");
        assert_eq!(result, ValidatorRule::Email);
    }

    #[test]
    fn build_rule_from_label_parses_url() {
        let theme = ColorfulTheme::default();
        let prev: BTreeSet<ValidatorRule> = BTreeSet::new();
        let result = build_rule_from_label(&theme, "homepage", "Url", &prev).expect("url");
        assert_eq!(result, ValidatorRule::Url);
    }

    #[test]
    fn build_rule_from_label_rejects_unknown_label() {
        let theme = ColorfulTheme::default();
        let prev: BTreeSet<ValidatorRule> = BTreeSet::new();
        let result = build_rule_from_label(&theme, "x", "Bogus", &prev);
        assert!(result.is_err());
    }
}
