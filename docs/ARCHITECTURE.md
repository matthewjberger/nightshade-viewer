# Architecture

How `nightshade-viewer` is put together: the crate layout, the thread split, the message protocol, and the asset pipeline. Paths are relative to the repository root.

## The one idea

The whole engine runs on a web worker; the page is pure UI. The worker owns an `OffscreenCanvas`, the Nightshade `World`, and the network fetching. The page owns the DOM: it forwards input, renders the scene tree / inspector / asset browsers from messages the worker streams back, and hands dropped file bytes across. The split is the same as [nightshade-worker-leptos](https://github.com/matthewjberger/nightshade-worker-leptos), grown into a tool.

## Workspace layout

| Crate | Path | Runs on | Role |
|---|---|---|---|
| `protocol` | `protocol/` | both | Shared message and data types. The wire contract. |
| `worker` | `worker/` | worker | The engine `World`, the `ViewerWorld` (a `freecs` world) of viewer state, and the systems. Owns `ehttp` fetching. |
| root | `src/` | main thread | The Leptos UI: `Viewport`, `SceneTree`, `Inspector`, `Toolbar`, `AssetBrowser`. |

Nightshade is the published crate, default features (`engine`, `wgpu`).

## The thread split

```
MAIN THREAD (Leptos)                         WEB WORKER (Nightshade)
--------------------                         -----------------------
src/app.rs        compose panels             worker/src/lib.rs     message pump + render loop
src/bridge.rs     postMessage + decode       worker/src/state.rs   Viewer: State impl
src/components/*  viewport, tree, inspector   worker/src/ecs.rs     ViewerWorld (freecs)
src/state.rs      grouped signals            worker/src/systems/*  camera, load, picking,
                                                                   scene, selection, browsers

  transfer_control_to_offscreen()  ----------->  create_wgpu_renderer(OffscreenCanvas)
  ClientMessage (+ transferred bytes) -------->  handle_message → systems
  WorkerMessage  <----------------------------   Scene / Selected / Stats / browser lists
```

## The message protocol

`protocol/src/lib.rs`. Page to worker (`ClientMessage`): camera (`Orbit`/`Pan`/`Zoom`/`Frame`), `Pick`, `Select`/`Deselect`, `SetTransform`, `DropAsset` (with bytes transferred), `LoadKhronos`/`LoadPolyhaven`, `RefreshBrowsers`. Worker to page (`WorkerMessage`): `Ready`, `Stats`, `Scene` (the flattened tree), `Selected` (inspector detail), `Loading`, `KhronosList`, `PolyhavenList`.

Most messages serialize through `serde_wasm_bindgen`. Dropped file bytes do not: they ride as a transferred `Uint8Array`/`ArrayBuffer` in the envelope (zero-copy), keyed by `BYTES_KEY`, alongside the serialized message.

## The asset pipeline

The worker owns fetching, reusing the engine's `ehttp` re-export:

- **Indices** — on startup the worker fetches the Khronos `model-index.json` and the Polyhaven `assets?type=hdris` API, then streams `KhronosList` / `PolyhavenList` to the page for the browser grids (thumbnails load directly as `<img>` from their CDNs).
- **Load on click** — the page sends `LoadKhronos { name }` or `LoadPolyhaven { slug }`. The worker resolves the URL (the GLB variant, or the HDRI files API), downloads the bytes into an inbox, and a poll system applies them.
- **Drop / open** — the page reads the local file's bytes and transfers them with `DropAsset { kind }`.

A model goes through `import_gltf_from_bytes` → `queue_gltf_load` → `spawn_prefab`; an HDRI through `load_hdr_skybox` with the atmosphere switched to `Hdr`. Loading a new asset despawns the previous model and requests a mesh rebuild.

## Scene, selection, and the inspector

After a load, the worker enumerates the spawned entities (`query_descendants` from the root), computes each node's depth and name, and posts `Scene`. Selection — from a tree click (`Select { id }`) or a GPU pick — is mapped back to a `freecs::Entity` (matching the raw id), recorded in `editor_selection.bounding_volume_selected_entity` so the engine's stencil `OutlinePass` highlights it, and posted as `Selected` with the entity's transform (quaternion converted to Euler degrees). Inspector edits send `SetTransform`, which writes the local transform and marks it dirty.

## Picking and the outline

Clicking the viewport sends `Pick { x, y }` in physical pixels. The worker uses the engine's GPU picking (`gpu_picking.request_pick` → `take_result`), resolves the `entity_id` to an entity, and selects it. The highlight is the engine's built-in selection mask + outline passes, enabled via `debug_draw.selection_outline_enabled` — the same technique the Nightshade editor uses.

## Build pipeline

`just run` builds the worker crate to wasm (`wasm-bindgen` + `wasm-opt`), generates the Tailwind stylesheet, and serves the Leptos bundle with Trunk. Pushing to `main` runs the same steps in [`.github/workflows/deploy.yml`](../.github/workflows/deploy.yml) and publishes `dist/` to GitHub Pages under `--public-url /nightshade-viewer/`.
