use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use futures_util::{SinkExt, StreamExt};
use protocol::{
    AgentCommand, AgentRequest, AgentResponse, CorrelationId, DeltaBatch, Environment,
    MaterialSpec, SubscriptionFilter, SubscriptionId, Version,
};
use serde::de::DeserializeOwned;
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
        | AgentResponse::ViewerState { correlation_id, .. }
        | AgentResponse::Materials { correlation_id, .. }
        | AgentResponse::Assets { correlation_id, .. } => Some(*correlation_id),
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
        | AgentRequest::SetEnvironment { correlation_id, .. }
        | AgentRequest::SetMaterial { correlation_id, .. }
        | AgentRequest::ListMaterials { correlation_id }
        | AgentRequest::ListAssets { correlation_id } => *correlation_id,
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

/// Typed tool arguments. Each struct drives both deserialization and, via
/// `enum2schema`, the tool's `inputSchema`, so the two cannot drift.
mod args {
    use enum2schema::Schema;
    use protocol::{ClientMessage, EntityRef, LightKind, PrimitiveKind};
    use serde::Deserialize;
    use serde_json::{Map, Value};

    /// A component bag: component name to its JSON value.
    pub type Bag = Map<String, Value>;

    #[derive(Deserialize, Schema, Default)]
    pub struct Empty {}

    #[derive(Deserialize, Schema)]
    pub struct Query {
        /// Component type names that an entity must all have.
        pub component_types: Vec<String>,
    }

    #[derive(Deserialize, Schema)]
    pub struct GetComponents {
        pub entity: EntityRef,
        pub component_types: Vec<String>,
    }

    #[derive(Deserialize, Schema)]
    pub struct Spawn {
        #[serde(default)]
        pub components: Bag,
    }

    #[derive(Deserialize, Schema)]
    pub struct SetComponents {
        pub entity: EntityRef,
        #[serde(default)]
        pub components: Bag,
    }

    #[derive(Deserialize, Schema)]
    pub struct RemoveComponents {
        pub entity: EntityRef,
        pub component_types: Vec<String>,
    }

    #[derive(Deserialize, Schema)]
    pub struct Reparent {
        pub child: EntityRef,
        /// Omit or null to detach to the scene root.
        #[serde(default)]
        pub new_parent: Option<EntityRef>,
    }

    #[derive(Deserialize, Schema)]
    pub struct Entity {
        pub entity: EntityRef,
    }

    #[derive(Deserialize, Schema)]
    pub struct LoadGltf {
        pub uri: String,
    }

    #[derive(Deserialize, Schema)]
    pub struct LoadPolyhavenModel {
        pub slug: String,
        /// Texture resolution in k. Defaults to 2.
        #[serde(default)]
        pub resolution: Option<u32>,
    }

    #[derive(Deserialize, Schema)]
    pub struct AddPrimitive {
        pub kind: PrimitiveKind,
        /// Applied at spawn (e.g. local_transform, material_ref).
        #[serde(default)]
        pub components: Bag,
    }

    #[derive(Deserialize, Schema)]
    pub struct AddLight {
        pub kind: LightKind,
        #[serde(default)]
        pub components: Bag,
    }

    #[derive(Deserialize, Schema)]
    pub struct ViewerAction {
        /// An externally tagged ClientMessage, e.g. {"SetGrid":{"enabled":false}}.
        pub action: ClientMessage,
    }

    #[derive(Deserialize, Schema)]
    pub struct SubscriptionId {
        pub subscription_id: u64,
    }

    #[derive(Deserialize, Schema)]
    pub struct Batch {
        pub ops: Vec<Op>,
    }

    #[derive(Deserialize, Schema)]
    pub struct Op {
        pub tool: String,
        #[serde(default)]
        pub arguments: Value,
    }
}

/// Deserializes tool arguments into a typed struct, reporting a readable error.
fn parse<T: DeserializeOwned>(arguments: Value) -> Result<T, String> {
    serde_json::from_value(arguments).map_err(|error| format!("invalid arguments: {error}"))
}

/// Flattens a component-bag object into the `(name, value)` pairs the worker takes.
fn bag(map: args::Bag) -> Vec<(String, Value)> {
    map.into_iter().collect()
}

/// Formats a terminal command response as an applied/version object.
fn applied_result(response: AgentResponse) -> Result<String, String> {
    match response {
        AgentResponse::CommandApplied { version, .. } => {
            Ok(json!({ "applied": true, "version": version }).to_string())
        }
        AgentResponse::CommandFailed { error, .. } => Err(error),
        other => Ok(compact(&response_payload(other))),
    }
}

async fn run_tool(shared: &Arc<Shared>, name: &str, arguments: Value) -> Result<String, String> {
    match name {
        "list_component_types" => {
            let correlation_id = shared.correlation();
            let response =
                send_request(shared, AgentRequest::ListComponentTypes { correlation_id }).await?;
            Ok(compact(&response_payload(response)))
        }
        "query" => {
            let typed: args::Query = parse(arguments)?;
            let correlation_id = shared.correlation();
            let response = send_request(
                shared,
                AgentRequest::Query {
                    correlation_id,
                    component_types: typed.component_types,
                },
            )
            .await?;
            Ok(compact(&response_payload(response)))
        }
        "get_components" => {
            let typed: args::GetComponents = parse(arguments)?;
            let correlation_id = shared.correlation();
            let response = send_request(
                shared,
                AgentRequest::GetComponents {
                    correlation_id,
                    entity: typed.entity,
                    component_types: typed.component_types,
                },
            )
            .await?;
            Ok(compact(&response_payload(response)))
        }
        "spawn_entity" => {
            let typed: args::Spawn = parse(arguments)?;
            command(
                shared,
                AgentCommand::SpawnEntity {
                    components: bag(typed.components),
                },
            )
            .await
        }
        "set_components" => {
            let typed: args::SetComponents = parse(arguments)?;
            command(
                shared,
                AgentCommand::SetComponents {
                    entity: typed.entity,
                    components: bag(typed.components),
                },
            )
            .await
        }
        "remove_components" => {
            let typed: args::RemoveComponents = parse(arguments)?;
            command(
                shared,
                AgentCommand::RemoveComponents {
                    entity: typed.entity,
                    component_types: typed.component_types,
                },
            )
            .await
        }
        "reparent" => {
            let typed: args::Reparent = parse(arguments)?;
            command(
                shared,
                AgentCommand::Reparent {
                    child: typed.child,
                    new_parent: typed.new_parent,
                },
            )
            .await
        }
        "clear_scene" => command(shared, AgentCommand::ClearScene).await,
        "delete_entity" => {
            let typed: args::Entity = parse(arguments)?;
            command(
                shared,
                AgentCommand::DeleteEntity {
                    entity: typed.entity,
                },
            )
            .await
        }
        "select_node" => {
            let typed: args::Entity = parse(arguments)?;
            command(
                shared,
                AgentCommand::SelectNode {
                    entity: typed.entity,
                },
            )
            .await
        }
        "set_active_camera" => {
            let typed: args::Entity = parse(arguments)?;
            command(
                shared,
                AgentCommand::SetActiveCamera {
                    entity: typed.entity,
                },
            )
            .await
        }
        "load_gltf" => {
            let typed: args::LoadGltf = parse(arguments)?;
            spawn_command(shared, AgentCommand::LoadGltf { uri: typed.uri }).await
        }
        "viewer_action" => {
            let typed: args::ViewerAction = parse(arguments)?;
            let correlation_id = shared.correlation();
            let response = send_request(
                shared,
                AgentRequest::ViewerAction {
                    correlation_id,
                    message: Box::new(typed.action),
                },
            )
            .await?;
            applied_result(response)
        }
        "get_viewer_state" => {
            let correlation_id = shared.correlation();
            let response =
                send_request(shared, AgentRequest::GetViewerState { correlation_id }).await?;
            if let AgentResponse::ViewerState { state, .. } = response {
                Ok(compact(&state))
            } else {
                Ok(compact(&response_payload(response)))
            }
        }
        "set_environment" => {
            let environment: Environment = parse(arguments)?;
            let correlation_id = shared.correlation();
            let response = send_request(
                shared,
                AgentRequest::SetEnvironment {
                    correlation_id,
                    environment,
                },
            )
            .await?;
            applied_result(response)
        }
        "load_polyhaven_model" => {
            let typed: args::LoadPolyhavenModel = parse(arguments)?;
            spawn_command(
                shared,
                AgentCommand::LoadPolyhavenModel {
                    slug: typed.slug,
                    resolution: typed.resolution.unwrap_or(2),
                },
            )
            .await
        }
        "add_primitive" => {
            let typed: args::AddPrimitive = parse(arguments)?;
            spawn_command(
                shared,
                AgentCommand::AddPrimitive {
                    kind: typed.kind,
                    components: bag(typed.components),
                },
            )
            .await
        }
        "add_light" => {
            let typed: args::AddLight = parse(arguments)?;
            spawn_command(
                shared,
                AgentCommand::AddLight {
                    kind: typed.kind,
                    components: bag(typed.components),
                },
            )
            .await
        }
        "batch" => batch_tool(shared, arguments).await,
        "set_material" => {
            let material: MaterialSpec = parse(arguments)?;
            if material.name.is_empty() {
                return Err("material name is required".to_string());
            }
            let correlation_id = shared.correlation();
            let response = send_request(
                shared,
                AgentRequest::SetMaterial {
                    correlation_id,
                    material,
                },
            )
            .await?;
            applied_result(response)
        }
        "list_materials" => {
            let correlation_id = shared.correlation();
            let response =
                send_request(shared, AgentRequest::ListMaterials { correlation_id }).await?;
            if let AgentResponse::Materials { materials, .. } = response {
                Ok(compact(&materials))
            } else {
                Ok(compact(&response_payload(response)))
            }
        }
        "list_assets" => {
            let correlation_id = shared.correlation();
            let response =
                send_request(shared, AgentRequest::ListAssets { correlation_id }).await?;
            if let AgentResponse::Assets { assets, .. } = response {
                Ok(compact(&assets))
            } else {
                Ok(compact(&response_payload(response)))
            }
        }
        "subscribe" => {
            let filter: SubscriptionFilter = parse(arguments)?;
            subscribe_tool(shared, filter).await
        }
        "poll_deltas" => {
            let typed: args::SubscriptionId = parse(arguments)?;
            poll_deltas_tool(shared, typed.subscription_id).await
        }
        "unsubscribe" => {
            let typed: args::SubscriptionId = parse(arguments)?;
            unsubscribe_tool(shared, typed.subscription_id).await
        }
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
    applied_result(response)
}

/// Runs a list of tool calls in one MCP round trip, returning each result. A
/// later op can reference an earlier op's result with a {"$ref":"<i>.<path>"}
/// placeholder anywhere in its arguments (e.g. {"$ref":"0.roots.0"} is the first
/// root the op at index 0 returned), so spawn-then-place is one batch.
async fn batch_tool(shared: &Arc<Shared>, arguments: Value) -> Result<String, String> {
    let typed: args::Batch = parse(arguments)?;
    let mut refs: Vec<Value> = Vec::with_capacity(typed.ops.len());
    let mut report: Vec<Value> = Vec::with_capacity(typed.ops.len());
    for op in typed.ops {
        let name = op.tool;
        let raw_arguments = if op.arguments.is_null() {
            json!({})
        } else {
            op.arguments
        };
        if name == "batch" {
            refs.push(Value::Null);
            report
                .push(json!({ "tool": name, "ok": false, "error": "nested batch is not allowed" }));
            continue;
        }
        let op_arguments = match resolve_refs(&raw_arguments, &refs) {
            Ok(arguments) => arguments,
            Err(error) => {
                refs.push(Value::Null);
                report.push(json!({ "tool": name, "ok": false, "error": error }));
                continue;
            }
        };
        match Box::pin(run_tool(shared, &name, op_arguments)).await {
            Ok(text) => {
                refs.push(serde_json::from_str(&text).unwrap_or(Value::String(text.clone())));
                report.push(json!({ "tool": name, "ok": true, "result": text }));
            }
            Err(error) => {
                refs.push(Value::Null);
                report.push(json!({ "tool": name, "ok": false, "error": error }));
            }
        }
    }
    Ok(serde_json::to_string(&Value::Array(report)).unwrap_or_default())
}

/// Replaces every {"$ref":"<index>.<path>"} placeholder in `value` with the
/// referenced part of an earlier op's result.
fn resolve_refs(value: &Value, results: &[Value]) -> Result<Value, String> {
    match value {
        Value::Object(map) => {
            if map.len() == 1
                && let Some(Value::String(path)) = map.get("$ref")
            {
                return lookup_ref(path, results);
            }
            let mut resolved = serde_json::Map::new();
            for (key, inner) in map {
                resolved.insert(key.clone(), resolve_refs(inner, results)?);
            }
            Ok(Value::Object(resolved))
        }
        Value::Array(items) => items
            .iter()
            .map(|item| resolve_refs(item, results))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        other => Ok(other.clone()),
    }
}

fn lookup_ref(path: &str, results: &[Value]) -> Result<Value, String> {
    let mut parts = path.split('.');
    let index: usize = parts
        .next()
        .and_then(|segment| segment.parse().ok())
        .ok_or_else(|| format!("bad $ref '{path}'"))?;
    let mut current = results
        .get(index)
        .ok_or_else(|| format!("$ref '{path}': op {index} has not run"))?;
    for part in parts {
        current = match part.parse::<usize>() {
            Ok(array_index) => current
                .get(array_index)
                .ok_or_else(|| format!("$ref '{path}': index {array_index} out of range"))?,
            Err(_) => current
                .get(part)
                .ok_or_else(|| format!("$ref '{path}': no key '{part}'"))?,
        };
    }
    Ok(current.clone())
}

async fn spawn_command(shared: &Arc<Shared>, command: AgentCommand) -> Result<String, String> {
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
        AgentResponse::Loaded { version, roots, .. } => {
            Ok(json!({ "applied": true, "version": version, "roots": roots }).to_string())
        }
        AgentResponse::CommandFailed { error, .. } => Err(error),
        other => Ok(compact(&response_payload(other))),
    }
}

async fn subscribe_tool(
    shared: &Arc<Shared>,
    filter: SubscriptionFilter,
) -> Result<String, String> {
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
        other => Ok(compact(&response_payload(other))),
    }
}

async fn poll_deltas_tool(shared: &Arc<Shared>, subscription_id: u64) -> Result<String, String> {
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

async fn unsubscribe_tool(shared: &Arc<Shared>, subscription_id: u64) -> Result<String, String> {
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
    Ok(compact(&response_payload(response)))
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

fn tool_definitions() -> Vec<Value> {
    use enum2schema::mcp::tool;
    vec![
        tool::<args::Empty>(
            "list_component_types",
            "Discover every component: name, write policy (Free, Owned by a command, or Derived), JSON schema, and an example value.",
        ),
        tool::<args::Query>(
            "query",
            "Return the entity handles whose archetype contains all of the named component types.",
        ),
        tool::<args::GetComponents>(
            "get_components",
            "Return serialized component values for one entity. A stale handle returns a not-live result, never another entity's data.",
        ),
        tool::<args::Spawn>(
            "spawn_entity",
            "Spawn an entity carrying the given component bag. Owned and Derived components are rejected.",
        ),
        tool::<args::SetComponents>(
            "set_components",
            "Write the given component bag onto an existing entity. Owned and Derived components are rejected with the command to use.",
        ),
        tool::<args::RemoveComponents>(
            "remove_components",
            "Remove the named components from an entity.",
        ),
        tool::<args::Reparent>(
            "reparent",
            "Reparent a child entity. Omit new_parent or pass null to detach to the scene root.",
        ),
        tool::<args::Entity>("delete_entity", "Despawn an entity and its descendants."),
        tool::<args::Empty>(
            "clear_scene",
            "Despawn the entire current scene (the default startup model and everything previously spawned), leaving an empty stage with the camera, sun, and environment intact. Call this first when you want to build a scene from scratch rather than around whatever is already loaded.",
        ),
        tool::<args::Entity>(
            "select_node",
            "Select an entity in the viewer (drives the inspector and gizmo).",
        ),
        tool::<args::Entity>(
            "set_active_camera",
            "Make a camera entity the active viewport camera. Query entities with a camera component to find one. The viewer always keeps a controllable pan-orbit camera alive, so use this to switch between cameras you have spawned or loaded.",
        ),
        tool::<args::LoadGltf>(
            "load_gltf",
            "Load a glTF or GLB by URI additively, returning the spawned root handle(s).",
        ),
        tool::<args::AddPrimitive>(
            "add_primitive",
            "Spawn a parametric primitive mesh and apply the optional components bag (local_transform, material_ref) at spawn, returning its root handle. Avoids a separate set_components round trip, so it is batchable.",
        ),
        tool::<args::AddLight>(
            "add_light",
            "Spawn a light and apply the optional components bag (local_transform, light) at spawn, returning its handle.",
        ),
        tool::<args::Batch>(
            "batch",
            "Run many tool calls in ONE round trip. ops is an array of {tool, arguments}, executed in order. A later op may reference an earlier op's result with {\"$ref\":\"<index>.<path>\"} (e.g. {\"$ref\":\"0.roots.0\"} is the first root op 0 returned), so spawn and placement fit in one batch. add_primitive and add_light can carry their components inline and need no follow-up.",
        ),
        tool::<MaterialSpec>(
            "set_material",
            "Create or edit a named material, then assign it with set_components material_ref. Only the fields you set are written, so editing keeps the rest. base_color is linear RGBA.",
        ),
        tool::<args::Empty>(
            "list_materials",
            "List every material in the library with its core PBR properties.",
        ),
        tool::<args::Empty>(
            "list_assets",
            "List the asset catalog the viewer can grab: Khronos models (with glb_url), Polyhaven hdris and models (with slugs). Large, so call it only when browsing; it is not part of get_viewer_state. If a list reads idle, run a RefreshBrowsers viewer_action first.",
        ),
        tool::<args::LoadPolyhavenModel>(
            "load_polyhaven_model",
            "Grab a Polyhaven model by slug (from list_assets' models list) and load it additively, returning the spawned root handle(s) to position with set_components.",
        ),
        tool::<SubscriptionFilter>(
            "subscribe",
            "Subscribe to a slice of the world. Returns a subscription_id and an initial snapshot; poll with poll_deltas.",
        ),
        tool::<args::SubscriptionId>(
            "poll_deltas",
            "Return the delta batches for a subscription since the last poll. resync_required true means re-subscribe.",
        ),
        tool::<args::SubscriptionId>("unsubscribe", "Tear down a subscription."),
        tool::<args::Empty>(
            "get_viewer_state",
            "Read render settings, the current selection (entity handle plus its name and local_transform), and loaded-model counts. Small and cheap; this is the one call for questions like what is selected. The asset catalog is separate (list_assets).",
        ),
        tool::<Environment>(
            "set_environment",
            "Set the sky and environment. atmosphere is one of None, Sky, CloudySky, Space, Nebula, Sunset, DayNight, Hdr. hour (0-24) drives the DayNight sun. clear_color is linear RGBA used when atmosphere is None. hdri_uri fetches an .hdr and uses it as the skybox.",
        ),
        tool::<args::ViewerAction>(
            "viewer_action",
            "Perform any viewer UI action (everything a user can click), e.g. {\"SetGrid\":{\"enabled\":false}}, {\"SetTurntable\":{\"enabled\":true}}, {\"SetShadingMode\":{\"mode\":\"Rendered\"}}, {\"PlayAnimation\":{\"index\":0}}, \"Frame\", {\"LoadPolyhaven\":{\"slug\":\"...\",\"resolution\":2}}, \"RefreshBrowsers\". Use list_assets to discover slugs.",
        ),
    ]
}

fn rpc_result(id: Option<Value>, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id.unwrap_or(Value::Null), "result": result })
}

fn rpc_error(id: Option<Value>, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id.unwrap_or(Value::Null), "error": { "code": code, "message": message } })
}

fn compact(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| value.to_string())
}

fn log(message: &str) {
    eprintln!("[nightshade-mcp] {message}");
}
