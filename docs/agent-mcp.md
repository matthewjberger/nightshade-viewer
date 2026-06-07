# Driving the viewer from Claude Code (MCP)

An external agent (Claude Code, or any MCP client) reads and mutates the
`ViewerWorld` without ever touching it directly. There is one transport: the
engine only runs in the browser, so the agent speaks MCP to a small native
bridge, which relays over a websocket to the page, which forwards onto the
worker's existing `postMessage` path.

```
Claude Code  --MCP stdio-->  nightshade-mcp (native bridge)
                                   |  ws://127.0.0.1:8787
                              Leptos page (relay)
                                   |  postMessage
                              wasm worker (nightshade World + agent systems)
```

The bridge holds no engine state. The world lives in the worker. Every message
carries a correlation id; commands acknowledge independently of the delta
stream; subscriptions return a version-stamped snapshot and are polled for
contiguous, atomically-applied delta batches.

## Pieces

- `protocol` — shared message types (`AgentRequest`, `AgentResponse`, the five
  `Delta` kinds, `DeltaBatch`, `Snapshot`, `ComponentInfo`).
- `host/` — the native `nightshade-mcp` binary: MCP stdio server + websocket
  server + per-subscription delta ring buffer.
- `src/relay.rs` — the page's websocket client that bridges to the worker.
- `worker/src/agent.rs` — the registry, the apply system (before transform
  propagation), and the collection system (last in the frame).

## Feature flag

The entire agent surface is behind a non-default `agent` cargo feature, so a
plain `just run` / `just dist` build carries none of it: no relay websocket, no
agent message types, no registry or agent systems in the worker. This keeps the
deployed viewer clean (a normal build never opens a localhost socket).

The feature spans three crates and is wired so enabling it on the app and worker
turns it on in `protocol` too:

- `protocol/agent` — the agent message types and their schemas.
- `worker/agent` — the registry, command apply, and delta collection.
- the app's `agent` — the page's relay websocket.

The `host` bridge always enables `protocol/agent`; it is the agent and has no
flag of its own.

Build the agent-enabled worker and app together (mismatched features would
desync the `protocol` enums between them), which the `*-agent` just recipes do
for you.

## Run it

1. Build the bridge (once):

   ```
   cargo build --manifest-path host/Cargo.toml
   ```

2. Serve the viewer with the agent feature on:

   ```
   just run-agent
   ```

   This builds the worker and app with `--features agent` and serves at
   http://127.0.0.1:8080. (Plain `just run` builds the viewer without the agent.)

3. Register the bridge with Claude Code (from this repo root):

   ```
   claude mcp add nightshade-viewer -- C:\Users\matth\code\nightshade-viewer\host\target\debug\nightshade-mcp.exe
   ```

   Claude Code spawns the bridge, which binds `ws://127.0.0.1:8787`. The page
   reconnects automatically, so order does not matter.

## Tools

- `list_component_types` — name, write policy (Free / Owned by a command /
  Derived), schema, and example for every registered component.
- `query { component_types }` — entity handles whose archetype has all of them.
- `get_components { entity, component_types }` — values; a stale handle returns
  not-live.
- `spawn_entity { components }`, `set_components { entity, components }`,
  `remove_components { entity, component_types }`.
- `reparent { child, new_parent? }`, `delete_entity { entity }`,
  `select_node { entity }`, `load_gltf { uri }`.
- `subscribe { component_types, entities? }` -> `{ subscription_id, snapshot,
  version }`; `poll_deltas { subscription_id }`; `unsubscribe { subscription_id }`.

Entities are full generational handles: `{ "id": <int>, "generation": <int> }`.

## Try it

In a Claude Code session with the server registered:

- "List the component types in the viewer."
- "Query entities that have a local_transform and a name, then show the name and
  local_transform of the first few."
- "Spawn an entity with a light and a local_transform at y = 3."
- "Subscribe to local_transform, then poll a couple of times while an animation
  plays."
