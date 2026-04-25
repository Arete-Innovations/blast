use crate::state::{FieldName, FieldState};
use crate::state::resource::ValidatorRule;

pub fn emit_ts(name: &FieldName, field: &FieldState) -> String {
    let mut out = String::new();
    out.push_str(&format!("export const validate_{} = [\n", name.as_str()));
    for v in &field.validators {
        out.push_str("  ");
        out.push_str(&emit_one(v));
        out.push_str(",\n");
    }
    out.push_str("];\n");
    out
}

pub fn emit_one(v: &ValidatorRule) -> String {
    match v {
        ValidatorRule::MinLen(n) => format!(
            "(v: string) => (v?.length ?? 0) >= {n} || 'min length {n}'"
        ),
        ValidatorRule::MaxLen(n) => format!(
            "(v: string) => (v?.length ?? 0) <= {n} || 'max length {n}'"
        ),
        ValidatorRule::MinValue(n) => format!(
            "(v: number) => v >= {n} || 'must be >= {n}'"
        ),
        ValidatorRule::MaxValue(n) => format!(
            "(v: number) => v <= {n} || 'must be <= {n}'"
        ),
        ValidatorRule::Pattern(pat) => {
            let escaped = pat.replace('\\', "\\\\").replace('\'', "\\'");
            format!(
                "(v: string) => new RegExp('{escaped}').test(v) || 'invalid format'"
            )
        }
        ValidatorRule::Email => {
            "(v: string) => /^[^\\s@]+@[^\\s@]+\\.[^\\s@]+$/.test(v) || 'invalid email'"
                .to_string()
        }
        ValidatorRule::Url => {
            "(v: string) => { try { new URL(v); return true; } catch { return 'invalid url'; } }"
                .to_string()
        }
        ValidatorRule::OneOf(values) => {
            let arr = values
                .iter()
                .map(|s: &String| format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'")))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "(v: string) => [{arr}].includes(v) || 'must be one of: {}'",
                values.join(", ").replace('\'', "\\'")
            )
        }
        ValidatorRule::Required => {
            "(v: unknown) => (v !== undefined && v !== null && v !== '') || 'required'"
                .to_string()
        }
    }
}
