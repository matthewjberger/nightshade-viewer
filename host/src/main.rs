use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use futures_util::{SinkExt, StreamExt};
use protocol::{
    AgentCommand, AgentRequest, AgentResponse, ClientMessage, CorrelationId, DeltaBatch, EntityRef,
    Environment, SubscriptionFilter, SubscriptionId, Version,
};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message;

const WS_ADDR: &str = "127.0.0.1:8787";
const REQUEST_TIMEOUT_SECS: u64 = 30;
const RING_CAPACITY: usize = 4096;

struct Subscription {
    filter: SubscriptionFilter,
    cursor: Version,
}

struct Shared {
    next_correlation: AtomicU64,
    pending: Mutex<HashMap<CorrelationId, oneshot::Sender<AgentResponse>>>,
    page_tx: Mutex<Option<mpsc::UnboundedSender<String>>>,
    ring: Mutex<Vec<DeltaBatch>>,
    subscriptions: Mutex<HashMap<SubscriptionId, Subscription>>,
}

impl Shared {
    fn new() -> Self {
        Self {
            next_correlation: AtomicU64::new(1),
            pending: Mutex::new(HashMap::new()),
            page_tx: Mutex::new(None),
            ring: Mutex::new(Vec::new()),
            subscriptions: Mutex::new(HashMap::new()),
        }
    }

    fn correlation(&self) -> CorrelationId {
        self.next_correlation.fetch_add(1, Ordering::Relaxed)
    }
}

#[tokio::main]
async fn main() {
    let shared = Arc::new(Shared::new());

    let ws_shared = shared.clone();
    tokio::spawn(async move {
        run_ws_server(ws_shared).await;
    });

    run_stdio(shared).await;
}

async fn run_ws_server(shared: Arc<Shared>) {
    let listener = match tokio::net::TcpListener::bind(WS_ADDR).await {
        Ok(listener) => listener,
        Err(error) => {
            log(&format!("failed to bind {WS_ADDR}: {error}"));
            return;
        }
    };
    log(&format!("websocket relay listening on ws://{WS_ADDR}"));
    loop {
        let Ok((stream, _addr)) = listener.accept().await else {
            continue;
        };
        let conn_shared = shared.clone();
        tokio::spawn(async move {
            handle_page(conn_shared, stream).await;
        });
    }
}

async fn handle_page(shared: Arc<Shared>, stream: tokio::net::TcpStream) {
    let websocket = match tokio_tungstenite::accept_async(stream).await {
        Ok(websocket) => websocket,
        Err(error) => {
            log(&format!("websocket handshake failed: {error}"));
            return;
        }
    };
    log("viewer page connected");
    let (mut sink, mut source) = websocket.split();

    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<String>();
    *shared.page_tx.lock().await = Some(out_tx);

    let writer = tokio::spawn(async move {
        while let Some(text) = out_rx.recv().await {
            if sink.send(Message::Text(text)).await.is_err() {
                break;
            }
        }
    });

    while let Some(message) = source.next().await {
        let Ok(message) = message else {
            break;
        };
        let text = match message {
            Message::Text(text) => text,
            Message::Close(_) => break,
            _ => continue,
        };
        let Ok(response) = serde_json::from_str::<AgentResponse>(&text) else {
            log(&format!("unparseable response from page: {text}"));
            continue;
        };
        route_response(&shared, response).await;
    }

    *shared.page_tx.lock().await = None;
    writer.abort();
    log("viewer page disconnected");
}

async fn route_response(shared: &Arc<Shared>, response: AgentResponse) {
    if let AgentResponse::Batch { batch } = response {
        let mut ring = shared.ring.lock().await;
        ring.push(batch);
        let overflow = ring.len().saturating_sub(RING_CAPACITY);
        if overflow > 0 {
            ring.drain(0..overflow);
        }
        return;
    }
    // Progress is informational, not the terminal reply; keep waiting.
    if let AgentResponse::CommandProgress { .. } = response {
        return;
    }
    if let Some(correlation_id) = response_correlation(&response) {
        let sender = shared.pending.lock().await.remove(&correlation_id);
        if let Some(sender) = sender {
            let _ = sender.send(response);
        }
    }
}

fn response_correlation(response: &AgentResponse) -> Option<CorrelationId> {
    match response {
        AgentResponse::ComponentTypes { correlation_id, .. }
        | AgentResponse::QueryResult { correlation_id, .. }
        | AgentResponse::GetResult { correlation_id, .. }
        | AgentResponse::CommandApplied { correlation_id, .. }
        | AgentResponse::Loaded { correlation_id, .. }
        | AgentResponse::CommandFailed { correlation_id, .. }
        | AgentResponse::CommandProgress { correlation_id, .. }
        | AgentResponse::Subscribed { correlation_id, .. }
        | AgentResponse::Unsubscribed { correlation_id, .. }
        | AgentResponse::ViewerState { correlation_id, .. } => Some(*correlation_id),
        AgentResponse::Batch { .. }
        | AgentResponse::Replay { .. }
        | AgentResponse::Resnapshot { .. } => None,
    }
}

async fn send_request(
    shared: &Arc<Shared>,
    request: AgentRequest,
) -> Result<AgentResponse, String> {
    let correlation_id = request_correlation(&request);
    let (tx, rx) = oneshot::channel();
    shared.pending.lock().await.insert(correlation_id, tx);

    let text = serde_json::to_string(&request).map_err(|error| error.to_string())?;
    {
        let guard = shared.page_tx.lock().await;
        let Some(sender) = guard.as_ref() else {
            shared.pending.lock().await.remove(&correlation_id);
            return Err("viewer page is not connected".to_string());
        };
        if sender.send(text).is_err() {
            shared.pending.lock().await.remove(&correlation_id);
            return Err("viewer page relay is closed".to_string());
        }
    }

    let timeout = tokio::time::Duration::from_secs(REQUEST_TIMEOUT_SECS);
    match tokio::time::timeout(timeout, rx).await {
        Ok(Ok(response)) => Ok(response),
        Ok(Err(_)) => Err("response channel dropped".to_string()),
        Err(_) => {
            shared.pending.lock().await.remove(&correlation_id);
            Err("timed out waiting for the viewer".to_string())
        }
    }
}

fn request_correlation(request: &AgentRequest) -> CorrelationId {
    match request {
        AgentRequest::ListComponentTypes { correlation_id }
        | AgentRequest::Query { correlation_id, .. }
        | AgentRequest::GetComponents { correlation_id, .. }
        | AgentRequest::Command { correlation_id, .. }
        | AgentRequest::Subscribe { correlation_id, .. }
        | AgentRequest::Unsubscribe { correlation_id, .. }
        | AgentRequest::ViewerAction { correlation_id, .. }
        | AgentRequest::GetViewerState { correlation_id }
        | AgentRequest::SetEnvironment { correlation_id, .. } => *correlation_id,
        AgentRequest::Resync { .. } => 0,
    }
}

async fn run_stdio(shared: Arc<Shared>) {
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin).lines();
    let mut stdout = tokio::io::stdout();

    while let Ok(Some(line)) = reader.next_line().await {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let id = message.get("id").cloned();
        let method = message
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let params = message.get("params").cloned().unwrap_or(Value::Null);

        let response = dispatch(&shared, &method, params, id.clone()).await;
        if let Some(response) = response {
            let mut text = response.to_string();
            text.push('\n');
            if stdout.write_all(text.as_bytes()).await.is_err() {
                break;
            }
            let _ = stdout.flush().await;
        }
    }
}

async fn dispatch(
    shared: &Arc<Shared>,
    method: &str,
    params: Value,
    id: Option<Value>,
) -> Option<Value> {
    match method {
        "initialize" => Some(rpc_result(
            id,
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "nightshade-viewer", "version": "0.1.0" }
            }),
        )),
        "notifications/initialized" => None,
        "ping" => Some(rpc_result(id, json!({}))),
        "tools/list" => Some(rpc_result(id, json!({ "tools": tool_definitions() }))),
        "tools/call" => Some(handle_tool_call(shared, params, id).await),
        _ => Some(rpc_error(
            id,
            -32601,
            &format!("method not found: {method}"),
        )),
    }
}

async fn handle_tool_call(shared: &Arc<Shared>, params: Value, id: Option<Value>) -> Value {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

    match run_tool(shared, &name, arguments).await {
        Ok(text) => rpc_result(
            id,
            json!({ "content": [{ "type": "text", "text": text }], "isError": false }),
        ),
        Err(error) => rpc_result(
            id,
            json!({ "content": [{ "type": "text", "text": error }], "isError": true }),
        ),
    }
}

async fn run_tool(shared: &Arc<Shared>, name: &str, arguments: Value) -> Result<String, String> {
    match name {
        "list_component_types" => {
            let correlation_id = shared.correlation();
            let response =
                send_request(shared, AgentRequest::ListComponentTypes { correlation_id }).await?;
            Ok(pretty(&response_payload(response)))
        }
        "query" => {
            let component_types = string_array(&arguments, "component_types")?;
            let correlation_id = shared.correlation();
            let response = send_request(
                shared,
                AgentRequest::Query {
                    correlation_id,
                    component_types,
                },
            )
            .await?;
            Ok(pretty(&response_payload(response)))
        }
        "get_components" => {
            let entity = parse_entity(&arguments, "entity")?;
            let component_types = string_array(&arguments, "component_types")?;
            let correlation_id = shared.correlation();
            let response = send_request(
                shared,
                AgentRequest::GetComponents {
                    correlation_id,
                    entity,
                    component_types,
                },
            )
            .await?;
            Ok(pretty(&response_payload(response)))
        }
        "spawn_entity" => {
            let components = parse_component_bag(&arguments, "components")?;
            command(shared, AgentCommand::SpawnEntity { components }).await
        }
        "set_components" => {
            let entity = parse_entity(&arguments, "entity")?;
            let components = parse_component_bag(&arguments, "components")?;
            command(shared, AgentCommand::SetComponents { entity, components }).await
        }
        "remove_components" => {
            let entity = parse_entity(&arguments, "entity")?;
            let component_types = string_array(&arguments, "component_types")?;
            command(
                shared,
                AgentCommand::RemoveComponents {
                    entity,
                    component_types,
                },
            )
            .await
        }
        "reparent" => {
            let child = parse_entity(&arguments, "child")?;
            let new_parent = match arguments.get("new_parent") {
                None | Some(Value::Null) => None,
                Some(_) => Some(parse_entity(&arguments, "new_parent")?),
            };
            command(shared, AgentCommand::Reparent { child, new_parent }).await
        }
        "delete_entity" => {
            let entity = parse_entity(&arguments, "entity")?;
            command(shared, AgentCommand::DeleteEntity { entity }).await
        }
        "select_node" => {
            let entity = parse_entity(&arguments, "entity")?;
            command(shared, AgentCommand::SelectNode { entity }).await
        }
        "load_gltf" => {
            let uri = arguments
                .get("uri")
                .and_then(Value::as_str)
                .ok_or("missing string field: uri")?
                .to_string();
            let correlation_id = shared.correlation();
            let response = send_request(
                shared,
                AgentRequest::Command {
                    correlation_id,
                    command: AgentCommand::LoadGltf { uri },
                },
            )
            .await?;
            match response {
                AgentResponse::Loaded { version, roots, .. } => {
                    Ok(json!({ "applied": true, "version": version, "roots": roots }).to_string())
                }
                AgentResponse::CommandFailed { error, .. } => Err(error),
                other => Ok(pretty(&response_payload(other))),
            }
        }
        "viewer_action" => {
            let action = arguments
                .get("action")
                .cloned()
                .ok_or("missing field: action")?;
            // Accept the action as a real object ({"SetGrid":{"enabled":false}})
            // or as a JSON string of one; a bare string ("Frame") stays a unit
            // variant name.
            let action = match action {
                Value::String(text) => {
                    serde_json::from_str::<Value>(&text).unwrap_or(Value::String(text))
                }
                other => other,
            };
            let message: ClientMessage = serde_json::from_value(action)
                .map_err(|error| format!("not a valid viewer action: {error}"))?;
            let correlation_id = shared.correlation();
            let response = send_request(
                shared,
                AgentRequest::ViewerAction {
                    correlation_id,
                    message: Box::new(message),
                },
            )
            .await?;
            match response {
                AgentResponse::CommandApplied { version, .. } => {
                    Ok(json!({ "applied": true, "version": version }).to_string())
                }
                AgentResponse::CommandFailed { error, .. } => Err(error),
                other => Ok(pretty(&response_payload(other))),
            }
        }
        "get_viewer_state" => {
            let correlation_id = shared.correlation();
            let response =
                send_request(shared, AgentRequest::GetViewerState { correlation_id }).await?;
            if let AgentResponse::ViewerState { state, .. } = response {
                Ok(pretty(&state))
            } else {
                Ok(pretty(&response_payload(response)))
            }
        }
        "set_environment" => {
            let environment: Environment = serde_json::from_value(arguments)
                .map_err(|error| format!("invalid environment: {error}"))?;
            let correlation_id = shared.correlation();
            let response = send_request(
                shared,
                AgentRequest::SetEnvironment {
                    correlation_id,
                    environment,
                },
            )
            .await?;
            match response {
                AgentResponse::CommandApplied { version, .. } => {
                    Ok(json!({ "applied": true, "version": version }).to_string())
                }
                AgentResponse::CommandFailed { error, .. } => Err(error),
                other => Ok(pretty(&response_payload(other))),
            }
        }
        "load_polyhaven_model" => {
            let slug = arguments
                .get("slug")
                .and_then(Value::as_str)
                .ok_or("missing string field: slug")?
                .to_string();
            let resolution = arguments
                .get("resolution")
                .and_then(Value::as_u64)
                .unwrap_or(2) as u32;
            let correlation_id = shared.correlation();
            let response = send_request(
                shared,
                AgentRequest::Command {
                    correlation_id,
                    command: AgentCommand::LoadPolyhavenModel { slug, resolution },
                },
            )
            .await?;
            match response {
                AgentResponse::Loaded { version, roots, .. } => {
                    Ok(json!({ "applied": true, "version": version, "roots": roots }).to_string())
                }
                AgentResponse::CommandFailed { error, .. } => Err(error),
                other => Ok(pretty(&response_payload(other))),
            }
        }
        "subscribe" => subscribe_tool(shared, arguments).await,
        "poll_deltas" => poll_deltas_tool(shared, arguments).await,
        "unsubscribe" => unsubscribe_tool(shared, arguments).await,
        other => Err(format!("unknown tool: {other}")),
    }
}

async fn command(shared: &Arc<Shared>, command: AgentCommand) -> Result<String, String> {
    let correlation_id = shared.correlation();
    let response = send_request(
        shared,
        AgentRequest::Command {
            correlation_id,
            command,
        },
    )
    .await?;
    match response {
        AgentResponse::CommandApplied { version, .. } => {
            Ok(json!({ "applied": true, "version": version }).to_string())
        }
        AgentResponse::CommandFailed { error, .. } => Err(error),
        other => Ok(pretty(&response_payload(other))),
    }
}

async fn subscribe_tool(shared: &Arc<Shared>, arguments: Value) -> Result<String, String> {
    let component_types = string_array(&arguments, "component_types")?;
    let entities = match arguments.get("entities") {
        None | Some(Value::Null) => None,
        Some(Value::Array(items)) => {
            let mut refs = Vec::with_capacity(items.len());
            for item in items {
                refs.push(entity_from_value(item)?);
            }
            Some(refs)
        }
        Some(_) => return Err("entities must be an array".to_string()),
    };
    let filter = SubscriptionFilter {
        component_types,
        entities,
    };
    let correlation_id = shared.correlation();
    let response = send_request(
        shared,
        AgentRequest::Subscribe {
            correlation_id,
            filter: filter.clone(),
        },
    )
    .await?;
    match response {
        AgentResponse::Subscribed {
            subscription_id,
            snapshot,
            ..
        } => {
            shared.subscriptions.lock().await.insert(
                subscription_id,
                Subscription {
                    filter,
                    cursor: snapshot.version,
                },
            );
            Ok(json!({
                "subscription_id": subscription_id,
                "version": snapshot.version,
                "snapshot": serde_json::to_value(&snapshot).unwrap_or(Value::Null),
            })
            .to_string())
        }
        AgentResponse::CommandFailed { error, .. } => Err(error),
        other => Ok(pretty(&response_payload(other))),
    }
}

async fn poll_deltas_tool(shared: &Arc<Shared>, arguments: Value) -> Result<String, String> {
    let subscription_id = arguments
        .get("subscription_id")
        .and_then(Value::as_u64)
        .ok_or("missing integer field: subscription_id")?;

    let mut subscriptions = shared.subscriptions.lock().await;
    let subscription = subscriptions
        .get_mut(&subscription_id)
        .ok_or("unknown subscription_id")?;

    let ring = shared.ring.lock().await;
    let oldest = ring.first().map(|batch| batch.base_version);
    if let Some(oldest) = oldest
        && subscription.cursor < oldest
    {
        return Ok(json!({
            "resync_required": true,
            "reason": "cursor aged out of the ring buffer; re-subscribe",
        })
        .to_string());
    }

    let mut delivered = Vec::new();
    for batch in ring.iter() {
        if batch.target_version <= subscription.cursor {
            continue;
        }
        let filtered = filter_batch(batch, &subscription.filter);
        subscription.cursor = batch.target_version;
        delivered.push(filtered);
    }

    Ok(json!({
        "resync_required": false,
        "version": subscription.cursor,
        "batches": serde_json::to_value(&delivered).unwrap_or(Value::Null),
    })
    .to_string())
}

async fn unsubscribe_tool(shared: &Arc<Shared>, arguments: Value) -> Result<String, String> {
    let subscription_id = arguments
        .get("subscription_id")
        .and_then(Value::as_u64)
        .ok_or("missing integer field: subscription_id")?;
    shared.subscriptions.lock().await.remove(&subscription_id);
    let correlation_id = shared.correlation();
    let response = send_request(
        shared,
        AgentRequest::Unsubscribe {
            correlation_id,
            subscription_id,
        },
    )
    .await?;
    Ok(pretty(&response_payload(response)))
}

fn filter_batch(batch: &DeltaBatch, filter: &SubscriptionFilter) -> DeltaBatch {
    let wants = |component: &str| {
        filter.component_types.is_empty()
            || filter.component_types.iter().any(|name| name == component)
    };
    let deltas = batch
        .deltas
        .iter()
        .filter(|delta| match delta {
            protocol::Delta::Changed { component, .. }
            | protocol::Delta::Added { component, .. }
            | protocol::Delta::Removed { component, .. } => wants(component),
            protocol::Delta::Spawned { .. } | protocol::Delta::Despawned { .. } => true,
        })
        .cloned()
        .collect();
    DeltaBatch {
        base_version: batch.base_version,
        target_version: batch.target_version,
        deltas,
        checksum: batch.checksum.clone(),
    }
}

fn response_payload(response: AgentResponse) -> Value {
    serde_json::to_value(&response).unwrap_or(Value::Null)
}

fn string_array(arguments: &Value, field: &str) -> Result<Vec<String>, String> {
    match arguments.get(field) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(items)) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(
                    item.as_str()
                        .ok_or_else(|| format!("{field} must be an array of strings"))?
                        .to_string(),
                );
            }
            Ok(out)
        }
        Some(_) => Err(format!("{field} must be an array of strings")),
    }
}

fn parse_component_bag(arguments: &Value, field: &str) -> Result<Vec<(String, Value)>, String> {
    match arguments.get(field) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Object(map)) => Ok(map
            .iter()
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect()),
        Some(_) => Err(format!("{field} must be an object of name to value")),
    }
}

fn parse_entity(arguments: &Value, field: &str) -> Result<EntityRef, String> {
    let value = arguments
        .get(field)
        .ok_or_else(|| format!("missing entity field: {field}"))?;
    entity_from_value(value)
}

fn entity_from_value(value: &Value) -> Result<EntityRef, String> {
    let id = value
        .get("id")
        .and_then(Value::as_u64)
        .ok_or("entity needs an integer id")? as u32;
    let generation = value
        .get("generation")
        .and_then(Value::as_u64)
        .ok_or("entity needs an integer generation")? as u32;
    Ok(EntityRef { id, generation })
}

fn entity_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "id": { "type": "integer" },
            "generation": { "type": "integer" }
        },
        "required": ["id", "generation"]
    })
}

fn tool_definitions() -> Vec<Value> {
    let string_list = json!({ "type": "array", "items": { "type": "string" } });
    let component_bag = json!({
        "type": "object",
        "description": "Map of component name to its JSON value. Discover shapes with list_component_types.",
        "additionalProperties": true
    });
    vec![
        json!({
            "name": "list_component_types",
            "description": "Discover every component: name, write policy (Free, Owned by a command, or Derived), JSON schema, and an example value.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "query",
            "description": "Return the entity handles whose archetype contains all of the named component types.",
            "inputSchema": {
                "type": "object",
                "properties": { "component_types": string_list },
                "required": ["component_types"]
            }
        }),
        json!({
            "name": "get_components",
            "description": "Return serialized component values for one entity. A stale handle returns a not-live result, never another entity's data.",
            "inputSchema": {
                "type": "object",
                "properties": { "entity": entity_schema(), "component_types": string_list },
                "required": ["entity", "component_types"]
            }
        }),
        json!({
            "name": "spawn_entity",
            "description": "Spawn an entity carrying the given component bag. Owned and Derived components are rejected.",
            "inputSchema": {
                "type": "object",
                "properties": { "components": component_bag },
                "required": ["components"]
            }
        }),
        json!({
            "name": "set_components",
            "description": "Write the given component bag onto an existing entity. Owned and Derived components are rejected with the command to use.",
            "inputSchema": {
                "type": "object",
                "properties": { "entity": entity_schema(), "components": component_bag },
                "required": ["entity", "components"]
            }
        }),
        json!({
            "name": "remove_components",
            "description": "Remove the named components from an entity.",
            "inputSchema": {
                "type": "object",
                "properties": { "entity": entity_schema(), "component_types": string_list },
                "required": ["entity", "component_types"]
            }
        }),
        json!({
            "name": "reparent",
            "description": "Reparent a child entity. Omit new_parent or pass null to detach to the scene root.",
            "inputSchema": {
                "type": "object",
                "properties": { "child": entity_schema(), "new_parent": entity_schema() },
                "required": ["child"]
            }
        }),
        json!({
            "name": "delete_entity",
            "description": "Despawn an entity and its descendants.",
            "inputSchema": {
                "type": "object",
                "properties": { "entity": entity_schema() },
                "required": ["entity"]
            }
        }),
        json!({
            "name": "select_node",
            "description": "Select an entity in the viewer (drives the inspector and gizmo).",
            "inputSchema": {
                "type": "object",
                "properties": { "entity": entity_schema() },
                "required": ["entity"]
            }
        }),
        json!({
            "name": "load_gltf",
            "description": "Load a glTF or GLB by URI. Acknowledges when the scene has finished spawning.",
            "inputSchema": {
                "type": "object",
                "properties": { "uri": { "type": "string" } },
                "required": ["uri"]
            }
        }),
        json!({
            "name": "load_polyhaven_model",
            "description": "Grab a Polyhaven model by slug (from get_viewer_state's models list) and load it additively. Returns the spawned root handle(s) to position with set_components. resolution is texture k (default 2).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "slug": { "type": "string" },
                    "resolution": { "type": "integer" }
                },
                "required": ["slug"]
            }
        }),
        json!({
            "name": "subscribe",
            "description": "Subscribe to a slice of the world. Returns a subscription_id and an initial snapshot; poll with poll_deltas.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "component_types": string_list,
                    "entities": { "type": "array", "items": entity_schema() }
                },
                "required": ["component_types"]
            }
        }),
        json!({
            "name": "poll_deltas",
            "description": "Return the delta batches for a subscription since the last poll. resync_required true means re-subscribe.",
            "inputSchema": {
                "type": "object",
                "properties": { "subscription_id": { "type": "integer" } },
                "required": ["subscription_id"]
            }
        }),
        json!({
            "name": "unsubscribe",
            "description": "Tear down a subscription.",
            "inputSchema": {
                "type": "object",
                "properties": { "subscription_id": { "type": "integer" } },
                "required": ["subscription_id"]
            }
        }),
        json!({
            "name": "get_viewer_state",
            "description": "Read the full viewer state: render settings (atmosphere, sky, grid, exposure, tonemap, debug overlays), current selection, loaded model counts, and the asset-browser index lists (Khronos models with glb_url, Polyhaven hdris and models with slugs) so you can see what is available to grab. If a list is idle, run a refresh_browsers viewer_action first.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "set_environment",
            "description": "Set the sky and environment. All fields optional. atmosphere is one of None, Sky, CloudySky, Space, Nebula, Sunset, DayNight, Hdr. hour (0-24) drives the DayNight sun. clear_color is linear RGBA used when atmosphere is None. hdri_uri fetches an .hdr and uses it as the skybox.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "atmosphere": { "type": "string" },
                    "show_sky": { "type": "boolean" },
                    "clear_color": { "type": "array", "items": { "type": "number" } },
                    "hour": { "type": "number" },
                    "exposure": { "type": "number" },
                    "hdri_uri": { "type": "string" }
                }
            }
        }),
        json!({
            "name": "viewer_action",
            "description": "Perform any viewer UI action (everything a user can click). The action is an externally tagged ClientMessage object, e.g. {\"SetGrid\":{\"enabled\":false}}, {\"AddPrimitive\":{\"kind\":\"Cube\"}}, {\"AddLight\":{\"kind\":\"Point\"}}, {\"Frame\":null} or \"Frame\", {\"SetTurntable\":{\"enabled\":true}}, {\"SetShadingMode\":{\"mode\":\"Rendered\"}}, {\"SetExposure\":{\"exposure\":1.2}}, {\"SetTonemap\":{\"algorithm\":\"Aces\"}}, {\"SetShowSky\":{\"show\":true}}, {\"SetShowBounds\":{\"enabled\":true}}, {\"SetShowNormals\":{\"enabled\":true}}, {\"SetVariant\":{\"name\":\"red\"}}, {\"SetGizmoMode\":{\"mode\":\"Translate\"}}, {\"PlayAnimation\":{\"index\":0}}, \"PauseAnimation\", \"ResumeAnimation\", \"StopAnimation\", {\"SeekAnimation\":{\"time\":1.0}}, {\"SetAnimationSpeed\":{\"speed\":2.0}}, {\"SetAnimationLoop\":{\"looping\":true}}, {\"Select\":{\"id\":3}}, \"Deselect\", {\"LoadKhronos\":{\"name\":\"Duck\"}}, {\"LoadPolyhaven\":{\"slug\":\"kloofendal_48d_partly_cloudy_puresky\",\"resolution\":2}}, {\"LoadPolyhavenModel\":{\"slug\":\"...\",\"resolution\":2}}, \"RefreshBrowsers\". Use get_viewer_state to discover Khronos and Polyhaven slugs. Khronos and Polyhaven model loads via these actions replace the scene; use load_gltf for additive model loads.",
            "inputSchema": {
                "type": "object",
                "properties": { "action": { "description": "An externally tagged ClientMessage." } },
                "required": ["action"]
            }
        }),
    ]
}

fn rpc_result(id: Option<Value>, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id.unwrap_or(Value::Null), "result": result })
}

fn rpc_error(id: Option<Value>, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id.unwrap_or(Value::Null), "error": { "code": code, "message": message } })
}

fn pretty(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

fn log(message: &str) {
    eprintln!("[nightshade-mcp] {message}");
}
