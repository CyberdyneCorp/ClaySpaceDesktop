//! JSON-RPC 2.0 framing, in the shape MCP uses it.
//!
//! One message per request body. Batching was removed from MCP at the
//! 2025-06-18 revision, so an array arrives here as an invalid request rather
//! than as something to half-support.

use serde_json::{json, Value};

/// The error codes JSON-RPC defines, which are the only ones this speaks.
pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
pub const INTERNAL_ERROR: i64 = -32603;

/// One message from a client.
#[derive(Debug, Clone, PartialEq)]
pub enum Incoming {
    /// Carries an id and expects an answer.
    Request {
        id: Value,
        method: String,
        params: Value,
    },
    /// Carries no id and expects none. `notifications/initialized` is the one
    /// that matters here.
    Notification { method: String, params: Value },
}

impl Incoming {
    pub fn method(&self) -> &str {
        match self {
            Self::Request { method, .. } | Self::Notification { method, .. } => method,
        }
    }

    pub fn params(&self) -> &Value {
        match self {
            Self::Request { params, .. } | Self::Notification { params, .. } => params,
        }
    }

    pub fn id(&self) -> Option<&Value> {
        match self {
            Self::Request { id, .. } => Some(id),
            Self::Notification { .. } => None,
        }
    }
}

/// A message that could not be understood, already shaped as its answer.
#[derive(Debug, Clone, PartialEq)]
pub struct Malformed(pub Value);

/// Reads one message.
pub fn parse(body: &[u8]) -> Result<Incoming, Malformed> {
    let value: Value = match serde_json::from_slice(body) {
        Ok(value) => value,
        Err(e) => return Err(Malformed(error(Value::Null, PARSE_ERROR, e.to_string()))),
    };

    if value.is_array() {
        return Err(Malformed(error(
            Value::Null,
            INVALID_REQUEST,
            "this server takes one message per request; JSON-RPC batching was \
             removed from MCP at revision 2025-06-18",
        )));
    }

    let object = match value.as_object() {
        Some(object) => object,
        None => {
            return Err(Malformed(error(
                Value::Null,
                INVALID_REQUEST,
                "a JSON-RPC message is an object",
            )))
        }
    };

    let id = object.get("id").cloned();

    if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Err(Malformed(error(
            id.unwrap_or(Value::Null),
            INVALID_REQUEST,
            "a JSON-RPC message states \"jsonrpc\": \"2.0\"",
        )));
    }

    let method = match object.get("method").and_then(Value::as_str) {
        Some(method) => method.to_string(),
        None => {
            return Err(Malformed(error(
                id.unwrap_or(Value::Null),
                INVALID_REQUEST,
                "a JSON-RPC message names a method",
            )))
        }
    };

    let params = object.get("params").cloned().unwrap_or(Value::Null);

    match id {
        // A null id is a notification's absence written out, not an identity.
        Some(Value::Null) | None => Ok(Incoming::Notification { method, params }),
        Some(id) => Ok(Incoming::Request { id, method, params }),
    }
}

/// An answer carrying a result.
pub fn result(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

/// An answer carrying an error.
pub fn error(id: Value, code: i64, message: impl Into<String>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message.into() },
    })
}

/// An answer carrying an error and something an agent can branch on.
pub fn error_with_data(id: Value, code: i64, message: impl Into<String>, data: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message.into(), "data": data },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_carries_its_id() {
        let message = parse(br#"{"jsonrpc":"2.0","id":7,"method":"ping"}"#).unwrap();
        assert_eq!(message.id(), Some(&json!(7)));
        assert_eq!(message.method(), "ping");
        assert_eq!(message.params(), &Value::Null);
    }

    #[test]
    fn a_string_id_is_an_id_too() {
        let message = parse(br#"{"jsonrpc":"2.0","id":"a","method":"ping"}"#).unwrap();
        assert_eq!(message.id(), Some(&json!("a")));
    }

    #[test]
    fn a_message_with_no_id_is_a_notification() {
        let message = parse(br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).unwrap();
        assert!(matches!(message, Incoming::Notification { .. }));
        assert_eq!(message.id(), None);
    }

    #[test]
    fn a_null_id_is_an_absence_and_not_an_identity() {
        let message = parse(br#"{"jsonrpc":"2.0","id":null,"method":"ping"}"#).unwrap();
        assert!(matches!(message, Incoming::Notification { .. }));
    }

    #[test]
    fn a_batch_is_refused_and_says_why() {
        let Malformed(answer) =
            parse(br#"[{"jsonrpc":"2.0","id":1,"method":"ping"}]"#).unwrap_err();
        assert_eq!(answer["error"]["code"], INVALID_REQUEST);
        assert!(
            answer["error"]["message"]
                .as_str()
                .unwrap()
                .contains("2025-06-18"),
            "{answer}"
        );
    }

    #[test]
    fn broken_json_is_a_parse_error() {
        let Malformed(answer) = parse(b"{").unwrap_err();
        assert_eq!(answer["error"]["code"], PARSE_ERROR);
        assert_eq!(answer["id"], Value::Null);
    }

    #[test]
    fn a_missing_version_is_refused_against_the_id_it_came_with() {
        let Malformed(answer) = parse(br#"{"id":3,"method":"ping"}"#).unwrap_err();
        assert_eq!(answer["error"]["code"], INVALID_REQUEST);
        assert_eq!(answer["id"], json!(3));
    }

    #[test]
    fn a_message_with_no_method_is_refused() {
        let Malformed(answer) = parse(br#"{"jsonrpc":"2.0","id":3}"#).unwrap_err();
        assert_eq!(answer["error"]["code"], INVALID_REQUEST);
    }
}
