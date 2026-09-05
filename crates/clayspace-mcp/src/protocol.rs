//! The Model Context Protocol methods, over the JSON-RPC framing.
//!
//! Deliberately small. `initialize`, `ping`, `tools/list`, `tools/call` and
//! the initialized notification are the whole of what an agent driving an
//! application needs; resources and prompts are a natural second step and
//! nothing here forecloses them.
//!
//! The revision this build implements is a constant reported in `initialize`,
//! so a client that wants a different one gets a negotiation it can read
//! rather than a misparse it cannot.

use serde_json::{json, Map, Value};

use crate::base64;
use crate::jsonrpc::{self, Incoming};
use crate::session::{Refusal, RefusalCode};

/// The MCP revision this build implements.
///
/// 2025-06-18 is the revision that carries Streamable HTTP and that removed
/// JSON-RPC batching, which is why [`crate::jsonrpc`] refuses an array.
pub const PROTOCOL_VERSION: &str = "2025-06-18";

pub const SERVER_NAME: &str = "clayspace";
pub const SERVER_TITLE: &str = "ClaySpaceDesktop";
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// One tool, as `tools/list` describes it.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolDescriptor {
    pub name: String,
    pub title: String,
    pub description: String,
    pub input_schema: Value,
}

impl ToolDescriptor {
    fn to_json(&self) -> Value {
        json!({
            "name": self.name,
            "title": self.title,
            "description": self.description,
            "inputSchema": self.input_schema,
        })
    }
}

/// A block of a tool's answer.
#[derive(Debug, Clone, PartialEq)]
pub enum Content {
    Text(String),
    /// A PNG, which a client shows rather than reads.
    Image(Vec<u8>),
}

impl Content {
    fn to_json(&self) -> Value {
        match self {
            Self::Text(text) => json!({ "type": "text", "text": text }),
            Self::Image(png) => json!({
                "type": "image",
                "data": base64::encode(png),
                "mimeType": "image/png",
            }),
        }
    }
}

/// What a tool produced.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CallResult {
    pub content: Vec<Content>,
    /// The same answer as data, for a client that would rather branch than
    /// read.
    pub structured: Option<Value>,
}

impl CallResult {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![Content::Text(text.into())],
            structured: None,
        }
    }

    /// An answer that is data, sent both ways: as `structuredContent` for a
    /// client that reads it, and as text for one that does not.
    pub fn data(value: Value) -> Self {
        Self {
            content: vec![Content::Text(
                serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string()),
            )],
            structured: Some(value),
        }
    }

    pub fn with_image(mut self, png: Vec<u8>) -> Self {
        self.content.push(Content::Image(png));
        self
    }

    fn to_json(&self) -> Value {
        let mut object = Map::new();
        object.insert(
            "content".into(),
            Value::Array(self.content.iter().map(Content::to_json).collect()),
        );
        if let Some(structured) = &self.structured {
            object.insert("structuredContent".into(), structured.clone());
        }
        object.insert("isError".into(), Value::Bool(false));
        Value::Object(object)
    }
}

/// What the protocol can be asked for. Implemented by the tool catalogue.
pub trait ToolSurface: Send + Sync {
    fn tools(&self) -> Vec<ToolDescriptor>;
    fn call(&self, name: &str, arguments: &Value) -> Result<CallResult, Refusal>;
    /// What an agent should know before it starts, sent with `initialize`.
    fn instructions(&self) -> String;
}

/// A refusal, as a tool result rather than as a transport error.
///
/// MCP draws this line and it is the right one: a tool that ran and refused is
/// something the *model* must see and reason about, while a transport error is
/// something the client handles. A gate the person has not lifted is the
/// former.
fn refusal_to_result(refusal: &Refusal) -> Value {
    let mut structured = json!({
        "refused": true,
        "code": refusal.code,
        "message": refusal.message,
    });
    if let Some(gate) = refusal.gate {
        structured["gate"] = json!(gate);
    }
    json!({
        "content": [{ "type": "text", "text": refusal.message }],
        "structuredContent": structured,
        "isError": true,
    })
}

/// Handles one message against a tool surface.
pub struct Protocol<'a> {
    pub surface: &'a dyn ToolSurface,
}

impl<'a> Protocol<'a> {
    pub fn new(surface: &'a dyn ToolSurface) -> Self {
        Self { surface }
    }

    /// The answer to one message, or none where the message was a
    /// notification and wants none.
    pub fn handle(&self, incoming: &Incoming) -> Option<Value> {
        let id = incoming.id().cloned();
        let method = incoming.method();
        let params = incoming.params();

        match (method, id) {
            ("notifications/initialized", _) | ("notifications/cancelled", None) => None,
            ("initialize", Some(id)) => Some(jsonrpc::result(id, self.initialize(params))),
            ("ping", Some(id)) => Some(jsonrpc::result(id, json!({}))),
            ("tools/list", Some(id)) => Some(jsonrpc::result(
                id,
                json!({ "tools": self.surface.tools().iter().map(ToolDescriptor::to_json).collect::<Vec<_>>() }),
            )),
            ("tools/call", Some(id)) => Some(self.call(id, params)),
            (method, Some(id)) => Some(jsonrpc::error(
                id,
                jsonrpc::METHOD_NOT_FOUND,
                format!("this server does not implement {method}"),
            )),
            // A notification naming something we do not implement is not an
            // error anyone can act on, and a notification takes no answer.
            (_, None) => None,
        }
    }

    fn initialize(&self, params: &Value) -> Value {
        // The client's requested version is read and answered with ours. Where
        // they differ the client decides whether to proceed, which is what the
        // negotiation is for.
        let requested = params
            .get("protocolVersion")
            .and_then(Value::as_str)
            .unwrap_or(PROTOCOL_VERSION);
        let _ = requested;
        json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": {
                "name": SERVER_NAME,
                "title": SERVER_TITLE,
                "version": SERVER_VERSION,
            },
            "instructions": self.surface.instructions(),
        })
    }

    fn call(&self, id: Value, params: &Value) -> Value {
        let name = match params.get("name").and_then(Value::as_str) {
            Some(name) => name,
            None => {
                return jsonrpc::error(
                    id,
                    jsonrpc::INVALID_PARAMS,
                    "tools/call names the tool in \"name\"",
                )
            }
        };
        let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

        match self.surface.call(name, &arguments) {
            Ok(result) => jsonrpc::result(id, result.to_json()),
            // An unknown *tool* is the client's mistake and belongs in the
            // transport; an unknown action within a tool is the model's, and
            // belongs where the model will read it.
            Err(refusal)
                if refusal.code == RefusalCode::UnknownAction
                    && !name_is_known(self.surface, name) =>
            {
                jsonrpc::error_with_data(
                    id,
                    jsonrpc::INVALID_PARAMS,
                    refusal.message.clone(),
                    json!({ "code": refusal.code }),
                )
            }
            Err(refusal) => jsonrpc::result(id, refusal_to_result(&refusal)),
        }
    }
}

fn name_is_known(surface: &dyn ToolSurface, name: &str) -> bool {
    surface.tools().iter().any(|tool| tool.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jsonrpc::parse;

    struct OneTool;

    impl ToolSurface for OneTool {
        fn tools(&self) -> Vec<ToolDescriptor> {
            vec![ToolDescriptor {
                name: "history".into(),
                title: "Histórico".into(),
                description: "Undo and redo.".into(),
                input_schema: json!({ "type": "object", "properties": {} }),
            }]
        }

        fn call(&self, name: &str, arguments: &Value) -> Result<CallResult, Refusal> {
            match name {
                "history" => match arguments.get("action").and_then(Value::as_str) {
                    Some("undo") => Ok(CallResult::data(json!({ "depth": 0 }))),
                    Some(other) => Err(Refusal::new(
                        RefusalCode::UnknownAction,
                        format!("history has no action {other}"),
                    )),
                    None => Err(Refusal::new(RefusalCode::BadArgument, "action is required")),
                },
                other => Err(Refusal::new(
                    RefusalCode::UnknownAction,
                    format!("there is no tool named {other}"),
                )),
            }
        }

        fn instructions(&self) -> String {
            "Drive the sculpting application.".into()
        }
    }

    fn answer(body: &str) -> Value {
        let incoming = parse(body.as_bytes()).unwrap();
        Protocol::new(&OneTool).handle(&incoming).unwrap()
    }

    #[test]
    fn initialize_reports_the_revision_this_build_implements() {
        let answer = answer(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}"#,
        );
        assert_eq!(answer["result"]["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(answer["result"]["serverInfo"]["name"], SERVER_NAME);
        assert!(answer["result"]["instructions"].is_string());
        assert_eq!(
            answer["result"]["capabilities"]["tools"]["listChanged"],
            false
        );
    }

    #[test]
    fn the_initialized_notification_takes_no_answer() {
        let incoming = parse(br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).unwrap();
        assert!(Protocol::new(&OneTool).handle(&incoming).is_none());
    }

    #[test]
    fn ping_answers_nothing_in_particular() {
        let answer = answer(r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#);
        assert_eq!(answer["result"], json!({}));
    }

    #[test]
    fn tools_are_listed_with_their_schemas() {
        let answer = answer(r#"{"jsonrpc":"2.0","id":3,"method":"tools/list"}"#);
        let tools = answer["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"], "history");
        assert_eq!(tools[0]["inputSchema"]["type"], "object");
    }

    #[test]
    fn a_call_answers_as_data_and_as_text() {
        let answer = answer(
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"history","arguments":{"action":"undo"}}}"#,
        );
        assert_eq!(answer["result"]["structuredContent"]["depth"], 0);
        assert_eq!(answer["result"]["content"][0]["type"], "text");
        assert_eq!(answer["result"]["isError"], false);
    }

    #[test]
    fn a_refusal_reaches_the_model_rather_than_the_client() {
        // A tool that ran and refused is something the model must reason
        // about, so it is a result with isError, not a transport error.
        let answer = answer(
            r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"history","arguments":{"action":"fly"}}}"#,
        );
        assert_eq!(answer["result"]["isError"], true);
        assert_eq!(
            answer["result"]["structuredContent"]["code"],
            "unknown_action"
        );
        assert!(answer["error"].is_null());
    }

    #[test]
    fn an_unknown_tool_is_the_clients_mistake_and_reaches_the_client() {
        let answer = answer(
            r#"{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"fly","arguments":{}}}"#,
        );
        assert_eq!(answer["error"]["code"], jsonrpc::INVALID_PARAMS);
    }

    #[test]
    fn a_call_with_no_tool_named_says_so() {
        let answer = answer(r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{}}"#);
        assert_eq!(answer["error"]["code"], jsonrpc::INVALID_PARAMS);
    }

    #[test]
    fn a_method_this_server_does_not_implement_is_named() {
        let answer = answer(r#"{"jsonrpc":"2.0","id":8,"method":"resources/list"}"#);
        assert_eq!(answer["error"]["code"], jsonrpc::METHOD_NOT_FOUND);
        assert!(answer["error"]["message"]
            .as_str()
            .unwrap()
            .contains("resources/list"));
    }

    #[test]
    fn an_image_is_a_content_block_a_client_can_show() {
        let result = CallResult::text("feito").with_image(b"\x89PNG".to_vec());
        let json = result.to_json();
        assert_eq!(json["content"][1]["type"], "image");
        assert_eq!(json["content"][1]["mimeType"], "image/png");
        assert_eq!(json["content"][1]["data"], base64::encode(b"\x89PNG"));
    }
}
