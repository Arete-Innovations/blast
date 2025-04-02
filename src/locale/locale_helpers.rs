use crate::configs::Config;
use serde_json::{json, Map, Value};
use std::fs::{self, read_to_string};
use std::path::Path;

// Get the locale directory from config or use the default
pub fn get_locale_dir(config: &Option<&Config>) -> String {
    if let Some(conf) = config {
        conf.assets
            .get("assets")
            .and_then(|a| a.get("locale"))
            .and_then(|l| l.get("dir"))
            .and_then(|v| v.as_str())
            .unwrap_or(crate::locale::DEFAULT_LOCALE_DIR)
            .to_string()
    } else {
        crate::locale::DEFAULT_LOCALE_DIR.to_string()
    }
}

// Get the default language code from config or return "en" if not found
pub fn get_default_language(config: &Option<&Config>) -> String {
    if let Some(conf) = config {
        conf.assets
            .get("assets")
            .and_then(|a| a.get("locale"))
            .and_then(|l| l.get("default"))
            .and_then(|v| v.as_str())
            .unwrap_or("en")
            .to_string()
    } else {
        "en".to_string()
    }
}

// Get a list of all language files in the locale directory
pub fn get_language_files() -> Vec<String> {
    let locale_dir = get_locale_dir(&None);
    let mut languages = Vec::new();

    if let Ok(entries) = fs::read_dir(locale_dir) {
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |ext| ext == "json") {
                if let Some(file_stem) = path.file_stem() {
                    if let Some(lang_code) = file_stem.to_str() {
                        languages.push(lang_code.to_string());
                    }
                }
            }
        }
    }

    // Sort the languages, with the default language first
    let default_lang = get_default_language(&None);
    languages.sort_by(|a, b| {
        if a == &default_lang {
            std::cmp::Ordering::Less
        } else if b == &default_lang {
            std::cmp::Ordering::Greater
        } else {
            a.cmp(b)
        }
    });

    languages
}

// Extract nested keys from a JSON object to a flattened list
pub fn extract_json_paths(prefix: &str, value: &Value, mut paths: Vec<String>) -> Vec<String> {
    match value {
        Value::Object(obj) => {
            for (key, val) in obj {
                let new_prefix = if prefix.is_empty() { key.clone() } else { format!("{}.{}", prefix, key) };
                paths = extract_json_paths(&new_prefix, val, paths);
            }
        }
        Value::Array(arr) => {
            for (index, val) in arr.iter().enumerate() {
                let new_prefix = format!("{}[{}]", prefix, index);
                paths = extract_json_paths(&new_prefix, val, paths);
            }
        }
        _ => {
            paths.push(prefix.to_string());
        }
    }
    paths
}

// Update a nested key in a JSON object by its dot notation path
pub fn update_nested_key(file_path: &str, key_path: &str, value: &str) -> bool {
    // Load the JSON file
    let json_result: Result<Value, Box<dyn std::error::Error>> = match read_to_string(file_path) {
        Ok(content) => match serde_json::from_str::<Value>(&content) {
            Ok(json) => Ok(json),
            Err(_) => {
                // If the file exists but is invalid JSON, create an empty object
                Ok(json!({}))
            }
        },
        Err(_) => {
            // If the file doesn't exist, create an empty object
            Ok(json!({}))
        }
    };

    let mut json = match json_result {
        Ok(json) => json,
        Err(_) => return false,
    };

    // Split the key path into parts
    let parts: Vec<&str> = key_path.split('.').collect();

    // Helper function to avoid multiple mutable borrows
    fn update_json_path(json: &mut Value, parts: &[&str], value: &str) {
        if parts.is_empty() {
            return;
        }

        if parts.len() == 1 {
            // We're at the leaf node, set the value
            if let Some(obj) = json.as_object_mut() {
                obj.insert(parts[0].to_string(), Value::String(value.to_string()));
            }
        } else {
            // We're at an intermediate node
            let part = parts[0];
            let rest = &parts[1..];

            if let Some(obj) = json.as_object_mut() {
                if !obj.contains_key(part) {
                    obj.insert(part.to_string(), json!({}));
                }

                if let Some(next) = obj.get_mut(part) {
                    update_json_path(next, rest, value);
                }
            }
        }
    }

    update_json_path(&mut json, &parts, value);

    // Save the updated JSON
    match serde_json::to_string_pretty(&json) {
        Ok(formatted) => match fs::write(file_path, formatted) {
            Ok(_) => true,
            Err(_) => false,
        },
        Err(_) => false,
    }
}

// Get a value from a JSON object by its dot notation path
pub fn get_value_from_path<'a>(json: &'a Value, path: &str) -> Option<&'a Value> {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = json;

    for part in parts {
        match current {
            Value::Object(obj) => {
                if let Some(value) = obj.get(part) {
                    current = value;
                } else {
                    return None;
                }
            }
            _ => return None,
        }
    }

    Some(current)
}

// Print a JSON object with indentation
pub fn print_json_content(prefix: &str, value: &Value) {
    match value {
        Value::Object(obj) => {
            for (key, val) in obj {
                let new_prefix = if prefix.is_empty() { key.clone() } else { format!("{}.{}", prefix, key) };
                print_json_content(&new_prefix, val);
            }
        }
        Value::Array(arr) => {
            for (index, val) in arr.iter().enumerate() {
                let new_prefix = format!("{}[{}]", prefix, index);
                print_json_content(&new_prefix, val);
            }
        }
        _ => {
            println!("{}: {}", prefix, value);
        }
    }
}

// Replace all string values in a JSON object with placeholder strings
pub fn replace_with_placeholders(value: &Value) -> Value {
    match value {
        Value::Object(obj) => {
            let mut new_obj = Map::new();
            for (key, val) in obj {
                new_obj.insert(key.clone(), replace_with_placeholders(val));
            }
            Value::Object(new_obj)
        }
        Value::Array(arr) => {
            let new_arr = arr.iter().map(replace_with_placeholders).collect();
            Value::Array(new_arr)
        }
        Value::String(_) => Value::String(crate::locale::TBI_PLACEHOLDER.to_string()),
        _ => value.clone(),
    }
}
