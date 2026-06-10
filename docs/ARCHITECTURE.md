# Architecture

How nightshade-viewer is wired: the crates, the thread split, the message protocol, and the asset pipeline. Paths are relative to the repository root.

## Two threads

The engine runs on a web worker. The page runs on the main thread. They share nothing but messages. The worker owns the OffscreenCanvas, the Nightshade World, and the network fetching. The page owns the DOM. It forwards input and renders its panels from the messages the worker sends back.

```
MAIN THREAD (Leptos)                  WEB WORKER (Nightshade)
src/app.rs        compose             worker/src/lib.rs     message pump, render loop
src/bridge.rs     postMessage         worker/src/state.rs   Viewer, the State impl
src/components/*  viewport, panels    worker/src/ecs.rs     ViewerWorld (freecs)
src/state.rs      grouped signals     worker/src/systems/*  camera, load, picking, scene, ...

  transfer_control_to_offscreen()      ->  create_wgpu_renderer(OffscreenCanvas)
  ClientMessage (+ transferred bytes)  ->  handle_message, systems
  WorkerMessage                        <-  Scene, Selected, Stats, browser lists
```

## Crates

protocol holds the message and data types, the one place the wire is defined. worker is the wasm module: the engine World plus a ViewerWorld, a freecs world that holds the viewer's own state (selection, the loaded model, camera input, the browser fetch state), driven by free functions in worker/src/systems. The root crate is the Leptos UI. desktop is the native shell that hosts the web bundle in a webview window. host is the native nightshade-mcp bridge that exposes the viewer to an external MCP agent, built only with the optional agent feature. Nightshade is the published crate with its default features.

## The message protocol

protocol/src/lib.rs is the contract. The page sends ClientMessage: camera moves (Orbit, Pan, Zoom, Frame), Pick, Select and Deselect, SetTransform, DropAsset, LoadKhronos and LoadPolyhaven, RefreshBrowsers. The worker sends WorkerMessage: Ready, Stats, Scene (the flattened tree), Selected (the inspector detail), Loading, KhronosList, PolyhavenList.

Most messages go through serde_wasm_bindgen. Dropped file bytes do not. They ride as a transferred Uint8Array in the envelope, keyed by BYTES_KEY, next to the serialized message, so a multi-megabyte model moves zero-copy instead of serializing into a JS array of numbers.

## The asset pipeline

The worker does the fetching, with the engine's ehttp re-export. On startup it fetches the Khronos model-index.json and the Polyhaven assets API, then streams KhronosList and PolyhavenList to the page for the browser grids. Thumbnails load straight from their CDNs as img tags, so they never pass through the worker.

A click in a browser sends LoadKhronos or LoadPolyhaven with an id. The worker resolves the URL (the GLB variant, or the HDRI files API), downloads the bytes into an inbox behind an Arc<Mutex> so the ehttp callback can write to it, and a poll system applies whatever landed. A drop or an open reads the local file on the page and transfers its bytes with DropAsset.

The inbox carries a kind. A model goes through import_gltf_from_bytes, queue_gltf_load, spawn_prefab. An HDRI goes through load_hdr_skybox with the atmosphere set to Hdr. Loading anything despawns the previous model first and requests a mesh rebuild.

## Scene and selection

After a load the worker walks the spawned entities from the root with query_descendants, records each node's depth and name, and posts Scene. Selection is one engine entity. A tree click and a viewport pick both resolve to it: the pick goes through the engine's GPU picking, request_pick then take_result, and the entity_id maps back to a freecs::Entity by its raw id. The worker writes that entity into editor_selection.bounding_volume_selected_entity, which the engine's selection mask and outline passes read to draw the highlight, then posts Selected with the entity's transform, the rotation converted from a quaternion to Euler degrees for the inspector fields. An inspector edit comes back as SetTransform, which writes the local transform and marks it dirty.

## The agent surface

Behind the optional agent feature, an external MCP agent can read and mutate the live world with the same access a user has plus structured queries and a delta stream. It is a fourth process: the agent speaks MCP to the native host bridge, which relays over a websocket to the page (src/relay.rs), which forwards onto the same worker postMessage path as the UI. The bridge holds no engine state; the world stays in the worker, where agent_apply (before transform propagation) and agent_collect (last in the frame) systems handle commands and diff the world into version-stamped delta batches. The Agent variant on ClientMessage and WorkerMessage carries the requests and responses. The whole surface compiles out by default, so a deployed build opens no localhost socket. See agent-mcp.md for the full design, tool list, and how to drive it.

## The desktop shell

The desktop crate runs the viewer as a standalone app without changing anything above. There is no second rendering path: the same wasm, worker, and Leptos bundle that a browser loads runs inside a wry webview (WebView2 on Windows, WebKit elsewhere, with WebGPU enabled through a browser flag on Windows). At startup a tiny_http server on a background thread serves the Trunk dist on an ephemeral 127.0.0.1 port, and a winit window hosts a webview pointed at it. Localhost is a secure context, so WebGPU and module workers behave exactly as they do in a browser tab, which is why the bundle is served over a port rather than loaded from a custom protocol. A navigation handler pins the webview to localhost.

The bundle rides along via rust-embed. Debug builds read dist from disk at request time, so a fresh trunk build shows up on relaunch without recompiling the shell; release builds embed the files in the executable, so just build-desktop produces a single self-contained binary.

## Build

just run builds the worker to wasm with wasm-bindgen and wasm-opt, generates the Tailwind stylesheet, builds the bundle with Trunk, and opens it in a native webview window. just run-web serves the bundle at 127.0.0.1:8080 for a browser instead. A push to main runs the same steps in .github/workflows/deploy.yml and publishes dist/ to GitHub Pages. just run-agent serves the web build with the agent feature on, for both the worker and the page.
