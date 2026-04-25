use crate::codegen::vue::plan::ResourcePlan;

pub fn build_resource_index_ts(plan: &ResourcePlan) -> String {
    let mut out = String::new();
    if plan.emit_form {
        out.push_str("export { default as Form } from './Form.vue'\n");
    }
    if plan.emit_list {
        out.push_str("export { default as List } from './List.vue'\n");
    }
    out
}

pub fn build_root_barrel_ts(resource_dirs: &[String]) -> String {
    let mut sorted: Vec<&str> = resource_dirs.iter().map(|s| s.as_str()).collect();
    sorted.sort();
    let mut out = String::new();
    for name in sorted {
        out.push_str(&format!("export * as {name} from './{name}'\n", name = name));
    }
    out
}
