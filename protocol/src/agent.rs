use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A full generational entity handle as it crosses the agent boundary. Never a
/// bare index: a reused slot reads as a different handle, not the same entity
/// mutated, so a command or read against a stale handle fails rather than
/// hitting the new occupant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityRef {
    pub id: u32,
    pub generation: u32,
}

/// Correlation id for request/response matching and the optional delta origin
/// tag. Unique per host process lifetime.
pub type CorrelationId = u64;

/// The host-assigned subscription handle. Unique for the host process lifetime,
/// never reused.
pub type SubscriptionId = u64;

/// The collection system's own monotonic batch counter. Not the freecs tick.
pub type Version = u64;

/// Whether the generic component bag may carry a component.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WritePolicyInfo {
    /// Plain data: the bag may carry it, no side effects on write.
    Free,
    /// A named command owns this component's cascade; reject it in the bag and
    /// point the agent at the command.
    Owned { command: String },
    /// A system computes this component; no command writes it and the bag must
    /// not carry it.
    Derived,
}

/// One registry entry as the agent discovers it. Schema and example are derived
/// from the same type so they cannot drift from the wire format.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComponentInfo {
    pub name: String,
    pub write_policy: WritePolicyInfo,
    /// Structural shape of the component's value.
    pub schema: Value,
    /// A concrete sample value (the serialized default), a complement to the
    /// schema for the flat cases.
    pub example: Value,
}

/// The command set. Named variants exist where the engine does something beyond
/// writing data (a cascade); the generic setters are the open end the registry
/// absorbs new components through.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AgentCommand {
    Reparent {
        child: EntityRef,
        new_parent: Option<EntityRef>,
    },
    DeleteEntity {
        entity: EntityRef,
    },
    LoadGltf {
        uri: String,
    },
    SelectNode {
        entity: EntityRef,
    },
    SpawnEntity {
        components: Vec<(String, Value)>,
    },
    SetComponents {
        entity: EntityRef,
        components: Vec<(String, Value)>,
    },
    RemoveComponents {
        entity: EntityRef,
        component_types: Vec<String>,
    },
}

/// Which components and optionally which entities a subscription covers.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubscriptionFilter {
    pub component_types: Vec<String>,
    pub entities: Option<Vec<EntityRef>>,
}

/// Host to worker. Every variant carries a correlation id except the resync
/// request, which is keyed by subscription.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AgentRequest {
    ListComponentTypes {
        correlation_id: CorrelationId,
    },
    Query {
        correlation_id: CorrelationId,
        component_types: Vec<String>,
    },
    GetComponents {
        correlation_id: CorrelationId,
        entity: EntityRef,
        component_types: Vec<String>,
    },
    Command {
        correlation_id: CorrelationId,
        command: AgentCommand,
    },
    Subscribe {
        correlation_id: CorrelationId,
        filter: SubscriptionFilter,
    },
    Unsubscribe {
        correlation_id: CorrelationId,
        subscription_id: SubscriptionId,
    },
    /// Recovery: the subscriber's watermark, for replay-or-snapshot.
    Resync {
        subscription_id: SubscriptionId,
        known_version: Version,
    },
}

/// A read against a stale handle returns this distinct outcome, never the new
/// occupant's data.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GetResult {
    Live {
        entity: EntityRef,
        components: Vec<(String, Value)>,
    },
    NotLive {
        entity: EntityRef,
    },
}

/// One entity's subscribed slice in a snapshot.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SnapshotEntity {
    pub entity: EntityRef,
    pub components: Vec<(String, Value)>,
}

/// A full point-in-time view of the subscribed slice, stamped with the version
/// the collection system assigned at its slot.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub version: Version,
    pub entities: Vec<SnapshotEntity>,
}

/// A cheap digest of a subscribed slice at a stated version, for the third
/// (drift-catching) layer of desync detection.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Checksum {
    pub version: Version,
    pub digest: u64,
}

/// The five delta kinds. Value-changes come from change detection; structural
/// changes from the freecs structural log. Never coalesced. `origin` is a
/// correlation tag only and never gates whether a delta is applied.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Delta {
    Changed {
        entity: EntityRef,
        component: String,
        value: Value,
        origin: Option<CorrelationId>,
    },
    Added {
        entity: EntityRef,
        component: String,
        value: Value,
        origin: Option<CorrelationId>,
    },
    Removed {
        entity: EntityRef,
        component: String,
        origin: Option<CorrelationId>,
    },
    Spawned {
        entity: EntityRef,
        components: Vec<(String, Value)>,
        origin: Option<CorrelationId>,
    },
    Despawned {
        entity: EntityRef,
        origin: Option<CorrelationId>,
    },
}

/// One frame's emission. Contiguous per subscriber (`next.base == prev.target`),
/// applied atomically. Ordered creates, then values, then deletes. Every frame
/// emits exactly one batch while tracking is on, empty frames included.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeltaBatch {
    pub base_version: Version,
    pub target_version: Version,
    pub deltas: Vec<Delta>,
    pub checksum: Option<Checksum>,
}

/// Worker to host.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AgentResponse {
    ComponentTypes {
        correlation_id: CorrelationId,
        components: Vec<ComponentInfo>,
    },
    QueryResult {
        correlation_id: CorrelationId,
        entities: Vec<EntityRef>,
    },
    GetResult {
        correlation_id: CorrelationId,
        result: GetResult,
    },
    /// A command landed. Independent of the delta stream. For long-running
    /// commands this fires at the version the last effect lands.
    CommandApplied {
        correlation_id: CorrelationId,
        version: Version,
    },
    CommandFailed {
        correlation_id: CorrelationId,
        error: String,
    },
    CommandProgress {
        correlation_id: CorrelationId,
        stage: String,
    },
    Subscribed {
        correlation_id: CorrelationId,
        subscription_id: SubscriptionId,
        snapshot: Snapshot,
    },
    Unsubscribed {
        correlation_id: CorrelationId,
        subscription_id: SubscriptionId,
    },
    /// One batch per tracking-on frame, broadcast to the host fan-out.
    Batch { batch: DeltaBatch },
    /// Recovery result: either replay batches or a fresh snapshot.
    Replay {
        subscription_id: SubscriptionId,
        batches: Vec<DeltaBatch>,
    },
    Resnapshot {
        subscription_id: SubscriptionId,
        snapshot: Snapshot,
    },
}
