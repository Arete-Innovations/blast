use gloo_net::http::Request;
use serde::{de::DeserializeOwned, Serialize};

use crate::meltdown::{MeltDown, MeltType};
use crate::structs::leptos::ErrorEnvelope;

fn map_response_error(envelope: ErrorEnvelope) -> MeltDown {
    let kind = match envelope.error.melt_type.as_str() {
        "AuthRejected" => MeltType::AuthRejected,
        "SessionMissing" => MeltType::SessionMissing,
        "SessionInvalid" => MeltType::SessionInvalid,
        "SessionExpired" => MeltType::SessionExpired,
        "InsufficientPermissions" => MeltType::InsufficientPermissions,
        "ValidationFailed" => MeltType::ValidationFailed,
        "RecordNotFound" => MeltType::RecordNotFound,
        "UniqueViolation" => MeltType::UniqueViolation,
        "BadRequest" => MeltType::BadRequest,
        _other => MeltType::Unexpected(envelope.error.melt_type),
    };
    let msg = envelope.error.message;
    let melt = MeltDown::new(kind, msg.clone());
    if msg.trim().is_empty() {
        melt
    } else {
        melt.with_user_message(msg)
    }
}

async fn parse_or_envelope_error<T: DeserializeOwned>(resp: gloo_net::http::Response) -> Result<T, MeltDown> {
    if (200..300).contains(&resp.status()) {
        match resp.json::<T>().await {
            Ok(v) => Ok(v),
            Err(e) => Err(MeltDown::new(MeltType::DeserializationFailed, format!("response decode: {}", e))),
        }
    } else {
        match resp.json::<ErrorEnvelope>().await {
            Ok(envelope) => Err(map_response_error(envelope)),
            Err(e) => Err(MeltDown::new(MeltType::Unexpected("network".to_string()), format!("non-2xx response and envelope decode failed: {}", e))),
        }
    }
}

pub async fn get_json<T: DeserializeOwned>(path: &str) -> Result<T, MeltDown> {
    let resp = match Request::get(path).send().await {
        Ok(r) => r,
        Err(e) => return Err(MeltDown::new(MeltType::Unexpected("network".to_string()), format!("GET {}: {}", path, e))),
    };
    parse_or_envelope_error(resp).await
}

pub async fn post_json<B: Serialize + ?Sized, T: DeserializeOwned>(path: &str, body: &B) -> Result<T, MeltDown> {
    let req = match Request::post(path).json(body) {
        Ok(r) => r,
        Err(e) => return Err(MeltDown::new(MeltType::SerializationFailed, format!("POST {}: encode: {}", path, e))),
    };
    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) => return Err(MeltDown::new(MeltType::Unexpected("network".to_string()), format!("POST {}: {}", path, e))),
    };
    parse_or_envelope_error(resp).await
}

pub async fn patch_json<B: Serialize + ?Sized, T: DeserializeOwned>(path: &str, body: &B) -> Result<T, MeltDown> {
    let req = match Request::patch(path).json(body) {
        Ok(r) => r,
        Err(e) => return Err(MeltDown::new(MeltType::SerializationFailed, format!("PATCH {}: encode: {}", path, e))),
    };
    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) => return Err(MeltDown::new(MeltType::Unexpected("network".to_string()), format!("PATCH {}: {}", path, e))),
    };
    parse_or_envelope_error(resp).await
}

pub async fn delete(path: &str) -> Result<(), MeltDown> {
    let resp = match Request::delete(path).send().await {
        Ok(r) => r,
        Err(e) => return Err(MeltDown::new(MeltType::Unexpected("network".to_string()), format!("DELETE {}: {}", path, e))),
    };
    if (200..300).contains(&resp.status()) {
        Ok(())
    } else {
        match resp.json::<ErrorEnvelope>().await {
            Ok(envelope) => Err(map_response_error(envelope)),
            Err(decode_err) => Err(MeltDown::new(MeltType::Unexpected("network".to_string()), format!("DELETE {}: non-2xx; envelope decode: {}", path, decode_err))),
        }
    }
}

pub async fn post_unit(path: &str) -> Result<(), MeltDown> {
    let resp = match Request::post(path).send().await {
        Ok(r) => r,
        Err(e) => return Err(MeltDown::new(MeltType::Unexpected("network".to_string()), format!("POST {}: {}", path, e))),
    };
    if (200..300).contains(&resp.status()) {
        Ok(())
    } else {
        match resp.json::<ErrorEnvelope>().await {
            Ok(envelope) => Err(map_response_error(envelope)),
            Err(decode_err) => Err(MeltDown::new(MeltType::Unexpected("network".to_string()), format!("POST {}: non-2xx; envelope decode: {}", path, decode_err))),
        }
    }
}
