use crate::state::SqlType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimeComponent {
    InputText,
    InputNumber,
    Checkbox,
    Calendar,
    Textarea,
}

impl PrimeComponent {
    pub fn tag_name(&self) -> &'static str {
        match self {
            Self::InputText => "InputText",
            Self::InputNumber => "InputNumber",
            Self::Checkbox => "Checkbox",
            Self::Calendar => "Calendar",
            Self::Textarea => "Textarea",
        }
    }

    pub fn extra_attrs(&self) -> &'static str {
        match self {
            Self::Checkbox => " :binary=\"true\"",
            Self::InputText => "",
            Self::InputNumber => "",
            Self::Calendar => "",
            Self::Textarea => "",
        }
    }

    pub fn import_module(&self) -> &'static str {
        match self {
            Self::InputText => "primevue/inputtext",
            Self::InputNumber => "primevue/inputnumber",
            Self::Checkbox => "primevue/checkbox",
            Self::Calendar => "primevue/calendar",
            Self::Textarea => "primevue/textarea",
        }
    }

    pub fn ts_initial(&self) -> &'static str {
        match self {
            Self::Checkbox => "false",
            Self::InputNumber => "0",
            Self::Calendar => "null",
            Self::InputText => "''",
            Self::Textarea => "''",
        }
    }
}

pub fn prime_component_for(sql: &SqlType) -> PrimeComponent {
    let lowered = sql.as_str().to_ascii_lowercase();
    if is_bool(&lowered) {
        return PrimeComponent::Checkbox;
    }
    if is_numeric(&lowered) {
        return PrimeComponent::InputNumber;
    }
    if is_temporal(&lowered) {
        return PrimeComponent::Calendar;
    }
    if is_json(&lowered) {
        return PrimeComponent::Textarea;
    }
    // text, varchar, uuid, char, bpchar, citext — and anything Blast hasn't
    // taught itself yet — fall back to a plain text input.
    PrimeComponent::InputText
}

fn is_bool(lowered: &str) -> bool {
    matches!(lowered, "bool" | "boolean")
}

fn is_numeric(lowered: &str) -> bool {
    matches!(
        lowered,
        "int2"
            | "smallint"
            | "int4"
            | "integer"
            | "int8"
            | "bigint"
            | "numeric"
            | "decimal"
            | "float4"
            | "real"
            | "float8"
            | "double"
            | "double precision"
    )
}

fn is_temporal(lowered: &str) -> bool {
    matches!(
        lowered,
        "timestamp" | "timestamptz" | "date" | "time" | "timetz"
    )
}

fn is_json(lowered: &str) -> bool {
    matches!(lowered, "jsonb" | "json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sql_type_to_primevue_mapping() {
        assert_eq!(
            prime_component_for(&SqlType::new("Bool")),
            PrimeComponent::Checkbox
        );
        assert_eq!(
            prime_component_for(&SqlType::new("Int8")),
            PrimeComponent::InputNumber
        );
        assert_eq!(
            prime_component_for(&SqlType::new("Numeric")),
            PrimeComponent::InputNumber
        );
        assert_eq!(
            prime_component_for(&SqlType::new("Timestamptz")),
            PrimeComponent::Calendar
        );
        assert_eq!(
            prime_component_for(&SqlType::new("Jsonb")),
            PrimeComponent::Textarea
        );
        assert_eq!(
            prime_component_for(&SqlType::new("Varchar")),
            PrimeComponent::InputText
        );
        assert_eq!(
            prime_component_for(&SqlType::new("Uuid")),
            PrimeComponent::InputText
        );
    }
}
