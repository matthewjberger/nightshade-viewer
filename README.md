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

`just run` builds the worker, generates the stylesheet, and opens the viewer in a native webview window. `just run-web` serves the same bundle at http://127.0.0.1:8080 instead; it needs a browser with WebGPU and OffscreenCanvas-in-workers support (Chromium 113+, Firefox 141+). The worker compiles the whole engine, so the first build is large and the worker wasm is several megabytes even after wasm-opt.

## Agent (MCP)

An external MCP agent (such as Claude Code) can drive the viewer with the same access a user has plus structured queries. This is behind a non-default `agent` cargo feature, so `just run` and `just dist` never include it and a deployed build never opens a localhost socket.

Three steps:

1. Serve the viewer with the agent feature on (builds the worker **and** the page with `--features agent`, served at http://127.0.0.1:8080):

   ```bash
   just run-agent
   ```

2. Build the bridge (`host` is its own workspace, so build it by manifest path):

   ```bash
   cargo build --manifest-path host/Cargo.toml
   ```

3. Register the bridge with Claude Code, from this repo root:

   ```bash
   claude mcp add nightshade-viewer -- C:\Users\matth\code\nightshade-viewer\host\target\debug\nightshade-mcp.exe
   ```

   You run this **once**. After that Claude Code spawns and owns the `nightshade-mcp.exe` process itself (over stdio) every time it starts. You never launch the bridge by hand. The bridge binds `ws://127.0.0.1:8787`; the agent-enabled page reconnects automatically, so start order does not matter. To stop it, `claude mcp remove nightshade-viewer`.

The worker and the page must share the same `agent` setting; the `*-agent` recipes handle that. See [docs/agent-mcp.md](docs/agent-mcp.md) for the architecture, the full MCP tool surface, and how to drive it.

## License

Dual-licensed under MIT or Apache-2.0, at your option.
