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

protocol holds the message and data types, the one place the wire is defined. worker is the wasm module: the engine World plus a ViewerWorld, a freecs world that holds the viewer's own state (selection, the loaded model, camera input, the browser fetch state), driven by free functions in worker/src/systems. The root crate is the Leptos UI. Nightshade is the published crate with its default features.

## The message protocol

protocol/src/lib.rs is the contract. The page sends ClientMessage: camera moves (Orbit, Pan, Zoom, Frame), Pick, Select and Deselect, SetTransform, DropAsset, LoadKhronos and LoadPolyhaven, RefreshBrowsers. The worker sends WorkerMessage: Ready, Stats, Scene (the flattened tree), Selected (the inspector detail), Loading, KhronosList, PolyhavenList.

Most messages go through serde_wasm_bindgen. Dropped file bytes do not. They ride as a transferred Uint8Array in the envelope, keyed by BYTES_KEY, next to the serialized message, so a multi-megabyte model moves zero-copy instead of serializing into a JS array of numbers.

## The asset pipeline

The worker does the fetching, with the engine's ehttp re-export. On startup it fetches the Khronos model-index.json and the Polyhaven assets API, then streams KhronosList and PolyhavenList to the page for the browser grids. Thumbnails load straight from their CDNs as img tags, so they never pass through the worker.

A click in a browser sends LoadKhronos or LoadPolyhaven with an id. The worker resolves the URL (the GLB variant, or the HDRI files API), downloads the bytes into an inbox behind an Arc<Mutex> so the ehttp callback can write to it, and a poll system applies whatever landed. A drop or an open reads the local file on the page and transfers its bytes with DropAsset.

The inbox carries a kind. A model goes through import_gltf_from_bytes, queue_gltf_load, spawn_prefab. An HDRI goes through load_hdr_skybox with the atmosphere set to Hdr. Loading anything despawns the previous model first and requests a mesh rebuild.

## Scene and selection

After a load the worker walks the spawned entities from the root with query_descendants, records each node's depth and name, and posts Scene. Selection is one engine entity. A tree click and a viewport pick both resolve to it: the pick goes through the engine's GPU picking, request_pick then take_result, and the entity_id maps back to a freecs::Entity by its raw id. The worker writes that entity into editor_selection.bounding_volume_selected_entity, which the engine's selection mask and outline passes read to draw the highlight, then posts Selected with the entity's transform, the rotation converted from a quaternion to Euler degrees for the inspector fields. An inspector edit comes back as SetTransform, which writes the local transform and marks it dirty.

## Build

just run builds the worker to wasm with wasm-bindgen and wasm-opt, generates the Tailwind stylesheet, and serves the bundle with Trunk. A push to main runs the same steps in .github/workflows/deploy.yml and publishes dist/ to GitHub Pages.
