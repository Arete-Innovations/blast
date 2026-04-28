use std::io::{BufRead, BufReader, Write};

use serde_json::{json, Value};

use crate::{
    arsenal::scanner::{ArsenalReport, Entry},
    error::{BlastError, BlastResult},
};

const PROTOCOL_VERSION: &str = "2024-11-05";
const SERVER_NAME: &str = "blast-arsenal";
const SERVER_VERSION: &str = "0.1.0";

pub fn serve(report: ArsenalReport) -> BlastResult<()> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = stdout.lock();

    loop {
        let mut line = String::new();
        let read = reader.read_line(&mut line)?;
        if read == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(err) => {
                let resp = error_response(Value::Null, -32700, &format!("parse error: {}", err));
                write_message(&mut writer, &resp)?;
                continue;
            }
        };
        let outcome = handle_message(&report, &msg);
        match outcome {
            Outcome::Reply(resp) => write_message(&mut writer, &resp)?,
            Outcome::Quiet => continue,
            Outcome::Stop => break,
        }
    }

    Ok(())
}

enum Outcome {
    Reply(Value),
    Quiet,
    Stop,
}

fn handle_message(report: &ArsenalReport, msg: &Value) -> Outcome {
    let id = match msg.get("id") {
        Some(v) => v.clone(),
        None => Value::Null,
    };
    let method = match msg.get("method").and_then(|m| m.as_str()) {
        Some(m) => m.to_string(),
        None => {
            return Outcome::Reply(error_response(id, -32600, "missing method"));
        }
    };
    let params = match msg.get("params") {
        Some(v) => v.clone(),
        None => Value::Null,
    };
    let is_notification = msg.get("id").is_none();

    match method.as_str() {
        "initialize" => Outcome::Reply(success(id, initialize_result())),
        "initialized" | "notifications/initialized" => Outcome::Quiet,
        "shutdown" => Outcome::Reply(success(id, json!({}))),
        "exit" => Outcome::Stop,
        "ping" => Outcome::Reply(success(id, json!({}))),
        "tools/list" => Outcome::Reply(success(id, tools_list_result())),
        "tools/call" => {
            let result = handle_tool_call(report, &params);
            match result {
                Ok(content) => Outcome::Reply(success(id, content)),
                Err(err) => Outcome::Reply(error_response(id, -32000, &err.to_string())),
            }
        }
        _other_method => {
            if is_notification {
                Outcome::Quiet
            } else {
                Outcome::Reply(error_response(id, -32601, &format!("method not found: {}", method)))
            }
        }
    }
}

fn initialize_result() -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": { "tools": {} },
        "serverInfo": {
            "name": SERVER_NAME,
            "version": SERVER_VERSION,
        },
    })
}

fn tools_list_result() -> Value {
    json!({
        "tools": [
            {
                "name": "list",
                "description": "List arsenal entries, optionally filtered by layer (services, routines, models, flows, transport).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "layer": { "type": "string" }
                    }
                }
            },
            {
                "name": "search",
                "description": "Fuzzy substring match on entry name, fqn, and doc.",
                "inputSchema": {
                    "type": "object",
                    "required": ["query"],
                    "properties": {
                        "query": { "type": "string" }
                    }
                }
            },
            {
                "name": "describe",
                "description": "Describe a single entry by fully qualified name.",
                "inputSchema": {
                    "type": "object",
                    "required": ["fqn"],
                    "properties": {
                        "fqn": { "type": "string" }
                    }
                }
            },
            {
                "name": "routes",
                "description": "Return the transport-layer route to flow mapping.",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            }
        ]
    })
}

fn handle_tool_call(report: &ArsenalReport, params: &Value) -> BlastResult<Value> {
    let name = match params.get("name").and_then(|n| n.as_str()) {
        Some(n) => n.to_string(),
        None => return Err(BlastError::Invalid("missing tool name".to_string())),
    };
    let args = match params.get("arguments") {
        Some(v) => v.clone(),
        None => Value::Null,
    };

    let payload: Value = match name.as_str() {
        "list" => tool_list(report, &args),
        "search" => tool_search(report, &args)?,
        "describe" => tool_describe(report, &args)?,
        "routes" => tool_routes(report),
        other => {
            return Err(BlastError::Invalid(format!("unknown tool: {}", other)));
        }
    };

    let pretty = serde_json::to_string_pretty(&payload)?;
    Ok(json!({
        "content": [
            { "type": "text", "text": pretty }
        ],
        "structuredContent": payload,
        "isError": false
    }))
}

fn tool_list(report: &ArsenalReport, args: &Value) -> Value {
    let layer_filter = args.get("layer").and_then(|l| l.as_str());
    let mut all: Vec<&Entry> = Vec::new();
    for (layer, entries) in &report.layers {
        if !layer_matches(layer_filter, layer) {
            continue;
        }
        for e in entries {
            all.push(e);
        }
    }
    json!({ "entries": all })
}

fn layer_matches(filter: Option<&str>, layer: &str) -> bool {
    match filter {
        Some(f) => f == layer,
        None => {
            return true;
        }
    }
}

fn tool_search(report: &ArsenalReport, args: &Value) -> BlastResult<Value> {
    let query = match args.get("query").and_then(|q| q.as_str()) {
        Some(q) => q.to_lowercase(),
        None => return Err(BlastError::Invalid("missing arg: query".to_string())),
    };
    let mut hits: Vec<&Entry> = Vec::new();
    for entries in report.layers.values() {
        for entry in entries {
            let blob = format!("{} {} {}", entry.name.to_lowercase(), entry.fqn.to_lowercase(), entry.doc.to_lowercase());
            if blob.contains(&query) {
                hits.push(entry);
            }
        }
    }
    Ok(json!({ "hits": hits }))
}

fn tool_describe(report: &ArsenalReport, args: &Value) -> BlastResult<Value> {
    let fqn = match args.get("fqn").and_then(|f| f.as_str()) {
        Some(f) => f.to_string(),
        None => return Err(BlastError::Invalid("missing arg: fqn".to_string())),
    };
    for entries in report.layers.values() {
        for entry in entries {
            if entry.fqn == fqn {
                return Ok(json!({ "entry": entry }));
            }
        }
    }
    Err(BlastError::NotFound(fqn))
}

fn tool_routes(report: &ArsenalReport) -> Value {
    json!({ "routes": report.routes })
}

fn success(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    })
}

fn error_response(id: Value, code: i32, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    })
}

fn write_message<W: Write>(writer: &mut W, msg: &Value) -> BlastResult<()> {
    let serialized = match serde_json::to_string(msg) {
        Ok(s) => s,
        Err(err) => return Err(BlastError::Json(err)),
    };
    writer.write_all(serialized.as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::arsenal::scanner::{Entry, RouteEntry};

    fn fixture() -> ArsenalReport {
        let mut layers: BTreeMap<String, Vec<Entry>> = BTreeMap::new();
        layers.insert(
            "services".to_string(),
            vec![Entry {
                module: "email".to_string(),
                name: "send".to_string(),
                fqn: "services::email::send".to_string(),
                signature: "pub async fn send(to: & str)".to_string(),
                doc: "Sends mail.".to_string(),
                side_effects: vec!["net".to_string()],
                origin: "custom".to_string(),
                path: "services/email.rs".to_string(),
                line: 1,
            }],
        );
        layers.insert(
            "flows".to_string(),
            vec![Entry {
                module: "auth".to_string(),
                name: "login".to_string(),
                fqn: "flows::auth::login".to_string(),
                signature: "pub async fn login()".to_string(),
                doc: "Authenticates a user.".to_string(),
                side_effects: vec!["db".to_string()],
                origin: "custom".to_string(),
                path: "flows/auth.rs".to_string(),
                line: 5,
            }],
        );
        ArsenalReport {
            generated_at: "2026-04-25T00:00:00Z".to_string(),
            layers,
            routes: vec![RouteEntry {
                method: "POST".to_string(),
                path: "/auth/login".to_string(),
                flow: "login".to_string(),
                source: "routes/auth.rs".to_string(),
            }],
        }
    }

    #[test]
    fn initialize_returns_server_info() {
        let report = fixture();
        let req = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {}
        });
        let outcome = handle_message(&report, &req);
        let resp = match outcome {
            Outcome::Reply(v) => v,
            _other => panic!("expected reply"),
        };
        let server_name = resp.pointer("/result/serverInfo/name").expect("name").as_str().expect("str");
        assert_eq!(server_name, SERVER_NAME);
        let caps = resp.pointer("/result/capabilities/tools").expect("caps");
        assert!(caps.is_object());
    }

    #[test]
    fn tools_list_advertises_four_tools() {
        let report = fixture();
        let req = json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"});
        let outcome = handle_message(&report, &req);
        let resp = match outcome {
            Outcome::Reply(v) => v,
            _other => panic!("expected reply"),
        };
        let tools = resp.pointer("/result/tools").expect("tools").as_array().expect("arr");
        assert_eq!(tools.len(), 4);
        let names: Vec<&str> = tools.iter().filter_map(|t| t.get("name").and_then(|n| n.as_str())).collect();
        assert!(names.contains(&"list"));
        assert!(names.contains(&"search"));
        assert!(names.contains(&"describe"));
        assert!(names.contains(&"routes"));
    }

    #[test]
    fn tool_search_finds_entries() {
        let report = fixture();
        let req = json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": { "name": "search", "arguments": { "query": "mail" } }
        });
        let outcome = handle_message(&report, &req);
        let resp = match outcome {
            Outcome::Reply(v) => v,
            _other => panic!("expected reply"),
        };
        let hits = resp.pointer("/result/structuredContent/hits").expect("hits").as_array().expect("arr");
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn tool_describe_returns_entry() {
        let report = fixture();
        let req = json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": { "name": "describe", "arguments": { "fqn": "flows::auth::login" } }
        });
        let outcome = handle_message(&report, &req);
        let resp = match outcome {
            Outcome::Reply(v) => v,
            _other => panic!("expected reply"),
        };
        let name = resp.pointer("/result/structuredContent/entry/name").expect("name").as_str().expect("str");
        assert_eq!(name, "login");
    }

    #[test]
    fn tool_describe_unknown_fqn_errors() {
        let report = fixture();
        let req = json!({
            "jsonrpc": "2.0",
            "id": 5,
            "method": "tools/call",
            "params": { "name": "describe", "arguments": { "fqn": "nope::nope" } }
        });
        let outcome = handle_message(&report, &req);
        let resp = match outcome {
            Outcome::Reply(v) => v,
            _other => panic!("expected reply"),
        };
        assert!(resp.pointer("/error").is_some());
    }

    #[test]
    fn tool_routes_returns_routes() {
        let report = fixture();
        let req = json!({
            "jsonrpc": "2.0",
            "id": 6,
            "method": "tools/call",
            "params": { "name": "routes" }
        });
        let outcome = handle_message(&report, &req);
        let resp = match outcome {
            Outcome::Reply(v) => v,
            _other => panic!("expected reply"),
        };
        let routes = resp.pointer("/result/structuredContent/routes").expect("routes").as_array().expect("arr");
        assert_eq!(routes.len(), 1);
    }

    #[test]
    fn unknown_method_returns_jsonrpc_error() {
        let report = fixture();
        let req = json!({"jsonrpc": "2.0", "id": 7, "method": "nope/nope"});
        let outcome = handle_message(&report, &req);
        let resp = match outcome {
            Outcome::Reply(v) => v,
            _other => panic!("expected reply"),
        };
        let code = resp.pointer("/error/code").expect("code").as_i64().expect("i64");
        assert_eq!(code, -32601);
    }

    #[test]
    fn notifications_get_no_reply() {
        let report = fixture();
        let req = json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
        let outcome = handle_message(&report, &req);
        match outcome {
            Outcome::Quiet => {}
            _other => panic!("expected quiet"),
        }
    }
}
