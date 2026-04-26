use crate::codegen::vue::naming::{pascal_case, singularize, ts_object_key};
use crate::codegen::vue::sql_map::{prime_component_for, PrimeComponent};
use crate::state::{FieldName, FieldState, FieldVariant, ResourceState, Verb};

pub fn build_form_sfc(r: &ResourceState) -> String {
    let table = r.name.as_str();
    let pascal = pascal_case(&singularize(table));
    let has_create = r.verbs.contains_key(&Verb::Create);
    let has_update = r.verbs.contains_key(&Verb::Update);

    let create_fields: Vec<(&FieldName, &FieldState)> = r
        .fields
        .iter()
        .filter(|(_, f)| f.variants.contains(&FieldVariant::Insertable))
        .collect();
    let patch_fields: Vec<(&FieldName, &FieldState)> = r
        .fields
        .iter()
        .filter(|(_, f)| f.variants.contains(&FieldVariant::Patch))
        .collect();

    let union_fields: Vec<(&FieldName, &FieldState)> = if has_create {
        create_fields
    } else {
        patch_fields
    };

    let inputs_html = render_form_inputs(&union_fields);
    let imports = build_form_imports(&union_fields, table);
    let mode_default = if has_create { "'create'" } else { "'update'" };
    let model_init = render_model_init(&union_fields);
    let emit_decl = render_form_emits(has_create, has_update, &pascal);

    format!(
        "<script setup lang=\"ts\">\n\
{imports}\n\
type FormMode = 'create' | 'update'\n\
\n\
const props = defineProps<{{\n\
  mode?: FormMode\n\
  submitting?: boolean\n\
  submitError?: string\n\
}}>()\n\
const emit = defineEmits{emits}()\n\
\n\
const toast = useToast()\n\
const mode = computed<FormMode>(() => props.mode ?? {mode_default})\n\
const model = reactive<Record<string, unknown>>({{ {model_init} }})\n\
const errors = reactive<Record<string, string>>({{}})\n\
const submitting = computed<boolean>(() => props.submitting === true)\n\
const submitLabel = computed<string>(() => mode.value === 'create' ? 'Create' : 'Save')\n\
\n\
function clearErrors(): void {{\n\
  for (const key of Object.keys(errors)) {{\n\
    delete errors[key]\n\
  }}\n\
}}\n\
\n\
function onSubmit(): void {{\n\
  clearErrors()\n\
  const result = validate(model)\n\
  if (result.ok !== true) {{\n\
    for (const issue of result.errors) {{\n\
      errors[issue.field] = issue.message\n\
    }}\n\
    toast.add({{\n\
      severity: 'warn',\n\
      summary: 'Check your input',\n\
      detail: 'Some fields need attention before saving.',\n\
      life: 4000,\n\
    }})\n\
    return\n\
  }}\n\
  emit('submit', model)\n\
}}\n\
\n\
function onRetry(): void {{\n\
  emit('retry')\n\
}}\n\
\n\
defineExpose({{ clearErrors }})\n\
</script>\n\
\n\
<template>\n\
  <form\n\
    class=\"resource-form\"\n\
    novalidate\n\
    :aria-busy=\"submitting\"\n\
    @submit.prevent=\"onSubmit\"\n\
  >\n\
    <Message\n\
      v-if=\"submitError !== undefined && submitError !== ''\"\n\
      severity=\"error\"\n\
      class=\"form-banner\"\n\
      :closable=\"false\"\n\
    >\n\
      <div class=\"form-banner-row\">\n\
        <span>{{{{ submitError }}}}</span>\n\
        <Button\n\
          type=\"button\"\n\
          text\n\
          size=\"small\"\n\
          icon=\"pi pi-refresh\"\n\
          label=\"Retry\"\n\
          aria-label=\"Retry submitting the form\"\n\
          @click=\"onRetry\"\n\
        />\n\
      </div>\n\
    </Message>\n\
{inputs}\
    <div class=\"form-actions\">\n\
      <Button\n\
        type=\"submit\"\n\
        :label=\"submitLabel\"\n\
        :loading=\"submitting\"\n\
        :disabled=\"submitting\"\n\
        :aria-label=\"submitLabel\"\n\
      />\n\
    </div>\n\
  </form>\n\
</template>\n\
\n\
<style scoped>\n\
@layer app {{\n\
  .resource-form {{\n\
    display: flex;\n\
    flex-direction: column;\n\
    gap: var(--app-space-lg);\n\
  }}\n\
  .form-row {{\n\
    display: flex;\n\
    flex-direction: column;\n\
    gap: var(--app-space-xs);\n\
  }}\n\
  .form-actions {{\n\
    display: flex;\n\
    justify-content: flex-end;\n\
    gap: var(--app-space-md);\n\
  }}\n\
  .form-banner {{\n\
    margin-bottom: var(--app-space-md);\n\
  }}\n\
  .form-banner-row {{\n\
    display: flex;\n\
    align-items: center;\n\
    justify-content: space-between;\n\
    gap: var(--app-space-md);\n\
  }}\n\
  .field-error {{\n\
    color: var(--p-message-error-color, var(--app-color-danger));\n\
    font-size: var(--app-fs-sm);\n\
  }}\n\
}}\n\
</style>\n",
        imports = imports,
        emits = emit_decl,
        mode_default = mode_default,
        model_init = model_init,
        inputs = inputs_html,
    )
}

fn build_form_imports(fields: &[(&FieldName, &FieldState)], table: &str) -> String {
    let mut out = String::new();
    out.push_str("import { computed, reactive } from 'vue'\n");
    out.push_str("import { useToast } from 'primevue/usetoast'\n");
    out.push_str(&primevue_imports_for_fields(fields));
    out.push_str(&format!(
        "import {{ validate }} from '@/generated/validators/{table}'\n",
        table = table
    ));
    out
}

fn render_form_emits(has_create: bool, has_update: bool, pascal: &str) -> String {
    let payload = match (has_create, has_update) {
        (true, true) => format!("{p}Insertable | {p}Patch", p = pascal),
        (true, false) => format!("{}Insertable", pascal),
        (false, true) => format!("{}Patch", pascal),
        (false, false) => "unknown".to_string(),
    };
    format!("<{{ submit: [payload: {}]; retry: [] }}>", payload)
}

fn render_model_init(fields: &[(&FieldName, &FieldState)]) -> String {
    let parts: Vec<String> = fields
        .iter()
        .map(|(name, f)| {
            format!(
                "{}: {}",
                ts_object_key(name.as_str()),
                prime_component_for(&f.sql_type).ts_initial()
            )
        })
        .collect();
    parts.join(", ")
}

fn render_form_inputs(fields: &[(&FieldName, &FieldState)]) -> String {
    let mut out = String::new();
    for (name, field) in fields {
        let label = name.as_str();
        let model_key = ts_object_key(name.as_str());
        let component = prime_component_for(&field.sql_type);
        let tag = component.tag_name();
        let extra = component.extra_attrs();
        out.push_str(&format!(
            "    <div class=\"form-row\">\n\
      <label :for=\"'fld-{name}'\">{label}</label>\n\
      <{tag}\n\
        input-id=\"fld-{name}\"\n\
        v-model=\"model.{key}\"\n\
        :invalid=\"errors['{name}'] !== undefined\"\n\
        :aria-invalid=\"errors['{name}'] !== undefined\"\n\
        :aria-describedby=\"errors['{name}'] !== undefined ? 'err-{name}' : undefined\"\n\
        :disabled=\"submitting\"{extra}\n\
      />\n\
      <small\n\
        v-if=\"errors['{name}'] !== undefined\"\n\
        :id=\"'err-{name}'\"\n\
        class=\"field-error p-error\"\n\
        role=\"alert\"\n\
      >{{{{ errors['{name}'] }}}}</small>\n\
    </div>\n",
            name = name.as_str(),
            label = label,
            tag = tag,
            key = model_key,
            extra = extra,
        ));
    }
    out
}

fn primevue_imports_for_fields(fields: &[(&FieldName, &FieldState)]) -> String {
    let mut seen: Vec<PrimeComponent> = Vec::new();
    for (_, f) in fields {
        let comp = prime_component_for(&f.sql_type);
        if !seen.contains(&comp) {
            seen.push(comp);
        }
    }
    let mut out = String::new();
    out.push_str("import Button from 'primevue/button'\n");
    out.push_str("import Message from 'primevue/message'\n");
    for comp in seen {
        out.push_str(&format!(
            "import {tag} from '{module}'\n",
            tag = comp.tag_name(),
            module = comp.import_module()
        ));
    }
    out
}
