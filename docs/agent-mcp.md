# Driving the viewer from an agent (MCP)

An external agent (Claude Code, or any MCP client) reads and mutates the live
`ViewerWorld` through the [Model Context Protocol](https://modelcontextprotocol.io).
It gets everything a user clicking the UI can do, plus structured queries,
component-level edits, and a live delta stream the UI itself does not expose.

The agent never touches the engine directly. The engine only runs in the
browser, so the agent speaks MCP to a small native bridge, which relays over a
websocket to the page, which forwards onto the worker's existing `postMessage`
path.

```
Claude Code  --MCP stdio-->  nightshade-mcp (native bridge)
                                   |  ws://127.0.0.1:8787
                              Leptos page (relay)
                                   |  postMessage
                              wasm worker (Nightshade World + agent systems)
```

## How it works

**The bridge holds no engine state.** The world lives in the worker. The bridge
(`host/`) is a stdio MCP server in front of a websocket server: it turns each
`tools/call` into an `AgentRequest`, sends it to the page, and waits for the
matching `AgentResponse`. The page (`src/relay.rs`) is a thin websocket client
that hands requests to the worker and ships responses back. The worker
(`worker/src/agent.rs`) is where requests actually meet the `World`.

**Correlation, not ordering.** Every request carries a correlation id. Responses
are matched by it, so many requests can be in flight and a slow command (a model
download) never blocks a fast read. A long command may emit `CommandProgress`
notes before its terminal reply; the bridge treats those as informational and
keeps waiting.

**Everything applies immediately.** A query, a `get_components`, *and* a mutating
command all run against the idle world the instant the request arrives (between
frames) and acknowledge right away. Writes never wait on a render tick, so they
stay fast even when the browser tab is backgrounded, which throttles the frame
loop. A write's cascade (a `local_transform` edit re-deriving the world matrix, a
`material_ref` write rebinding the renderer) is applied with it. Loads fetch
asynchronously and acknowledge once their bytes land and spawn.

**The delta stream.** An `agent_collect` system runs last each frame. It keeps a
shadow copy of every tracked entity's component mask and diffs it against the
current world, emitting five delta kinds - `Spawned`, `Despawned`, `Added`,
`Removed`, `Changed` - plus value changes from the engine's change-detection
window. Deltas are bundled into version-stamped `DeltaBatch`es and held in a ring
buffer in the bridge. A subscription returns a snapshot at a version; polling
returns the contiguous batches since the last poll. If the cursor has aged out of
the ring, the poll returns `resync_required` and the agent re-subscribes.

## What the agent can do

The bridge exposes 25 tools. Entities are always full generational handles:
`{ "id": <int>, "generation": <int> }`. A reused slot reads as a *different*
handle, so a command against a stale handle fails rather than hitting whatever
now occupies the slot.

### Discover the world

- **`list_component_types`**: every registered component with its name, write
  policy (see below), JSON **schema**, and an example value. This is how the
  agent learns the exact shape of a `local_transform`, `light`, `camera`, etc.
  before writing one.
- **`query { component_types }`**: the entity handles whose archetype contains
  *all* of the named components. The agent's primary "find things" tool.
- **`get_components { entity, component_types }`**: the serialized values for one
  entity. A stale handle returns a not-live marker, never another entity's data.
- **`get_viewer_state`**: render settings (atmosphere, sky, grid, exposure,
  tonemap, debug overlays), the current selection (handle plus its name and
  `local_transform`), loaded-model counts, and FPS. Small and cheap, so it is the
  one call for questions like "what is selected". The asset catalog is separate
  (`list_assets`) to keep this lightweight.
- **`list_assets { search? }`**: the asset-browser index lists, Khronos models
  (with `glb_url`) and Polyhaven HDRIs and models (with `slug`s, each tagged with
  its `categories`). Pass `search` to filter by a case-insensitive substring of
  an asset's name, slug, category, or tag (e.g. `"chair"`, `"furniture"`,
  `"sunset"`) and get back only the matches; omit it for the whole catalog
  (large). The response also lists `model_categories` and `hdri_categories` so
  the agent can see what to search by. The viewer fetches the indices once at
  startup and the call waits for that, so a single call returns results (no
  `idle`/retry handling).

### Spawn and edit entities

- **`spawn_entity { components }`**: spawn an entity carrying a component bag
  (`{ "local_transform": {...}, "name": {...} }`). Only writable components are
  accepted.
- **`set_components { entity, components }`**: write a bag onto an existing
  entity.
- **`remove_components { entity, component_types }`**: drop the named components.
- **`reparent { child, new_parent? }`**: re-parent a child; omit `new_parent` (or
  pass null) to detach it to the scene root.
- **`delete_entity { entity }`**: despawn an entity and its descendants.
- **`select_node { entity }`**: select in the viewer, driving the inspector and
  gizmo just like a tree click.

### Add content

- **`add_primitive { kind, components? }`**: spawn a parametric mesh (`Cube`,
  `Sphere`, `Cylinder`, `Cone`, `Torus`, `Plane`) and apply the optional bag
  (e.g. `local_transform` to place/scale, `material_ref` to color it) *at spawn*.
  Returns the new entity's root handle.
- **`add_light { kind, components? }`**: spawn a light (`Directional`, `Point`,
  `Spot`) and the optional bag (e.g. `local_transform` to place it), returning its
  handle. The agent's lights are bare (no marker mesh), so they illuminate without
  adding a stray object to the scene.
- **`load_gltf { uri }`**: load a glTF/GLB by URI **additively**, returning the
  spawned root handle(s).
- **`load_polyhaven_model { slug, resolution? }`**: pull a Polyhaven model by
  slug (from `list_assets`' `models` list) and load it **additively**, returning
  its root handle(s) to position with `set_components`. `resolution` is texture k
  (default 2).

`add_primitive`, `add_light`, and the loaders all return
`{ applied, version, roots: [entity] }`, so the agent can place what it just made
in the next call (or the same batch, see below).

### Scene and camera

- **`clear_scene`**: despawn the whole current scene (the default startup model
  and everything the agent has spawned), leaving an empty stage with the camera,
  sun, and environment intact. Call it first to build from scratch instead of
  around whatever is already loaded.
- **`set_active_camera { entity }`**: make a camera entity the active viewport
  camera (find cameras by querying the `camera` component). The viewer keeps a
  controllable pan-orbit camera alive at all times, respawning one the moment the
  active camera is deleted or cleared, so viewer control is never lost.

### Materials

- **`set_material { name, ... }`**: create or edit a named material in the library.
  Only the fields you set are written, so editing keeps the rest (including
  textures): `base_color` (linear RGBA), `metallic`, `roughness`,
  `emissive_factor`, `emissive_strength`, `unlit`, `double_sided`,
  `base_texture`. `base_texture` is the name of a loaded texture: the engine ships
  three prototype textures always available, `checkerboard`, `gradient`, and
  `uv_test`, and any texture from a loaded model is referenceable by its name
  (visible via `list_materials`). Assign the material to entities with
  `set_components` `material_ref { "name": "<name>" }` (a bare string
  `material_ref: "<name>"` is also accepted); the write rebinds the renderer so
  the new material actually shows.
- **`list_materials`**: every material in the library with its core PBR
  properties. Use it to find a loaded model's material name (also visible via
  `get_components material_ref`) so you can edit it in place.

### Environment and sky

- **`set_environment { ... }`**: all fields optional. `atmosphere` is one of
  `None`, `Sky`, `CloudySky`, `Space`, `Nebula`, `Sunset`, `DayNight`, `Hdr`.
  `hour` (0 to 24) drives the `DayNight` sun. `clear_color` (linear RGBA) is used
  when `atmosphere` is `None`. `exposure` sets tonemap exposure. `show_sky`
  toggles the skybox. `hdri_uri` fetches an `.hdr` and uses it as the skybox
  (sets `atmosphere` to `Hdr`).

### Full UI parity

- **`viewer_action { action }`**: perform *any* viewer action a user could
  click. `action` is an externally tagged `ClientMessage`, e.g.
  `{"SetGrid":{"enabled":false}}`, `{"SetShadingMode":{"mode":"Rendered"}}`,
  `{"SetTonemap":{"algorithm":"Aces"}}`, `{"SetExposure":{"exposure":1.2}}`,
  `{"SetTurntable":{"enabled":true}}`, `{"PlayAnimation":{"index":0}}`,
  `"PauseAnimation"`, `{"SeekAnimation":{"time":1.0}}`,
  `{"SetGizmoMode":{"mode":"Translate"}}`, `{"SetVariant":{"name":"red"}}`,
  `"Frame"`, `{"Select":{"id":3}}`, `"Deselect"`, `"RefreshBrowsers"`.

  It also covers the **scene-replacing** loads: `{"LoadKhronos":{"name":"Duck"}}`,
  `{"LoadPolyhaven":{"slug":"...","resolution":2}}` (an HDRI sky), and
  `{"LoadPolyhavenModel":{"slug":"..."}}`. These replace the current model - use
  `load_gltf` / `load_polyhaven_model` for additive loads instead. Discover slugs
  and Khronos names with `get_viewer_state`.

### Watch the world change

- **`subscribe { component_types, entities? }`** -> `{ subscription_id, version,
  snapshot }`. An empty `component_types` tracks everything; `entities` narrows to
  specific handles.
- **`poll_deltas { subscription_id }`** -> `{ resync_required, version, batches }`.
  Returns the contiguous batches since the last poll; `resync_required` means the
  cursor aged out and the agent should re-subscribe.
- **`unsubscribe { subscription_id }`**.

This is how an agent watches an animation, a physics step, or its own edits land
without re-querying the whole scene.

### Do it fast: batching and `$ref`

- **`batch { ops: [{ tool, arguments }] }`**: run many tool calls in **one** MCP
  round trip, in order, returning each result. This is the bulk of scene
  building; without it every call is a separate slow agent round trip.

  A later op can reference an earlier op's result with a `{"$ref":"<index>.<path>"}`
  placeholder anywhere in its arguments. So **spawn and placement fit in one
  batch**: op 0 is `load_polyhaven_model`, op 1 is `set_components` with
  `{"entity":{"$ref":"0.roots.0"},"components":{"local_transform":{...}}}` to place
  the model op 0 just loaded. (`add_primitive` / `add_light` can also carry their
  `local_transform` / `material_ref` inline and need no follow-up at all.) Nested
  batches are rejected.

## Write policies

`list_component_types` tags every component with how the generic bag may touch it:

- **Free**: the agent may write it directly: `local_transform`, `name`,
  `visibility`, `camera`, `light`, `material_ref`, `casts_shadow`,
  `render_layer`.
- **Owned by a command**: a structural invariant the bag must not break, so it is
  reserved to a dedicated command. `parent` is owned by `reparent` (a raw parent
  write would desync the transform hierarchy).
- **Derived**: computed by the engine and not writable at all: `bounding_volume`.

Writing an owned or derived component through `spawn_entity` / `set_components`
fails with a message naming the command to use instead.

## Schemas

The component schemas in `list_component_types`, and the `inputSchema` of every
tool, are derived from the Rust types with
[enum2schema](https://crates.io/crates/enum2schema) - the engine's own component
types in the worker, the agent argument and payload types in the bridge. The
schema and the example value for each component both flow through the same
`serde` impl, so they cannot drift from what the deserializer actually accepts.

## Feature flag

The entire agent surface is behind a non-default `agent` cargo feature, so a
plain `just run` / `just dist` build carries none of it: no relay websocket, no
agent message types, no registry or agent systems in the worker. A deployed
viewer never opens a localhost socket.

The feature spans three crates and is wired so enabling it on the app and worker
turns it on in `protocol` too:

- `protocol/agent` - the agent message types and their `Schema` derives.
- `worker/agent` - the component registry, command apply, and delta collection.
- the app's `agent` - the page's relay websocket (`src/relay.rs`).

The `host` bridge always enables `protocol/agent`; it *is* the agent and has no
flag of its own.

Build the agent-enabled worker and app **together**: mismatched features would
desync the `protocol` enums between the two halves. The `*-agent` just recipes do
this for you.

## Run it

1. Serve the viewer with the agent feature on (builds the worker and app with
   `--features agent` and serves at http://127.0.0.1:8080):

   ```
   just run-agent
   ```

2. Build the bridge (its own workspace, so build it by manifest path):

   ```
   cargo build --manifest-path host/Cargo.toml
   ```

3. Register the bridge with Claude Code (from this repo root):

   ```
   claude mcp add nightshade-viewer -- C:\Users\matth\code\nightshade-viewer\host\target\debug\nightshade-mcp.exe
   ```

   Claude Code spawns the bridge, which binds `ws://127.0.0.1:8787`. The page
   reconnects automatically, so start order does not matter - open the tab and
   the relay finds the bridge.

## Example sessions

With the server registered, in a Claude Code session:

- "What is selected, and what is its transform?" (one `get_viewer_state`).
- "Clear the scene, then spawn a row of five cubes one unit apart, each with its
  own colored material." (a `clear_scene`, then one `batch` of a `set_material`
  and an `add_primitive` with `material_ref` + `local_transform` per cube)
- "Give the floor the checkerboard prototype texture." (`set_material` with
  `base_texture: "checkerboard"`, then `set_components material_ref`)
- "Pull a Polyhaven model and scatter four copies around the origin." (`list_assets`
  for a slug, then a `batch` of `load_polyhaven_model` ops each followed by a
  `set_components` referencing `{"$ref":"<i>.roots.0"}`)
- "Set an HDRI sky from this `.hdr` URL and turn the grid off."
- "Subscribe to `local_transform`, play the first animation, then poll a few
  times and tell me which entities moved."
