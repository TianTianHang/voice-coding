use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum JsonRpcId {
    Number(u64),
    String(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: JsonRpcId,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: JsonRpcId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IncomingMessage {
    Response(JsonRpcResponse),
    Notification(JsonRpcNotification),
}

#[derive(Default)]
pub struct RequestIds {
    next_id: AtomicU64,
}

impl RequestIds {
    pub fn next(&self) -> JsonRpcId {
        JsonRpcId::Number(self.next_id.fetch_add(1, Ordering::Relaxed) + 1)
    }
}

pub fn request(id: JsonRpcId, method: impl Into<String>, params: Option<Value>) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id,
        method: method.into(),
        params,
    }
}

pub fn notification(method: impl Into<String>, params: Option<Value>) -> JsonRpcNotification {
    JsonRpcNotification {
        jsonrpc: "2.0".into(),
        method: method.into(),
        params,
    }
}

pub fn parse_incoming(line: &str) -> Result<IncomingMessage, String> {
    let value: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
    if value.get("id").is_some()
        && (value.get("result").is_some() || value.get("error").is_some())
    {
        let response = serde_json::from_value(value).map_err(|e| e.to_string())?;
        return Ok(IncomingMessage::Response(response));
    }

    if value.get("method").is_some() {
        let notification = serde_json::from_value(value).map_err(|e| e.to_string())?;
        return Ok(IncomingMessage::Notification(notification));
    }

    Err("Unrecognized JSON-RPC message".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_notification() {
        let parsed = parse_incoming(r#"{"jsonrpc":"2.0","method":"x","params":{"a":1}}"#).unwrap();

        assert!(matches!(parsed, IncomingMessage::Notification(_)));
    }

    #[test]
    fn parses_response() {
        let parsed = parse_incoming(r#"{"jsonrpc":"2.0","id":1,"result":{"ok":true}}"#).unwrap();

        assert!(matches!(parsed, IncomingMessage::Response(_)));
    }
}
