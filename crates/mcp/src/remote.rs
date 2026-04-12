use std::collections::BTreeMap;
use std::io;

use futures_util::{SinkExt, StreamExt};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, ACCEPT, CONTENT_TYPE};
use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio_tungstenite::tungstenite::Message as WsMessage;

use crate::client::{McpClientAuth, McpClientBootstrap, McpClientTransport, McpRemoteTransport};
use crate::stdio::types::{
    JsonRpcId, JsonRpcRequest, JsonRpcResponse, McpInitializeParams, McpInitializeResult,
    McpListToolsParams, McpListToolsResult, McpToolCallParams, McpToolCallResult,
    McpTransportError,
};

#[derive(Debug)]
enum RemoteTransport {
    Http {
        client: reqwest::Client,
        url: String,
        headers: HeaderMap,
    },
    WebSocket {
        ws: Box<
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
        >,
    },
}

#[derive(Debug)]
pub struct McpRemoteClient {
    transport: RemoteTransport,
}

impl McpRemoteClient {
    pub async fn connect(bootstrap: &McpClientBootstrap) -> Result<Self, McpTransportError> {
        match &bootstrap.transport {
            McpClientTransport::Sse(remote) | McpClientTransport::Http(remote) => {
                Self::connect_http(remote).await
            }
            McpClientTransport::WebSocket(remote) => Self::connect_ws(remote).await,
            other => Err(McpTransportError::Protocol {
                message: format!(
                    "MCP bootstrap transport for {} is not remote: {other:?}",
                    bootstrap.server_name
                ),
            }),
        }
    }

    async fn connect_http(remote: &McpRemoteTransport) -> Result<Self, McpTransportError> {
        let mut headers = build_headers(&remote.headers);
        if let McpClientAuth::OAuth(ref oauth) = remote.auth {
            if let Some(ref client_id) = oauth.client_id {
                let _ = headers.insert(
                    HeaderName::from_static("x-oauth-client-id"),
                    HeaderValue::from_str(client_id)
                        .unwrap_or_else(|_| HeaderValue::from_static("")),
                );
            }
        }

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(io::Error::other)?;

        Ok(Self {
            transport: RemoteTransport::Http {
                client,
                url: remote.url.clone(),
                headers,
            },
        })
    }

    async fn connect_ws(remote: &McpRemoteTransport) -> Result<Self, McpTransportError> {
        let mut request = tokio_tungstenite::tungstenite::http::Request::builder()
            .uri(&remote.url)
            .header("Sec-WebSocket-Protocol", "mcp");

        for (key, value) in &remote.headers {
            request = request.header(key.as_str(), value.as_str());
        }

        let request = request.body(()).map_err(|e| McpTransportError::Protocol {
            message: e.to_string(),
        })?;

        let (ws, _response) = tokio_tungstenite::connect_async(request)
            .await
            .map_err(|e| {
                McpTransportError::Connection(io::Error::new(
                    io::ErrorKind::ConnectionRefused,
                    e.to_string(),
                ))
            })?;

        Ok(Self {
            transport: RemoteTransport::WebSocket { ws: Box::new(ws) },
        })
    }

    pub async fn request<TParams: Serialize, TResult: DeserializeOwned>(
        &mut self,
        id: JsonRpcId,
        method: impl Into<String>,
        params: Option<TParams>,
    ) -> Result<JsonRpcResponse<TResult>, McpTransportError> {
        let method = method.into();
        let request = JsonRpcRequest::new(id.clone(), method.clone(), params);

        match &mut self.transport {
            RemoteTransport::Http {
                client,
                url,
                headers,
            } => {
                let body = serde_json::to_vec(&request)?;

                let response = client
                    .post(url.as_str())
                    .headers(headers.clone())
                    .header(CONTENT_TYPE, "application/json")
                    .header(ACCEPT, "application/json, text/event-stream")
                    .body(body)
                    .send()
                    .await
                    .map_err(|e| {
                        McpTransportError::Connection(io::Error::new(
                            io::ErrorKind::ConnectionRefused,
                            e.to_string(),
                        ))
                    })?;

                if !response.status().is_success() {
                    return Err(McpTransportError::Http {
                        status: response.status().as_u16(),
                    });
                }

                let content_type = response
                    .headers()
                    .get(CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("")
                    .to_string();

                if content_type.contains("text/event-stream") {
                    let text = response.text().await.map_err(|e| {
                        McpTransportError::Io(io::Error::new(io::ErrorKind::InvalidData, e))
                    })?;
                    parse_sse_jsonrpc(&text, &id)
                } else {
                    let bytes = response.bytes().await.map_err(|e| {
                        McpTransportError::Io(io::Error::new(io::ErrorKind::InvalidData, e))
                    })?;
                    Ok(serde_json::from_slice(&bytes)?)
                }
            }
            RemoteTransport::WebSocket { ws } => {
                let body = serde_json::to_string(&request)?;
                ws.send(WsMessage::Text(body.into()))
                    .await
                    .map_err(|e| McpTransportError::WebSocket(e.to_string()))?;

                let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(120);
                loop {
                    let msg = tokio::time::timeout_at(deadline, ws.next())
                        .await
                        .map_err(|_| McpTransportError::Timeout {
                            timeout_ms: 120_000,
                        })?
                        .ok_or_else(|| {
                            McpTransportError::Io(io::Error::new(
                                io::ErrorKind::UnexpectedEof,
                                "WebSocket stream closed while waiting for response",
                            ))
                        })?
                        .map_err(|e| McpTransportError::WebSocket(e.to_string()))?;

                    match msg {
                        WsMessage::Text(text) => {
                            let response: JsonRpcResponse<TResult> = serde_json::from_str(&text)?;
                            if response.id == id {
                                return Ok(response);
                            }
                        }
                        WsMessage::Ping(data) => {
                            let _ = ws.send(WsMessage::Pong(data)).await;
                        }
                        WsMessage::Close(_) => {
                            return Err(McpTransportError::WebSocket(
                                "WebSocket connection closed by server".into(),
                            ));
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    pub async fn initialize(
        &mut self,
        id: JsonRpcId,
        params: McpInitializeParams,
    ) -> Result<JsonRpcResponse<McpInitializeResult>, McpTransportError> {
        self.request(id, "initialize", Some(params)).await
    }

    pub async fn list_tools(
        &mut self,
        id: JsonRpcId,
        params: Option<McpListToolsParams>,
    ) -> Result<JsonRpcResponse<McpListToolsResult>, McpTransportError> {
        self.request(id, "tools/list", params).await
    }

    pub async fn call_tool(
        &mut self,
        id: JsonRpcId,
        params: McpToolCallParams,
    ) -> Result<JsonRpcResponse<McpToolCallResult>, McpTransportError> {
        self.request(id, "tools/call", Some(params)).await
    }

    pub async fn shutdown(&mut self) -> Result<(), McpTransportError> {
        match &mut self.transport {
            RemoteTransport::Http { .. } => Ok(()),
            RemoteTransport::WebSocket { ws } => {
                let _ = SinkExt::<WsMessage>::close(ws.as_mut()).await;
                Ok(())
            }
        }
    }
}

fn build_headers(headers: &BTreeMap<String, String>) -> HeaderMap {
    let mut header_map = HeaderMap::new();
    for (key, value) in headers {
        if let (Ok(name), Ok(val)) = (
            HeaderName::from_bytes(key.as_bytes()),
            HeaderValue::from_str(value),
        ) {
            header_map.insert(name, val);
        }
    }
    header_map
}

fn parse_sse_jsonrpc<T: DeserializeOwned>(
    sse_text: &str,
    expected_id: &JsonRpcId,
) -> Result<JsonRpcResponse<T>, McpTransportError> {
    for line in sse_text.lines() {
        let data = match line.strip_prefix("data: ") {
            Some(d) if !d.is_empty() => d.trim(),
            _ => continue,
        };
        if let Ok(response) = serde_json::from_str::<JsonRpcResponse<T>>(data) {
            if &response.id == expected_id {
                return Ok(response);
            }
        }
    }
    Err(McpTransportError::Protocol {
        message: "no matching JSON-RPC response found in SSE stream".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value as JsonValue;

    #[test]
    fn build_headers_creates_header_map() {
        let mut input = BTreeMap::new();
        input.insert("X-Custom".to_string(), "value1".to_string());
        input.insert("Authorization".to_string(), "Bearer tok".to_string());

        let headers = build_headers(&input);
        assert_eq!(headers.get("x-custom").unwrap(), "value1");
        assert_eq!(headers.get("authorization").unwrap(), "Bearer tok");
    }

    #[test]
    fn parse_sse_extracts_matching_response() {
        let sse_text =
            "event: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"tools\":[]}}\n\n";
        let result: JsonRpcResponse<JsonValue> =
            parse_sse_jsonrpc(sse_text, &JsonRpcId::Number(1)).unwrap();
        assert_eq!(result.id, JsonRpcId::Number(1));
        assert!(result.result.is_some());
    }

    #[test]
    fn parse_sse_rejects_when_no_match() {
        let sse_text = "data: {\"jsonrpc\":\"2.0\",\"id\":99,\"result\":null}\n\n";
        let result: Result<JsonRpcResponse<JsonValue>, McpTransportError> =
            parse_sse_jsonrpc(sse_text, &JsonRpcId::Number(1));
        assert!(result.is_err());
    }

    #[test]
    fn parse_sse_skips_non_data_lines() {
        let sse_text = ": comment\nevent: ping\nretry: 3000\ndata: {\"jsonrpc\":\"2.0\",\"id\":5,\"result\":42}\n\n";
        let result: JsonRpcResponse<JsonValue> =
            parse_sse_jsonrpc(sse_text, &JsonRpcId::Number(5)).unwrap();
        assert_eq!(result.id, JsonRpcId::Number(5));
    }
}
