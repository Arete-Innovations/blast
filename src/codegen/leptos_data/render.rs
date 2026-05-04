use crate::{
    codegen::{leptos_forms::render::primary_key_field, structs::naming::type_stem_for_resource},
    state::{ResourceState, Verb},
};

pub fn render_resource_helpers(resource: &ResourceState) -> String {
    let table = resource.name.as_str();
    let stem = type_stem_for_resource(resource);
    let public_ty = format!("{stem}Public");
    let insertable_ty = format!("{stem}Insertable");
    let patch_ty = format!("{stem}Patch");
    let pk_ty = pk_rust_type(resource);

    let has_list = resource.verbs.contains_key(&Verb::List);
    let has_get = resource.verbs.contains_key(&Verb::Get);
    let has_create = resource.verbs.contains_key(&Verb::Create);
    let has_update = resource.verbs.contains_key(&Verb::Update) && primary_key_field(resource).is_some();
    let has_delete = resource.verbs.contains_key(&Verb::Delete) && primary_key_field(resource).is_some();

    let mut out = String::new();
    out.push_str("use crate::meltdown::MeltDown;\n");
    let mut ty_imports: Vec<&str> = vec![public_ty.as_str()];
    if has_create {
        ty_imports.push(insertable_ty.as_str());
    }
    if has_update {
        ty_imports.push(patch_ty.as_str());
    }
    out.push_str(&format!("use crate::structs::generated::{table}::{{{}}};\n", ty_imports.join(", ")));
    if has_list {
        out.push_str("use crate::structs::list_query::{ListQuery, ListResponse};\n");
    }
    out.push('\n');

    if has_list {
        out.push_str(&render_list_helper(table, &public_ty));
    }
    if has_get {
        out.push_str(&render_get_helper(table, &public_ty, &pk_ty));
    }
    if has_create {
        out.push_str(&render_create_helper(table, &public_ty, &insertable_ty));
    }
    if has_update {
        out.push_str(&render_update_helper(table, &public_ty, &patch_ty, &pk_ty));
    }
    if has_delete {
        out.push_str(&render_delete_helper(table, &pk_ty));
    }

    if has_list {
        out.push_str(&render_query_string_helper());
    }

    out
}

fn render_list_helper(table: &str, public_ty: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("pub async fn load_{table}_list(query: ListQuery) -> ::std::result::Result<ListResponse<{public_ty}>, MeltDown> {{\n"));
    out.push_str("    #[cfg(not(target_arch = \"wasm32\"))]\n");
    out.push_str("    {\n");
    out.push_str("        let ctx = ::leptos::prelude::expect_context::<crate::ctx::Ctx>();\n");
    out.push_str(&format!("        crate::flows::generated::{table}::list::run(&ctx, query).await\n"));
    out.push_str("    }\n");
    out.push_str("    #[cfg(target_arch = \"wasm32\")]\n");
    out.push_str("    {\n");
    out.push_str(&format!("        let path = format!(\"/api/{table}?{{}}\", query_to_query_string(&query));\n"));
    out.push_str("        crate::transport::leptos::api_client::get_json(&path).await\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");
    out
}

fn render_get_helper(table: &str, public_ty: &str, pk_ty: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("pub async fn load_{table}_one(id: {pk_ty}) -> ::std::result::Result<{public_ty}, MeltDown> {{\n"));
    out.push_str("    #[cfg(not(target_arch = \"wasm32\"))]\n");
    out.push_str("    {\n");
    out.push_str("        let ctx = ::leptos::prelude::expect_context::<crate::ctx::Ctx>();\n");
    out.push_str(&format!("        crate::flows::generated::{table}::get::run(&ctx, id).await\n"));
    out.push_str("    }\n");
    out.push_str("    #[cfg(target_arch = \"wasm32\")]\n");
    out.push_str("    {\n");
    out.push_str(&format!("        let path = format!(\"/api/{table}/{{}}\", id);\n"));
    out.push_str("        crate::transport::leptos::api_client::get_json(&path).await\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");
    out
}

fn render_create_helper(table: &str, public_ty: &str, insertable_ty: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("pub async fn do_{table}_create(input: {insertable_ty}) -> ::std::result::Result<{public_ty}, MeltDown> {{\n"));
    out.push_str("    #[cfg(not(target_arch = \"wasm32\"))]\n");
    out.push_str("    {\n");
    out.push_str("        let ctx = ::leptos::prelude::expect_context::<crate::ctx::Ctx>();\n");
    out.push_str(&format!("        crate::flows::generated::{table}::create::run(&ctx, input).await\n"));
    out.push_str("    }\n");
    out.push_str("    #[cfg(target_arch = \"wasm32\")]\n");
    out.push_str("    {\n");
    out.push_str(&format!("        crate::transport::leptos::api_client::post_json(\"/api/{table}\", &input).await\n"));
    out.push_str("    }\n");
    out.push_str("}\n\n");
    out
}

fn render_update_helper(table: &str, public_ty: &str, patch_ty: &str, pk_ty: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("pub async fn do_{table}_update(id: {pk_ty}, patch: {patch_ty}) -> ::std::result::Result<{public_ty}, MeltDown> {{\n"));
    out.push_str("    #[cfg(not(target_arch = \"wasm32\"))]\n");
    out.push_str("    {\n");
    out.push_str("        let ctx = ::leptos::prelude::expect_context::<crate::ctx::Ctx>();\n");
    out.push_str(&format!("        crate::flows::generated::{table}::update::run(&ctx, id, patch).await\n"));
    out.push_str("    }\n");
    out.push_str("    #[cfg(target_arch = \"wasm32\")]\n");
    out.push_str("    {\n");
    out.push_str(&format!("        let path = format!(\"/api/{table}/{{}}\", id);\n"));
    out.push_str("        crate::transport::leptos::api_client::patch_json(&path, &patch).await\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");
    out
}

fn render_delete_helper(table: &str, pk_ty: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("pub async fn do_{table}_delete(id: {pk_ty}) -> ::std::result::Result<(), MeltDown> {{\n"));
    out.push_str("    #[cfg(not(target_arch = \"wasm32\"))]\n");
    out.push_str("    {\n");
    out.push_str("        let ctx = ::leptos::prelude::expect_context::<crate::ctx::Ctx>();\n");
    out.push_str(&format!("        crate::flows::generated::{table}::delete::run(&ctx, id).await\n"));
    out.push_str("    }\n");
    out.push_str("    #[cfg(target_arch = \"wasm32\")]\n");
    out.push_str("    {\n");
    out.push_str(&format!("        let path = format!(\"/api/{table}/{{}}\", id);\n"));
    out.push_str("        crate::transport::leptos::api_client::delete(&path).await\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");
    out
}

fn render_query_string_helper() -> String {
    let mut out = String::new();
    out.push_str("#[cfg(target_arch = \"wasm32\")]\n");
    out.push_str("fn query_to_query_string(query: &ListQuery) -> String {\n");
    out.push_str("    let mut parts: Vec<String> = Vec::new();\n");
    out.push_str("    parts.push(format!(\"page={}\", query.page));\n");
    out.push_str("    parts.push(format!(\"page_size={}\", query.page_size));\n");
    out.push_str("    for sort in &query.sort {\n");
    out.push_str("        let prefix = match sort.direction {\n");
    out.push_str("            crate::structs::list_query::SortDirection::Asc => \"\",\n");
    out.push_str("            crate::structs::list_query::SortDirection::Desc => \"-\",\n");
    out.push_str("        };\n");
    out.push_str("        parts.push(format!(\"sort={}{}\", prefix, sort.column));\n");
    out.push_str("    }\n");
    out.push_str("    for (col, val) in &query.filter {\n");
    out.push_str("        parts.push(format!(\"filter[{}]={}\", col, val));\n");
    out.push_str("    }\n");
    out.push_str("    parts.join(\"&\")\n");
    out.push_str("}\n\n");
    out
}

fn pk_rust_type(resource: &ResourceState) -> String {
    match primary_key_field(resource) {
        Some((_pk_name, field)) => map_pk_sql_to_rust(field.sql_type.as_str(), field.nullable),
        None => "i64".to_string(),
    }
}

fn map_pk_sql_to_rust(sql: &str, nullable: bool) -> String {
    let base = match sql.to_ascii_lowercase().as_str() {
        "bool" | "boolean" => "bool",
        "int2" | "smallint" | "smallserial" => "i16",
        "int4" | "integer" | "serial" => "i32",
        "int8" | "bigint" | "bigserial" => "i64",
        "uuid" => "uuid::Uuid",
        "text" | "varchar" | "bpchar" | "char" | "citext" => "String",
        _other => "i64",
    };
    match nullable {
        true => format!("Option<{base}>"),
        false => base.to_string(),
    }
}

pub fn render_top_data_barrel(tables: &[&str]) -> String {
    let mut sorted: Vec<&&str> = tables.iter().collect();
    sorted.sort();
    let mut out = String::new();
    for t in &sorted {
        out.push_str(&format!("pub mod {t};\n"));
    }
    out
}

pub fn render_api_client_module() -> String {
    let mut out = String::new();
    out.push_str("#[cfg(target_arch = \"wasm32\")]\n");
    out.push_str("use crate::meltdown::{MeltDown, MeltType};\n");
    out.push_str("#[cfg(target_arch = \"wasm32\")]\n");
    out.push_str("use serde::{de::DeserializeOwned, Serialize};\n");
    out.push('\n');

    out.push_str("#[cfg(target_arch = \"wasm32\")]\n");
    out.push_str("pub async fn get_json<T: DeserializeOwned>(path: &str) -> ::std::result::Result<T, MeltDown> {\n");
    out.push_str("    let response = match ::gloo_net::http::Request::get(path).send().await {\n");
    out.push_str("        Ok(r) => r,\n");
    out.push_str("        Err(net_err) => return Err(network_error(net_err)),\n");
    out.push_str("    };\n");
    out.push_str("    decode_response(response).await\n");
    out.push_str("}\n\n");

    out.push_str("#[cfg(target_arch = \"wasm32\")]\n");
    out.push_str("pub async fn post_json<I: Serialize, T: DeserializeOwned>(path: &str, input: &I) -> ::std::result::Result<T, MeltDown> {\n");
    out.push_str("    let request = match ::gloo_net::http::Request::post(path).json(input) {\n");
    out.push_str("        Ok(req) => req,\n");
    out.push_str("        Err(json_err) => return Err(serialize_error(json_err)),\n");
    out.push_str("    };\n");
    out.push_str("    let response = match request.send().await {\n");
    out.push_str("        Ok(r) => r,\n");
    out.push_str("        Err(net_err) => return Err(network_error(net_err)),\n");
    out.push_str("    };\n");
    out.push_str("    decode_response(response).await\n");
    out.push_str("}\n\n");

    out.push_str("#[cfg(target_arch = \"wasm32\")]\n");
    out.push_str("pub async fn patch_json<I: Serialize, T: DeserializeOwned>(path: &str, input: &I) -> ::std::result::Result<T, MeltDown> {\n");
    out.push_str("    let request = match ::gloo_net::http::Request::patch(path).json(input) {\n");
    out.push_str("        Ok(req) => req,\n");
    out.push_str("        Err(json_err) => return Err(serialize_error(json_err)),\n");
    out.push_str("    };\n");
    out.push_str("    let response = match request.send().await {\n");
    out.push_str("        Ok(r) => r,\n");
    out.push_str("        Err(net_err) => return Err(network_error(net_err)),\n");
    out.push_str("    };\n");
    out.push_str("    decode_response(response).await\n");
    out.push_str("}\n\n");

    out.push_str("#[cfg(target_arch = \"wasm32\")]\n");
    out.push_str("pub async fn delete(path: &str) -> ::std::result::Result<(), MeltDown> {\n");
    out.push_str("    let response = match ::gloo_net::http::Request::delete(path).send().await {\n");
    out.push_str("        Ok(r) => r,\n");
    out.push_str("        Err(net_err) => return Err(network_error(net_err)),\n");
    out.push_str("    };\n");
    out.push_str("    if status_is_success(response.status()) {\n");
    out.push_str("        return Ok(());\n");
    out.push_str("    }\n");
    out.push_str("    Err(http_error(response).await)\n");
    out.push_str("}\n\n");

    out.push_str("#[cfg(target_arch = \"wasm32\")]\n");
    out.push_str("async fn decode_response<T: DeserializeOwned>(response: ::gloo_net::http::Response) -> ::std::result::Result<T, MeltDown> {\n");
    out.push_str("    if !status_is_success(response.status()) {\n");
    out.push_str("        return Err(http_error(response).await);\n");
    out.push_str("    }\n");
    out.push_str("    match response.json::<T>().await {\n");
    out.push_str("        Ok(value) => Ok(value),\n");
    out.push_str("        Err(decode_err) => Err(MeltDown::new(MeltType::DeserializationFailed, format!(\"failed to decode response body: {}\", decode_err))),\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");

    out.push_str("#[cfg(target_arch = \"wasm32\")]\n");
    out.push_str("async fn http_error(response: ::gloo_net::http::Response) -> MeltDown {\n");
    out.push_str("    let status = response.status();\n");
    out.push_str("    let body = match response.text().await {\n");
    out.push_str("        Ok(b) => b,\n");
    out.push_str("        Err(read_err) => format!(\"<failed to read body: {}>\", read_err),\n");
    out.push_str("    };\n");
    out.push_str("    match ::serde_json::from_str::<MeltDownEnvelope>(&body) {\n");
    out.push_str("        Ok(parsed) => MeltDown::new(parsed.melt_type_to_meltdown(), parsed.details).with_user_message(parsed.message),\n");
    out.push_str("        Err(parse_err) => {\n");
    out.push_str("            drop(parse_err);\n");
    out.push_str("            MeltDown::new(MeltType::ExternalServiceError, format!(\"HTTP {}: {}\", status, body))\n");
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");

    out.push_str("#[cfg(target_arch = \"wasm32\")]\n");
    out.push_str("fn network_error(err: ::gloo_net::Error) -> MeltDown {\n");
    out.push_str("    MeltDown::new(MeltType::ExternalServiceError, format!(\"network error: {}\", err))\n");
    out.push_str("}\n\n");

    out.push_str("#[cfg(target_arch = \"wasm32\")]\n");
    out.push_str("fn serialize_error(err: ::gloo_net::Error) -> MeltDown {\n");
    out.push_str("    MeltDown::new(MeltType::SerializationFailed, format!(\"failed to encode request body: {}\", err))\n");
    out.push_str("}\n\n");

    out.push_str("#[cfg(target_arch = \"wasm32\")]\n");
    out.push_str("fn status_is_success(status: u16) -> bool {\n");
    out.push_str("    status >= 200 && status < 300\n");
    out.push_str("}\n\n");

    out.push_str("#[cfg(target_arch = \"wasm32\")]\n");
    out.push_str("#[derive(::serde::Deserialize)]\n");
    out.push_str("struct MeltDownEnvelope {\n");
    out.push_str("    #[serde(default)]\n");
    out.push_str("    pub melt_type: String,\n");
    out.push_str("    #[serde(default)]\n");
    out.push_str("    pub details: String,\n");
    out.push_str("    #[serde(default)]\n");
    out.push_str("    pub message: String,\n");
    out.push_str("}\n\n");

    out.push_str("#[cfg(target_arch = \"wasm32\")]\n");
    out.push_str("impl MeltDownEnvelope {\n");
    out.push_str("    fn melt_type_to_meltdown(&self) -> MeltType {\n");
    out.push_str("        match self.melt_type.as_str() {\n");
    out.push_str("            \"DatabaseConnection\" => MeltType::DatabaseConnection,\n");
    out.push_str("            \"DatabaseError\" => MeltType::DatabaseError,\n");
    out.push_str("            \"RecordNotFound\" => MeltType::RecordNotFound,\n");
    out.push_str("            \"UniqueViolation\" => MeltType::UniqueViolation,\n");
    out.push_str("            \"ForeignKeyViolation\" => MeltType::ForeignKeyViolation,\n");
    out.push_str("            \"CheckViolation\" => MeltType::CheckViolation,\n");
    out.push_str("            \"NotNullViolation\" => MeltType::NotNullViolation,\n");
    out.push_str("            \"AuthRejected\" => MeltType::AuthRejected,\n");
    out.push_str("            \"SessionExpired\" => MeltType::SessionExpired,\n");
    out.push_str("            \"SessionInvalid\" => MeltType::SessionInvalid,\n");
    out.push_str("            \"SessionMissing\" => MeltType::SessionMissing,\n");
    out.push_str("            \"InsufficientPermissions\" => MeltType::InsufficientPermissions,\n");
    out.push_str("            \"ValidationFailed\" => MeltType::ValidationFailed,\n");
    out.push_str("            \"BadRequest\" => MeltType::BadRequest,\n");
    out.push_str("            \"Unauthorized\" => MeltType::Unauthorized,\n");
    out.push_str("            \"Forbidden\" => MeltType::Forbidden,\n");
    out.push_str("            \"NotFound\" => MeltType::NotFound,\n");
    out.push_str("            \"Conflict\" => MeltType::Conflict,\n");
    out.push_str("            \"UnprocessableEntity\" => MeltType::UnprocessableEntity,\n");
    out.push_str("            \"MethodNotAllowed\" => MeltType::MethodNotAllowed,\n");
    out.push_str("            \"TooManyRequests\" => MeltType::TooManyRequests,\n");
    out.push_str("            \"FileNotFound\" => MeltType::FileNotFound,\n");
    out.push_str("            \"FilePermissionDenied\" => MeltType::FilePermissionDenied,\n");
    out.push_str("            \"FileOperationFailed\" => MeltType::FileOperationFailed,\n");
    out.push_str("            \"SerializationFailed\" => MeltType::SerializationFailed,\n");
    out.push_str("            \"DeserializationFailed\" => MeltType::DeserializationFailed,\n");
    out.push_str("            \"ConfigurationError\" => MeltType::ConfigurationError,\n");
    out.push_str("            \"EnvironmentError\" => MeltType::EnvironmentError,\n");
    out.push_str("            \"ExternalServiceError\" => MeltType::ExternalServiceError,\n");
    out.push_str("            other => MeltType::Unexpected(other.to_string()),\n");
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("}\n");
    out
}
