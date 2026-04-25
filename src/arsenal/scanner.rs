use std::collections::BTreeMap;
use std::path::Path;

use quote::ToTokens;
use serde::{Deserialize, Serialize};
use syn::{Item, Visibility};
use walkdir::WalkDir;

use crate::error::{BlastError, BlastResult};

const SCANNED_LAYERS: &[&str] = &[
    "services",
    "routines",
    "models",
    "flows",
    "transport",
    "routes",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub module: String,
    pub name: String,
    pub fqn: String,
    pub signature: String,
    pub doc: String,
    pub side_effects: Vec<String>,
    pub origin: String,
    pub path: String,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteEntry {
    pub method: String,
    pub path: String,
    pub flow: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArsenalReport {
    pub generated_at: String,
    pub layers: BTreeMap<String, Vec<Entry>>,
    pub routes: Vec<RouteEntry>,
}

pub fn scan(project_root: &Path) -> BlastResult<ArsenalReport> {
    let src_dir = project_root.join("src");
    if !src_dir.is_dir() {
        return Err(BlastError::NotFound(format!(
            "src/ not found under {}",
            project_root.display()
        )));
    }

    let mut layers: BTreeMap<String, Vec<Entry>> = BTreeMap::new();
    for layer in SCANNED_LAYERS {
        let layer_dir = src_dir.join(layer);
        if !layer_dir.is_dir() {
            continue;
        }
        let entries = scan_layer(layer, &src_dir, &layer_dir)?;
        if !entries.is_empty() {
            layers.insert((*layer).to_string(), entries);
        }
    }

    let mut routes: Vec<RouteEntry> = Vec::new();
    for layer in &["transport", "routes"] {
        let layer_dir = src_dir.join(layer);
        if !layer_dir.is_dir() {
            continue;
        }
        let mut found = scan_routes(&src_dir, &layer_dir)?;
        routes.append(&mut found);
    }
    routes.sort_by(|a, b| (&a.path, &a.method, &a.flow).cmp(&(&b.path, &b.method, &b.flow)));

    let generated_at = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    Ok(ArsenalReport {
        generated_at,
        layers,
        routes,
    })
}

fn scan_layer(layer: &str, src_dir: &Path, layer_dir: &Path) -> BlastResult<Vec<Entry>> {
    let mut entries: Vec<Entry> = Vec::new();

    for dent in WalkDir::new(layer_dir).into_iter() {
        let dent = match dent {
            Ok(d) => d,
            Err(err) => return Err(BlastError::Walk(err)),
        };
        let path = dent.path();
        if !path.is_file() {
            continue;
        }
        if !is_rust_file(path) {
            continue;
        }
        let file_name = match path.file_name() {
            Some(name) => name.to_string_lossy().to_string(),
            None => continue,
        };
        if file_name == "mod.rs" {
            continue;
        }
        let file_entries = scan_file(layer, src_dir, path)?;
        entries.extend(file_entries);
    }

    entries.sort_by(|a, b| a.fqn.cmp(&b.fqn));
    Ok(entries)
}

fn scan_file(layer: &str, src_dir: &Path, path: &Path) -> BlastResult<Vec<Entry>> {
    let content = std::fs::read_to_string(path)?;
    let parsed = match syn::parse_file(&content) {
        Ok(file) => file,
        Err(err) => {
            return Err(BlastError::Invalid(format!(
                "syn parse {}: {}",
                path.display(),
                err
            )));
        }
    };

    let module_path = derive_module_path(src_dir, path)?;
    let module_label = module_path_label(layer, &module_path);
    let origin = if path.to_string_lossy().contains("/generated/") {
        "generated".to_string()
    } else {
        "custom".to_string()
    };

    let side_effects = detect_side_effects(&parsed);
    let rel_path = match path.strip_prefix(src_dir) {
        Ok(p) => p.to_path_buf(),
        Err(err) => return Err(BlastError::StripPrefix(err)),
    };

    let mut out: Vec<Entry> = Vec::new();
    let line_number_lookup = LineLookup::new(&content);

    for item in &parsed.items {
        let func = match item {
            Item::Fn(f) => f,
            _other_item => continue,
        };
        let is_pub = matches!(func.vis, Visibility::Public(_));
        if !is_pub {
            continue;
        }

        let name = func.sig.ident.to_string();
        let fqn = format!("{}::{}", module_path, name);
        let signature = stringify_signature(&func.sig);
        let doc = extract_doc(&func.attrs);
        let line = line_number_lookup.line_of_ident(&func.sig.ident);

        out.push(Entry {
            module: module_label.clone(),
            name,
            fqn,
            signature,
            doc,
            side_effects: side_effects.clone(),
            origin: origin.clone(),
            path: rel_path.to_string_lossy().to_string(),
            line,
        });
    }

    Ok(out)
}

fn derive_module_path(src_dir: &Path, file: &Path) -> BlastResult<String> {
    let rel = match file.strip_prefix(src_dir) {
        Ok(p) => p,
        Err(err) => return Err(BlastError::StripPrefix(err)),
    };
    let mut parts: Vec<String> = Vec::new();
    let parent = match rel.parent() {
        Some(p) => p,
        None => Path::new(""),
    };
    for comp in parent.components() {
        let s = comp.as_os_str().to_string_lossy().to_string();
        if s.is_empty() {
            continue;
        }
        parts.push(s);
    }
    let stem = match rel.file_stem() {
        Some(s) => s.to_string_lossy().to_string(),
        None => return Err(BlastError::Invalid(format!("no file stem: {}", file.display()))),
    };
    if stem != "mod" {
        parts.push(stem);
    }
    Ok(parts.join("::"))
}

fn module_path_label(layer: &str, module_path: &str) -> String {
    let prefix = format!("{}::", layer);
    let trimmed = match module_path.strip_prefix(&prefix) {
        Some(rest) => rest.to_string(),
        None => module_path.to_string(),
    };
    if trimmed == layer || trimmed.is_empty() {
        return layer.to_string();
    }
    let pieces: Vec<&str> = trimmed.split("::").collect();
    if pieces.len() <= 1 {
        return trimmed;
    }
    let last_idx = pieces.len() - 1;
    pieces[..last_idx].join("::")
}

fn stringify_signature(sig: &syn::Signature) -> String {
    let tokens = sig.to_token_stream();
    let raw = tokens.to_string();
    collapse_whitespace(&raw)
}

fn collapse_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = false;
    for ch in s.chars() {
        let is_ws = ch.is_whitespace();
        if is_ws {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    out.trim().to_string()
}

fn extract_doc(attrs: &[syn::Attribute]) -> String {
    let mut paragraphs: Vec<String> = Vec::new();
    let mut current: Vec<String> = Vec::new();

    for attr in attrs {
        if !attr.path().is_ident("doc") {
            continue;
        }
        let meta = match &attr.meta {
            syn::Meta::NameValue(nv) => nv,
            _other_meta => continue,
        };
        let lit_str = match &meta.value {
            syn::Expr::Lit(expr_lit) => match &expr_lit.lit {
                syn::Lit::Str(s) => s.value(),
                _other_lit => continue,
            },
            _other_expr => continue,
        };
        let trimmed = lit_str.trim().to_string();
        if trimmed.is_empty() {
            if !current.is_empty() {
                paragraphs.push(current.join(" "));
                current.clear();
            }
            continue;
        }
        current.push(trimmed);
    }
    if !current.is_empty() {
        paragraphs.push(current.join(" "));
    }

    match paragraphs.into_iter().next() {
        Some(first) => first,
        None => empty_string(),
    }
}

fn detect_side_effects(file: &syn::File) -> Vec<String> {
    let mut classes: Vec<&'static str> = Vec::new();
    let mut text = String::new();
    for item in &file.items {
        let use_item = match item {
            Item::Use(u) => u,
            _other => continue,
        };
        let toks = use_item.to_token_stream().to_string();
        text.push_str(&toks);
        text.push('\n');
    }

    if text.contains("diesel") {
        classes.push("db");
    }
    if text.contains("reqwest") || text.contains("hyper") || text.contains("tower :: client") {
        classes.push("net");
    }
    if text.contains("tokio :: fs") || text.contains("std :: fs") {
        classes.push("io");
    }
    if classes.is_empty() {
        classes.push("pure");
    }

    let mut out: Vec<String> = classes.into_iter().map(|s| s.to_string()).collect();
    out.sort();
    out.dedup();
    out
}

fn scan_routes(src_dir: &Path, layer_dir: &Path) -> BlastResult<Vec<RouteEntry>> {
    let pattern_str = r#"\.route\s*\(\s*"([^"]+)"\s*,\s*([a-zA-Z_]+)\s*\(\s*([A-Za-z_:][A-Za-z0-9_:]*)\s*\)"#;
    let route_re = match regex::Regex::new(pattern_str) {
        Ok(re) => re,
        Err(err) => return Err(BlastError::Regex(err)),
    };

    let mut entries: Vec<RouteEntry> = Vec::new();
    for dent in WalkDir::new(layer_dir).into_iter() {
        let dent = match dent {
            Ok(d) => d,
            Err(err) => return Err(BlastError::Walk(err)),
        };
        let path = dent.path();
        if !path.is_file() {
            continue;
        }
        if !is_rust_file(path) {
            continue;
        }
        let content = std::fs::read_to_string(path)?;
        let rel = match path.strip_prefix(src_dir) {
            Ok(p) => p.to_string_lossy().to_string(),
            Err(err) => return Err(BlastError::StripPrefix(err)),
        };
        for caps in route_re.captures_iter(&content) {
            let path_match = match caps.get(1) {
                Some(m) => m.as_str().to_string(),
                None => continue,
            };
            let verb = match caps.get(2) {
                Some(m) => m.as_str().to_string(),
                None => continue,
            };
            let target = match caps.get(3) {
                Some(m) => m.as_str().to_string(),
                None => continue,
            };
            let method = method_from_verb(&verb);
            entries.push(RouteEntry {
                method,
                path: path_match,
                flow: target,
                source: rel.clone(),
            });
        }
    }
    Ok(entries)
}

fn is_rust_file(path: &Path) -> bool {
    let ext = match path.extension() {
        Some(e) => e,
        None => { return false; }
    };
    ext == "rs"
}

fn empty_string() -> String {
    String::new()
}

const UNKNOWN_LINE: usize = 0;

fn method_from_verb(verb: &str) -> String {
    match verb {
        "get" => "GET".to_string(),
        "post" => "POST".to_string(),
        "put" => "PUT".to_string(),
        "delete" => "DELETE".to_string(),
        "patch" => "PATCH".to_string(),
        "head" => "HEAD".to_string(),
        "options" => "OPTIONS".to_string(),
        other => other.to_uppercase(),
    }
}

struct LineLookup {
    line_starts: Vec<usize>,
    content: String,
}

impl LineLookup {
    fn new(content: &str) -> Self {
        let mut starts: Vec<usize> = vec![0];
        for (idx, ch) in content.char_indices() {
            if ch == '\n' {
                starts.push(idx + 1);
            }
        }
        Self {
            line_starts: starts,
            content: content.to_string(),
        }
    }

    fn line_of_ident(&self, ident: &syn::Ident) -> usize {
        let needle = format!("fn {}", ident);
        match self.content.find(&needle) {
            Some(byte_idx) => self.byte_to_line(byte_idx),
            None => UNKNOWN_LINE,
        }
    }

    fn byte_to_line(&self, byte_idx: usize) -> usize {
        let mut line = 1usize;
        for (idx, start) in self.line_starts.iter().enumerate() {
            if *start > byte_idx {
                break;
            }
            line = idx + 1;
        }
        line
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn scans_pub_fn_in_services() {
        let dir = tempdir().expect("tempdir");
        let services = dir.path().join("src/services");
        std::fs::create_dir_all(&services).expect("create");
        let mut f = std::fs::File::create(services.join("email.rs")).expect("create");
        let src = "use diesel::prelude::*;\n\
/// Sends plain-text email via SMTP.\n\
pub async fn send(to: &str, subject: &str, body: &str) -> Result<(), String> { Ok(()) }\n\
fn private_helper() {}\n";
        f.write_all(src.as_bytes()).expect("write");

        let report = scan(dir.path()).expect("scan");
        let entries = report.layers.get("services").expect("services");
        assert_eq!(entries.len(), 1);
        let entry = &entries[0];
        assert_eq!(entry.name, "send");
        assert_eq!(entry.fqn, "services::email::send");
        assert_eq!(entry.module, "email");
        assert!(entry.signature.contains("send"));
        assert!(entry.doc.contains("Sends plain-text email"));
        assert!(entry.side_effects.contains(&"db".to_string()));
        assert_eq!(entry.origin, "custom");
    }

    #[test]
    fn detects_generated_origin() {
        let dir = tempdir().expect("tempdir");
        let gen_dir = dir.path().join("src/flows/generated");
        std::fs::create_dir_all(&gen_dir).expect("create");
        let mut f = std::fs::File::create(gen_dir.join("orders.rs")).expect("create");
        f.write_all(b"pub fn list() -> () {}\n").expect("write");

        let report = scan(dir.path()).expect("scan");
        let entries = report.layers.get("flows").expect("flows");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].origin, "generated");
        assert_eq!(entries[0].fqn, "flows::generated::orders::list");
    }

    #[test]
    fn extracts_routes_from_axum() {
        let dir = tempdir().expect("tempdir");
        let routes_dir = dir.path().join("src/routes");
        std::fs::create_dir_all(&routes_dir).expect("create");
        let mut f = std::fs::File::create(routes_dir.join("auth.rs")).expect("create");
        let src = "pub fn router() -> Router {\n\
    Router::new().route(\"/auth/login\", post(login)).route(\"/auth/me\", get(me))\n\
}\n\
pub async fn login() {}\n\
pub async fn me() {}\n";
        f.write_all(src.as_bytes()).expect("write");

        let report = scan(dir.path()).expect("scan");
        assert_eq!(report.routes.len(), 2);
        let login = report.routes.iter().find(|r| r.path == "/auth/login").expect("login");
        assert_eq!(login.method, "POST");
        assert_eq!(login.flow, "login");
    }
}
