use crate::codegen::header;
use crate::codegen::ir_loader;
use crate::codegen::ts_validator;
use crate::error::BlastResult;
use crate::state::{FieldName, FieldState, ResourceState, Verb, VerbState};
use std::fs;
use std::path::{Path, PathBuf};

const MAX_PAGE_SIZE_WIRE: u32 = 200;
const DEFAULT_PAGE_SIZE_WIRE: u32 = 50;

pub fn run_frontend(project_root: &Path) -> BlastResult<()> {
    let resources = ir_loader::load_resource_states(project_root)?;
    emit_validators(project_root, &resources)?;
    emit_list_query_module(project_root)?;
    emit_per_resource_list_helpers(project_root, &resources)?;
    Ok(())
}

fn emit_validators(project_root: &Path, resources: &[ResourceState]) -> BlastResult<()> {
    let out_dir = validators_dir(project_root);
    fs::create_dir_all(&out_dir)?;

    for r in resources {
        let mut body = header::marker_for_resource(project_root, r.name.as_str())?;
        for (name, field) in fields_with_validators(&r.fields) {
            body.push_str(&ts_validator::emit_ts(name, field));
            body.push('\n');
        }
        let file = out_dir.join(format!("{}.ts", r.name.as_str()));
        fs::write(&file, body)?;
    }

    let mut index = header::marker_for_app(project_root)?;
    let mut names: Vec<&str> = resources.iter().map(|r| r.name.as_str()).collect();
    names.sort();
    for name in names {
        index.push_str(&format!("export * from './{}'\n", name));
    }
    fs::write(out_dir.join("index.ts"), index)?;

    Ok(())
}

fn fields_with_validators<'a>(
    fields: &'a indexmap::IndexMap<FieldName, FieldState>,
) -> impl Iterator<Item = (&'a FieldName, &'a FieldState)> {
    fields.iter().filter(|(_, f)| !f.validators.is_empty())
}

fn validators_dir(project_root: &Path) -> PathBuf {
    project_root
        .join("frontend")
        .join("src")
        .join("generated")
        .join("validators")
}

fn generated_dir(project_root: &Path) -> PathBuf {
    project_root.join("frontend").join("src").join("generated")
}

fn queries_dir(project_root: &Path) -> PathBuf {
    generated_dir(project_root).join("queries")
}

fn emit_list_query_module(project_root: &Path) -> BlastResult<()> {
    let dir = generated_dir(project_root);
    fs::create_dir_all(&dir)?;
    let marker = header::marker_for_app(project_root)?;
    let body = format!("{}{}", marker, build_list_query_ts());
    fs::write(dir.join("list_query.ts"), body)?;
    Ok(())
}

fn build_list_query_ts() -> String {
    format!(
        "export const DEFAULT_PAGE_SIZE = {default_size}\n\
export const MAX_PAGE_SIZE = {max_size}\n\
export const DEFAULT_PAGE = 1\n\
\n\
export interface ListQuery {{\n\
  page: number\n\
  page_size: number\n\
  sort?: string\n\
  filters: Record<string, string>\n\
}}\n\
\n\
export function buildListQuery(q: ListQuery): URLSearchParams {{\n\
  const out = new URLSearchParams()\n\
  out.set('page', String(q.page))\n\
  out.set('page_size', String(q.page_size))\n\
  if (q.sort !== undefined && q.sort !== '') {{\n\
    out.set('sort', q.sort)\n\
  }}\n\
  for (const [col, val] of Object.entries(q.filters)) {{\n\
    out.append(`filter[${{col}}]`, val)\n\
  }}\n\
  return out\n\
}}\n\
\n\
export function parseListQuery(s: string, maxPageSize: number = MAX_PAGE_SIZE): ListQuery {{\n\
  const params = new URLSearchParams(s)\n\
  let page = DEFAULT_PAGE\n\
  let page_size = DEFAULT_PAGE_SIZE\n\
  let sort: string | undefined\n\
  const filters: Record<string, string> = {{}}\n\
\n\
  const rawPage = params.get('page')\n\
  if (rawPage !== null) {{\n\
    const n = Number(rawPage)\n\
    if (Number.isInteger(n) && n >= 1) {{\n\
      page = n\n\
    }}\n\
  }}\n\
\n\
  const rawSize = params.get('page_size')\n\
  if (rawSize !== null) {{\n\
    const n = Number(rawSize)\n\
    if (Number.isInteger(n) && n >= 1) {{\n\
      page_size = Math.min(n, maxPageSize)\n\
    }}\n\
  }}\n\
\n\
  const rawSort = params.get('sort')\n\
  if (rawSort !== null && rawSort !== '') {{\n\
    sort = rawSort\n\
  }}\n\
\n\
  for (const [key, value] of params.entries()) {{\n\
    const m = /^filter\\[(.+)\\]$/.exec(key)\n\
    if (m !== null && m[1].length > 0) {{\n\
      filters[m[1]] = value\n\
    }}\n\
  }}\n\
\n\
  return {{ page, page_size, sort, filters }}\n\
}}\n",
        default_size = DEFAULT_PAGE_SIZE_WIRE,
        max_size = MAX_PAGE_SIZE_WIRE,
    )
}

fn emit_per_resource_list_helpers(
    project_root: &Path,
    resources: &[ResourceState],
) -> BlastResult<()> {
    let dir = queries_dir(project_root);
    fs::create_dir_all(&dir)?;

    let mut emitted_names: Vec<&str> = Vec::new();
    for r in resources {
        let list_verb = r.verbs.get(&Verb::List);
        match list_verb {
            Some(verb) => {
                emitted_names.push(r.name.as_str());
                let marker = header::marker_for_resource(project_root, r.name.as_str())?;
                let body = format!("{}{}", marker, build_resource_list_ts(verb));
                fs::write(dir.join(format!("{}_list.ts", r.name.as_str())), body)?;
            }
            None => continue,
        }
    }

    let mut index = header::marker_for_app(project_root)?;
    emitted_names.sort();
    for name in emitted_names {
        index.push_str(&format!("export * from './{}_list'\n", name));
    }
    fs::write(dir.join("index.ts"), index)?;

    Ok(())
}

fn build_resource_list_ts(verb: &VerbState) -> String {
    let (sortable, filterable, default_sort, max_page) = match &verb.list_options {
        Some(opts) => {
            let sortable = ts_field_name_set(opts.sortable_columns.iter());
            let filterable = ts_field_name_set(opts.filterable_columns.keys());
            let default_sort = match &opts.default_sort {
                Some(s) => {
                    let raw: &str = s.as_str();
                    format!("'{}'", raw.replace('\\', "\\\\").replace('\'', "\\'"))
                }
                None => "undefined".to_string(),
            };
            let max_page = match opts.max_page_size {
                Some(v) => v,
                None => MAX_PAGE_SIZE_WIRE,
            };
            (sortable, filterable, default_sort, max_page)
        }
        None => (
            "[]".to_string(),
            "[]".to_string(),
            "undefined".to_string(),
            MAX_PAGE_SIZE_WIRE,
        ),
    };

    format!(
        "export const SORTABLE_COLUMNS = {sortable} as const\n\
export const FILTERABLE_COLUMNS = {filterable} as const\n\
export const DEFAULT_SORT: string | undefined = {default_sort}\n\
export const MAX_PAGE_SIZE: number = {max_page}\n\
\n\
export type SortableColumn = typeof SORTABLE_COLUMNS[number]\n\
export type FilterableColumn = typeof FILTERABLE_COLUMNS[number]\n\
\n\
export function isSortable(col: string): col is SortableColumn {{\n\
  return (SORTABLE_COLUMNS as readonly string[]).includes(col)\n\
}}\n\
\n\
export function isFilterable(col: string): col is FilterableColumn {{\n\
  return (FILTERABLE_COLUMNS as readonly string[]).includes(col)\n\
}}\n",
    )
}

fn ts_field_name_set<'a, I>(values: I) -> String
where
    I: IntoIterator<Item = &'a FieldName>,
{
    let parts: Vec<String> = values
        .into_iter()
        .map(|s: &FieldName| format!("'{}'", s.as_str().replace('\\', "\\\\").replace('\'', "\\'")))
        .collect();
    if parts.is_empty() {
        return "[]".to_string();
    }
    format!("[{}]", parts.join(", "))
}
