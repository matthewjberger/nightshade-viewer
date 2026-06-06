# nightshade-viewer

A glTF/GLB viewer. The whole [Nightshade](https://github.com/matthewjberger/nightshade) engine runs inside a web worker against an OffscreenCanvas, and a full [Leptos](https://leptos.dev) UI drives it from the main thread. The worker renders through WebGPU off the main thread. The page owns the scene tree, the inspector, the asset browsers, drag and drop, and picking.

Live demo: https://matthewberger.dev/nightshade-viewer/

## How it works

The worker builds Nightshade's renderer straight from the transferred canvas and drives the frame loop through the engine's offscreen driver. It owns the engine world and the networking. Asset indices and bytes are fetched in the worker with ehttp. The page forwards input, renders the panels and browsers from messages the worker streams back, and posts dropped file bytes across as a transferred ArrayBuffer.

The engine does the hard parts. glTF parsing, PBR rendering, IBL, GPU picking, and the selection outline are all Nightshade.

## Workspace

- protocol, the message and data types both sides share, plus the postMessage envelope keys.
- worker, the wasm module inside the web worker. A ViewerWorld (its own freecs world) with systems for camera, loading, picking, scene enumeration, selection, and the asset browsers.
- the root crate, the Leptos UI. Viewport, SceneTree, Inspector, Toolbar, and AssetBrowser components over grouped signal state and a two-directional bridge.

## Quickstart

Tooling is pinned in [mise.toml](mise.toml). Install [mise](https://mise.jdx.dev) and [just](https://github.com/casey/just), then:

```bash
just init
just run
```

`just run` builds the worker, generates the stylesheet, and serves at http://127.0.0.1:8080. The worker compiles the whole engine, so the first build is large and the worker wasm is several megabytes even after wasm-opt. It needs a browser with WebGPU and OffscreenCanvas-in-workers support (Chromium 113+, Firefox 141+).

## License

Dual-licensed under MIT or Apache-2.0, at your option.
