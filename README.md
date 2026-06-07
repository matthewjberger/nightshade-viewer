# nightshade-viewer

A glTF/GLB viewer. The whole [Nightshade](https://github.com/matthewjberger/nightshade) engine runs inside a web worker against an OffscreenCanvas, and a full [Leptos](https://leptos.dev) UI drives it from the main thread. The worker renders through WebGPU off the main thread. The page owns the scene tree, the inspector, the asset browsers, drag and drop, and picking.

Live demo: https://matthewberger.dev/nightshade-viewer/

![Nightshade Viewer](assets/screenshot.png)

## Workspace

- protocol, the message and data types both sides share, plus the postMessage envelope keys.
- worker, the wasm module inside the web worker. A ViewerWorld (its own freecs world) with systems for camera, loading, picking, scene enumeration, selection, and the asset browsers.
- the root crate, the Leptos UI. Viewport, SceneTree, Inspector, Toolbar, and AssetBrowser components over grouped signal state and a two-directional bridge.
- host, the native `nightshade-mcp` bridge that exposes the viewer to an external MCP agent. Built and used only with the optional `agent` feature (see below).

## Quickstart

Tooling is pinned in [mise.toml](mise.toml). Install [mise](https://mise.jdx.dev) and [just](https://github.com/casey/just), then:

```bash
just init
just run
```

`just run` builds the worker, generates the stylesheet, and serves at http://127.0.0.1:8080. The worker compiles the whole engine, so the first build is large and the worker wasm is several megabytes even after wasm-opt. It needs a browser with WebGPU and OffscreenCanvas-in-workers support (Chromium 113+, Firefox 141+).

## Agent (MCP)

An external MCP agent (such as Claude Code) can drive the viewer with the same access a user has plus structured queries. This is behind a non-default `agent` cargo feature, so `just run` and `just dist` never include it and a deployed build never opens a localhost socket. Build the worker and the page together with the feature, then run the bridge:

```bash
just run-agent                                   # serves page + worker built with --features agent
cargo build --manifest-path host/Cargo.toml      # the nightshade-mcp stdio <-> websocket bridge
```

The worker and the page must share the same `agent` setting; the `*-agent` recipes handle that. See [docs/agent-mcp.md](docs/agent-mcp.md) for the architecture, the MCP tool surface, and how to point Claude Code at it.

## License

Dual-licensed under MIT or Apache-2.0, at your option.
