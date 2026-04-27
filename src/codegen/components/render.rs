use crate::codegen::components::input_map::{self, enum_meta, enum_options_const_name, enum_type_alias};
use crate::codegen::enums::ParsedEnum;
use crate::codegen::structs::naming::type_stem_for_resource;
use crate::state::{FieldName, FieldState, FieldVariant, ResourceState, SqlType};

pub fn forms_for_resource(resource: &ResourceState, enums: &[ParsedEnum]) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let insertable_fields = collect_fields(resource, FieldVariant::Insertable);
    if !insertable_fields.is_empty() {
        out.push(("CreateForm.vue".to_string(), build_create_form(resource, &insertable_fields, enums)));
    }
    let patch_fields = collect_fields(resource, FieldVariant::Patch);
    if !patch_fields.is_empty() {
        out.push(("EditForm.vue".to_string(), build_edit_form(resource, &patch_fields, enums)));
    }
    out
}

fn collect_fields<'a>(resource: &'a ResourceState, variant: FieldVariant) -> Vec<(&'a FieldName, &'a FieldState)> {
    resource.fields.iter().filter(|(_, f)| f.variants.contains(&variant) && !f.primary_key).collect()
}

pub fn build_create_form(resource: &ResourceState, fields: &[(&FieldName, &FieldState)], enums: &[ParsedEnum]) -> String {
    let table = resource.name.as_str();
    let stem = type_stem_for_resource(resource);
    let dto = format!("{stem}Insertable");

    let imports_block = render_primevue_imports(fields, enums);
    let enum_imports_block = render_enum_imports(fields, enums);
    let initial_block = render_initial_create(fields, enums);
    let template_fields = render_template_fields(fields, enums);
    let serialize_block = render_serialize_block(fields);

    format!(
        "<script setup lang=\"ts\">\n\
import {{ ref, reactive }} from 'vue'\n\
import Button from 'primevue/button'\n\
{imports_block}import {{ create{stem} }} from '@/generated/api/{table}'\n\
import type {{ {dto} }} from '@/generated/types/{table}'\n\
{enum_imports_block}\n\
const emit = defineEmits<{{\n\
  (event: 'created', payload: {dto}): void\n\
  (event: 'cancel'): void\n\
}}>()\n\
\n\
{initial_block}\n\
const submitting = ref<boolean>(false)\n\
const error_message = ref<string | null>(null)\n\
\n\
async function on_submit(): Promise<void> {{\n\
  submitting.value = true\n\
  error_message.value = null\n\
{serialize_block}  const result = await create{stem}(payload as {dto})\n\
  submitting.value = false\n\
  if (result.error !== null) {{\n\
    error_message.value = result.error.error.message\n\
    return\n\
  }}\n\
  emit('created', payload as {dto})\n\
}}\n\
</script>\n\
\n\
<template>\n\
  <form class=\"{table}-create-form\" novalidate @submit.prevent=\"on_submit\">\n\
{template_fields}    <div v-if=\"error_message !== null\" class=\"{table}-create-form-error\" role=\"alert\">\n\
      {{{{ error_message }}}}\n\
    </div>\n\
    <div class=\"{table}-create-form-actions\">\n\
      <Button type=\"button\" label=\"Cancel\" severity=\"secondary\" :disabled=\"submitting\" @click=\"emit('cancel')\" />\n\
      <Button type=\"submit\" label=\"Create\" :loading=\"submitting\" :disabled=\"submitting\" />\n\
    </div>\n\
  </form>\n\
</template>\n\
\n\
<style scoped>\n\
@layer app {{\n\
  .{table}-create-form {{\n\
    display: flex;\n\
    flex-direction: column;\n\
    gap: var(--app-space-md);\n\
  }}\n\
  .{table}-create-form-actions {{\n\
    display: flex;\n\
    gap: var(--app-space-sm);\n\
    justify-content: flex-end;\n\
  }}\n\
  .{table}-create-form-error {{\n\
    color: var(--p-message-error-color, var(--app-color-danger, #b00020));\n\
  }}\n\
}}\n\
</style>\n",
        table = table,
        stem = stem,
        dto = dto,
        imports_block = imports_block,
        enum_imports_block = enum_imports_block,
        initial_block = initial_block,
        template_fields = template_fields,
        serialize_block = serialize_block,
    )
}

pub fn build_edit_form(resource: &ResourceState, fields: &[(&FieldName, &FieldState)], enums: &[ParsedEnum]) -> String {
    let table = resource.name.as_str();
    let stem = type_stem_for_resource(resource);
    let public = format!("{stem}Public");
    let patch = format!("{stem}Patch");

    let imports_block = render_primevue_imports(fields, enums);
    let enum_imports_block = render_enum_imports(fields, enums);
    let initial_block = render_initial_edit(fields, enums);
    let template_fields = render_template_fields(fields, enums);
    let serialize_block = render_serialize_block(fields);

    format!(
        "<script setup lang=\"ts\">\n\
import {{ ref, reactive, watch }} from 'vue'\n\
import Button from 'primevue/button'\n\
{imports_block}import {{ update{stem} }} from '@/generated/api/{table}'\n\
import type {{ {public}, {patch} }} from '@/generated/types/{table}'\n\
{enum_imports_block}\n\
const props = defineProps<{{ entity: {public} }}>()\n\
const emit = defineEmits<{{\n\
  (event: 'updated', payload: {patch}): void\n\
  (event: 'cancel'): void\n\
}}>()\n\
\n\
{initial_block}\n\
const submitting = ref<boolean>(false)\n\
const error_message = ref<string | null>(null)\n\
\n\
watch(() => props.entity, (next) => {{\n\
  reset_form(next as unknown as {{ [key: string]: unknown }})\n\
}}, {{ deep: true }})\n\
\n\
async function on_submit(): Promise<void> {{\n\
  submitting.value = true\n\
  error_message.value = null\n\
{serialize_block}  const result = await update{stem}((props.entity as unknown as {{ id: number }}).id, payload as {patch})\n\
  submitting.value = false\n\
  if (result.error !== null) {{\n\
    error_message.value = result.error.error.message\n\
    return\n\
  }}\n\
  emit('updated', payload as {patch})\n\
}}\n\
</script>\n\
\n\
<template>\n\
  <form class=\"{table}-edit-form\" novalidate @submit.prevent=\"on_submit\">\n\
{template_fields}    <div v-if=\"error_message !== null\" class=\"{table}-edit-form-error\" role=\"alert\">\n\
      {{{{ error_message }}}}\n\
    </div>\n\
    <div class=\"{table}-edit-form-actions\">\n\
      <Button type=\"button\" label=\"Cancel\" severity=\"secondary\" :disabled=\"submitting\" @click=\"emit('cancel')\" />\n\
      <Button type=\"submit\" label=\"Save\" :loading=\"submitting\" :disabled=\"submitting\" />\n\
    </div>\n\
  </form>\n\
</template>\n\
\n\
<style scoped>\n\
@layer app {{\n\
  .{table}-edit-form {{\n\
    display: flex;\n\
    flex-direction: column;\n\
    gap: var(--app-space-md);\n\
  }}\n\
  .{table}-edit-form-actions {{\n\
    display: flex;\n\
    gap: var(--app-space-sm);\n\
    justify-content: flex-end;\n\
  }}\n\
  .{table}-edit-form-error {{\n\
    color: var(--p-message-error-color, var(--app-color-danger, #b00020));\n\
  }}\n\
}}\n\
</style>\n",
        table = table,
        stem = stem,
        public = public,
        patch = patch,
        imports_block = imports_block,
        enum_imports_block = enum_imports_block,
        initial_block = initial_block,
        template_fields = template_fields,
        serialize_block = serialize_block,
    )
}

fn render_primevue_imports(fields: &[(&FieldName, &FieldState)], enums: &[ParsedEnum]) -> String {
    let mut seen: Vec<String> = Vec::new();
    for (_, f) in fields {
        let comp = input_map::primevue_component(&f.sql_type, enums);
        let owned = comp.to_string();
        if !seen.contains(&owned) {
            seen.push(owned);
        }
    }
    seen.sort();
    let mut out = String::new();
    for comp in &seen {
        let lower = comp.to_ascii_lowercase();
        out.push_str(&format!("import {comp} from 'primevue/{lower}'\n"));
    }
    out
}

fn render_enum_imports(fields: &[(&FieldName, &FieldState)], enums: &[ParsedEnum]) -> String {
    let mut seen: Vec<(String, Vec<String>)> = Vec::new();
    for (_, f) in fields {
        match enum_meta(&f.sql_type, enums) {
            Some((name, variants)) => {
                if !seen.iter().any(|(n, _)| n == &name) {
                    seen.push((name, variants));
                }
            }
            None => continue, // allow: non-enum fields produce no enum import
        }
    }
    let mut out = String::new();
    for (name, _) in &seen {
        let alias = enum_type_alias(name);
        let const_name = enum_options_const_name(name);
        out.push_str(&format!("import {{ {const_name} }} from '@/generated/types/{name}'\n"));
        out.push_str(&format!("import type {{ {alias} }} from '@/generated/types/{name}'\n"));
    }
    out
}

fn render_initial_create(fields: &[(&FieldName, &FieldState)], enums: &[ParsedEnum]) -> String {
    let mut lines = String::new();
    lines.push_str("const form = reactive<{ [key: string]: unknown }>({\n");
    for (name, field) in fields {
        let init = empty_value(&field.sql_type, field.nullable, enums);
        lines.push_str(&format!("  {}: {},\n", name.as_str(), init));
    }
    lines.push_str("})\n");
    lines
}

fn render_initial_edit(fields: &[(&FieldName, &FieldState)], enums: &[ParsedEnum]) -> String {
    let mut lines = String::new();
    lines.push_str("function snapshot_from(entity: { [key: string]: unknown }): { [key: string]: unknown } {\n");
    lines.push_str("  return {\n");
    for (name, field) in fields {
        let key = name.as_str();
        if input_map::is_calendar(&field.sql_type) {
            lines.push_str(&format!("    {key}: typeof entity['{key}'] === 'string' ? new Date(entity['{key}'] as string) : null,\n"));
        } else {
            let fallback = empty_value(&field.sql_type, field.nullable, enums);
            lines.push_str(&format!("    {key}: entity['{key}'] === undefined ? {fallback} : entity['{key}'],\n"));
        }
    }
    lines.push_str("  }\n");
    lines.push_str("}\n");
    lines.push_str("const form = reactive<{ [key: string]: unknown }>(snapshot_from(props.entity as unknown as { [key: string]: unknown }))\n");
    lines.push_str("function reset_form(entity: { [key: string]: unknown }): void {\n");
    lines.push_str("  const next = snapshot_from(entity)\n");
    lines.push_str("  for (const key of Object.keys(form)) {\n");
    lines.push_str("    form[key] = next[key]\n");
    lines.push_str("  }\n");
    lines.push_str("}\n");
    lines
}

fn render_template_fields(fields: &[(&FieldName, &FieldState)], enums: &[ParsedEnum]) -> String {
    let mut out = String::new();
    for (name, field) in fields {
        let key = name.as_str();
        let label = humanize(key);
        let comp = input_map::primevue_component(&field.sql_type, enums);
        let attrs = render_attrs(comp, &field.sql_type, enums);
        let id = format!("form-{key}");
        out.push_str(&format!(
            "    <div class=\"form-field\">\n      <label for=\"{id}\" class=\"form-field-label\">{label}</label>\n      <{comp} id=\"{id}\" v-model=\"form.{key}\"{attrs} />\n    </div>\n"
        ));
    }
    out
}

fn render_attrs(component: &str, sql: &SqlType, enums: &[ParsedEnum]) -> String {
    match component {
        "Checkbox" => " :binary=\"true\"".to_string(),
        "Calendar" => {
            let mut s = String::new();
            if input_map::calendar_show_time(sql) {
                s.push_str(" show-time");
            }
            if input_map::calendar_time_only(sql) {
                s.push_str(" time-only");
            }
            s
        }
        "Textarea" => " rows=\"4\"".to_string(),
        "Dropdown" => match enum_meta(sql, enums) {
            Some((name, _variants)) => format!(" :options=\"{}\"", enum_options_const_name(&name)),
            None => String::new(), // allow: dropdown without enum metadata renders no options attr
        },
        _other => String::new(),
    }
}

fn render_serialize_block(fields: &[(&FieldName, &FieldState)]) -> String {
    let mut s = String::new();
    s.push_str("  const payload: { [key: string]: unknown } = {}\n");
    for (name, field) in fields {
        let key = name.as_str();
        if input_map::is_calendar(&field.sql_type) {
            s.push_str(&format!("  payload['{key}'] = form['{key}'] instanceof Date ? (form['{key}'] as Date).toISOString() : form['{key}']\n"));
        } else if input_map::is_json(&field.sql_type) {
            s.push_str(&format!("  if (typeof form['{key}'] === 'string' && form['{key}'] !== '') {{\n    try {{\n      payload['{key}'] = JSON.parse(form['{key}'] as string)\n    }} catch (_e) {{\n      submitting.value = false\n      error_message.value = 'Invalid JSON in {key}.'\n      return\n    }}\n  }} else {{\n    payload['{key}'] = form['{key}']\n  }}\n"));
        } else {
            s.push_str(&format!("  payload['{key}'] = form['{key}']\n"));
        }
    }
    s
}

fn empty_value(sql: &SqlType, nullable: bool, enums: &[ParsedEnum]) -> String {
    match enum_meta(sql, enums) {
        Some((name, _variants)) => {
            if nullable {
                return "null".to_string();
            }
            return format!("{}[0]", enum_options_const_name(&name));
        }
        None => {} // allow: fall through to non-enum scalar mapping
    }
    if nullable {
        return "null".to_string();
    }
    if input_map::is_bool(sql) {
        return "false".to_string();
    }
    if input_map::is_number(sql) {
        return "0".to_string();
    }
    if input_map::is_calendar(sql) {
        return "null".to_string();
    }
    if input_map::is_json(sql) {
        return "''".to_string();
    }
    "''".to_string()
}

fn humanize(snake: &str) -> String {
    let words: Vec<String> = snake
        .split('_')
        .filter(|w| !w.is_empty())
        .enumerate()
        .map(|(i, w)| {
            let mut chars = w.chars();
            match chars.next() {
                None => String::new(), // allow: empty word segment is not a failure
                Some(first) => {
                    if i == 0 {
                        let upper: String = first.to_uppercase().collect();
                        upper + chars.as_str()
                    } else {
                        let lower: String = first.to_lowercase().collect();
                        lower + chars.as_str()
                    }
                }
            }
        })
        .collect();
    words.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::names::{FieldName, ResourceName};
    use crate::state::resource::{AuthMode, FieldState, FieldVariant, ResourceState, Verb, VerbState, RESOURCE_SCHEMA_VERSION};
    use indexmap::IndexMap;
    use std::collections::{BTreeMap, BTreeSet};

    fn synth_resource() -> ResourceState {
        let mut fields: IndexMap<FieldName, FieldState> = IndexMap::new();
        let all_v: BTreeSet<FieldVariant> = [FieldVariant::Db, FieldVariant::Insertable, FieldVariant::Patch, FieldVariant::Public].into_iter().collect();
        let id_v: BTreeSet<FieldVariant> = [FieldVariant::Db, FieldVariant::Public].into_iter().collect();

        fields.insert(
            FieldName::new("id"),
            FieldState {
                sql_type: crate::state::SqlType::new("Int8"),
                variants: id_v,
                nullable: false,
                primary_key: true,
                validators: BTreeSet::new(),
            },
        );
        fields.insert(
            FieldName::new("email"),
            FieldState {
                sql_type: crate::state::SqlType::new("Varchar"),
                variants: all_v.clone(),
                nullable: false,
                primary_key: false,
                validators: BTreeSet::new(),
            },
        );
        fields.insert(
            FieldName::new("active"),
            FieldState {
                sql_type: crate::state::SqlType::new("Bool"),
                variants: all_v,
                nullable: false,
                primary_key: false,
                validators: BTreeSet::new(),
            },
        );

        let mut verbs: IndexMap<Verb, VerbState> = IndexMap::new();
        verbs.insert(
            Verb::Create,
            VerbState {
                auth: AuthMode::Public,
                list_options: None,
            },
        );
        verbs.insert(
            Verb::Update,
            VerbState {
                auth: AuthMode::Public,
                list_options: None,
            },
        );

        ResourceState {
            schema_version: RESOURCE_SCHEMA_VERSION,
            name: ResourceName::new("users"),
            fields,
            verbs,
            ws_events: None,
            singular_override: None,
            soft_delete: None,
            relations: BTreeMap::new(),
            gen_level: crate::state::GenLevel::default(),
        }
    }

    #[test]
    fn create_form_imports_required_primevue_components() {
        let r = synth_resource();
        let enums: Vec<crate::codegen::enums::ParsedEnum> = Vec::new();
        let pairs = forms_for_resource(&r, &enums);
        let create = pairs.iter().find(|(name, _)| name == "CreateForm.vue").map(|(_, body)| body.clone()).expect("CreateForm.vue produced");
        assert!(create.contains("from 'primevue/checkbox'"), "Checkbox import missing");
        assert!(create.contains("from 'primevue/inputtext'"), "InputText import missing");
        assert!(create.contains("from 'primevue/button'"), "Button import missing");
    }

    #[test]
    fn create_form_does_not_include_primary_key_field() {
        let r = synth_resource();
        let enums: Vec<crate::codegen::enums::ParsedEnum> = Vec::new();
        let pairs = forms_for_resource(&r, &enums);
        let create = pairs.iter().find(|(name, _)| name == "CreateForm.vue").map(|(_, body)| body.clone()).expect("CreateForm.vue produced");
        assert!(!create.contains("v-model=\"form.id\""));
    }

    #[test]
    fn edit_form_imports_patch_and_public_types() {
        let r = synth_resource();
        let enums: Vec<crate::codegen::enums::ParsedEnum> = Vec::new();
        let pairs = forms_for_resource(&r, &enums);
        let edit = pairs.iter().find(|(name, _)| name == "EditForm.vue").map(|(_, body)| body.clone()).expect("EditForm.vue produced");
        assert!(edit.contains("UserPatch"), "UserPatch type missing");
        assert!(edit.contains("UserPublic"), "UserPublic type missing");
        assert!(edit.contains("updateUser"), "updateUser API call missing");
    }

    #[test]
    fn create_form_calls_create_api() {
        let r = synth_resource();
        let enums: Vec<crate::codegen::enums::ParsedEnum> = Vec::new();
        let pairs = forms_for_resource(&r, &enums);
        let create = pairs.iter().find(|(name, _)| name == "CreateForm.vue").map(|(_, body)| body.clone()).expect("CreateForm.vue produced");
        assert!(create.contains("createUser"), "createUser API call missing");
        assert!(create.contains("UserInsertable"), "UserInsertable type missing");
    }

    #[test]
    fn humanize_converts_snake_to_title() {
        assert_eq!(humanize("email_address"), "Email address");
        assert_eq!(humanize("first_name"), "First name");
        assert_eq!(humanize("name"), "Name");
    }

    #[test]
    fn render_enum_imports_is_empty_when_no_enum_fields() {
        let r = synth_resource();
        let enums: Vec<crate::codegen::enums::ParsedEnum> = Vec::new();
        let fields = collect_fields(&r, FieldVariant::Insertable);
        let block = render_enum_imports(&fields, &enums);
        assert!(block.is_empty(), "no enum fields means no enum imports, got {block}");
    }

    #[test]
    fn empty_value_for_plain_text_is_empty_string() {
        let enums: Vec<crate::codegen::enums::ParsedEnum> = Vec::new();
        assert_eq!(empty_value(&SqlType::new("Varchar"), false, &enums), "''");
        assert_eq!(empty_value(&SqlType::new("Bool"), false, &enums), "false");
        assert_eq!(empty_value(&SqlType::new("Int8"), false, &enums), "0");
        assert_eq!(empty_value(&SqlType::new("Int4"), true, &enums), "null");
    }

    #[test]
    fn create_form_uses_dropdown_for_enum_field() {
        use std::path::PathBuf;
        let mut r = synth_resource();
        let all_v: BTreeSet<FieldVariant> = [FieldVariant::Db, FieldVariant::Insertable, FieldVariant::Patch, FieldVariant::Public].into_iter().collect();
        r.fields.insert(
            FieldName::new("role"),
            FieldState {
                sql_type: SqlType::new("UserRole"),
                variants: all_v,
                nullable: false,
                primary_key: false,
                validators: BTreeSet::new(),
            },
        );
        let enums = vec![crate::codegen::enums::ParsedEnum {
            name: "user_role".to_string(),
            variants: vec!["admin".to_string(), "member".to_string()],
            source_file: PathBuf::from("/tmp/dummy.sql"),
        }];
        let pairs = forms_for_resource(&r, &enums);
        let create = pairs.iter().find(|(name, _)| name == "CreateForm.vue").map(|(_, body)| body.clone()).expect("CreateForm.vue produced");
        assert!(create.contains("from 'primevue/dropdown'"), "Dropdown import missing: {create}");
        assert!(create.contains("USER_ROLE_VALUES"), "options const reference missing");
        assert!(create.contains("import { USER_ROLE_VALUES } from '@/generated/types/user_role'"), "enum values import missing");
        assert!(create.contains("import type { UserRole } from '@/generated/types/user_role'"), "enum type import missing");
        assert!(create.contains(":options=\"USER_ROLE_VALUES\""), "Dropdown options binding missing");
    }
}
