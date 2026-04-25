
use crate::codegen::ir_loader::{FieldSpec, Validator};

pub fn emit_ts(field: &FieldSpec) -> String {
    let mut out = String::new();
    out.push_str(&format!("export const validate_{} = [\n", field.name));
    for v in &field.validation.validators {
        out.push_str("  ");
        out.push_str(&emit_one(v));
        out.push_str(",\n");
    }
    out.push_str("];\n");
    out
}

pub fn emit_one(v: &Validator) -> String {
    match v {
        Validator::MinLen(n) => format!(
            "(v: string) => (v?.length ?? 0) >= {n} || 'min length {n}'"
        ),
        Validator::MaxLen(n) => format!(
            "(v: string) => (v?.length ?? 0) <= {n} || 'max length {n}'"
        ),
        Validator::MinValue(n) => format!(
            "(v: number) => v >= {n} || 'must be >= {n}'"
        ),
        Validator::MaxValue(n) => format!(
            "(v: number) => v <= {n} || 'must be <= {n}'"
        ),
        Validator::Regex(pat) => {
            let escaped = pat.replace('\\', "\\\\").replace('\'', "\\'");
            format!(
                "(v: string) => new RegExp('{escaped}').test(v) || 'invalid format'"
            )
        }
        Validator::Email => {
            "(v: string) => /^[^\\s@]+@[^\\s@]+\\.[^\\s@]+$/.test(v) || 'invalid email'"
                .to_string()
        }
        Validator::Url => {
            "(v: string) => { try { new URL(v); return true; } catch { return 'invalid url'; } }"
                .to_string()
        }
        Validator::OneOf(values) => {
            let arr = values
                .iter()
                .map(|s| format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'")))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "(v: string) => [{arr}].includes(v) || 'must be one of: {}'",
                values.join(", ").replace('\'', "\\'")
            )
        }
        Validator::Required => {
            "(v: unknown) => (v !== undefined && v !== null && v !== '') || 'required'"
                .to_string()
        }
    }
}
