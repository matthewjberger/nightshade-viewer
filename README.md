# nightshade-viewer

A glTF/GLB viewer that runs the [Nightshade](https://github.com/matthewjberger/nightshade) engine inside a web worker, with a full [Leptos](https://leptos.dev) UI on the main thread. The worker owns an `OffscreenCanvas` and renders the whole engine through WebGPU off the main thread; the page drives a scene tree, an inspector, asset browsers, drag-and-drop, and pixel-perfect picking.

Live demo: https://matthewberger.dev/nightshade-viewer/

This is the editor-shaped sibling of [nightshade-worker-leptos](https://github.com/matthewjberger/nightshade-worker-leptos). Same worker/Leptos split and shared `protocol` crate, scaled up to a real tool.

## Features

- **Load anything** — drag and drop a `.glb`, `.gltf`, or `.hdr` onto the window, or open one from disk.
- **Khronos sample browser** — browse and load the [glTF Sample Assets](https://github.com/KhronosGroup/glTF-Sample-Assets) with thumbnails.
- **Polyhaven environments** — browse and load [Polyhaven](https://polyhaven.com) HDRIs as the skybox and image-based lighting.
- **Scene tree** — the loaded model's hierarchy; click to select.
- **Inspector** — edit the selected entity's translation, rotation, and scale live.
- **Picking + outline** — click a mesh in the viewport to select it, highlighted with the engine's stencil selection outline.
- **Camera** — left-drag to orbit, right-drag to pan, scroll to zoom, Frame to fit.

The engine does the hard parts: glTF parsing, PBR rendering, IBL, GPU picking, and the selection outline are all Nightshade.

## How it works

The worker builds Nightshade's renderer straight from the transferred canvas and drives the frame loop through the engine's offscreen driver (`initialize_offscreen` / `tick_offscreen` / `spawn_animation_frame_loop`). It owns the engine world *and* the networking: asset indices and bytes are fetched in the worker with `ehttp`. The page owns the UI: it forwards input, renders the panels and browsers from messages the worker streams back, and posts dropped file bytes across as a transferred `ArrayBuffer`.

## Workspace

- `protocol` — the message and data types both sides share (scene nodes, entity detail, asset entries), plus the `postMessage` envelope keys.
- `worker` — the wasm module inside the web worker: a `ViewerWorld` (its own `freecs` world) with systems for camera, loading, picking, scene enumeration, selection, and the asset browsers.
- The root crate — the Leptos UI: `Viewport`, `SceneTree`, `Inspector`, `Toolbar`, and `AssetBrowser` components, with grouped signal state and a two-directional `bridge`.

## Quickstart

The worker depends on the published [`nightshade`](https://crates.io/crates/nightshade) crate. Tooling is pinned in [`mise.toml`](mise.toml). Install [mise](https://mise.jdx.dev) and [just](https://github.com/casey/just), then:

```bash
just init        # fetch the pinned toolchain (node, rust, wasm-bindgen, wasm-opt, trunk)
just run         # build the worker, the stylesheet, and serve at http://127.0.0.1:8080
```

Because the worker compiles the full engine, the first build is large and the worker wasm is several megabytes even after `wasm-opt -Oz`. Needs a browser with WebGPU and `OffscreenCanvas`-in-workers support (Chromium 113+, Firefox 141+).

## Credits

The bundled default model and the Khronos browser use the [glTF Sample Assets](https://github.com/KhronosGroup/glTF-Sample-Assets). Environments are from [Polyhaven](https://polyhaven.com) (CC0).

## License

Dual-licensed under MIT or Apache-2.0, at your option. Bundled and fetched assets are under their own licenses, see Credits.
