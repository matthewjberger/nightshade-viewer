set windows-shell := ["powershell.exe"]
export RUST_BACKTRACE := "1"

# Displays the list of available commands
@just:
    just --list

# Installs the tools pinned in mise.toml (node, rust, wasm-bindgen, wasm-opt, trunk)
init:
    mise install

# Installs the web dependencies (tailwindcss)
install:
    npm install

# Builds the worker crate to wasm and generates web bindings into runtime/
worker:
    cargo build --release -p worker --target wasm32-unknown-unknown
    wasm-bindgen --target web --out-dir runtime --out-name engine target/wasm32-unknown-unknown/release/worker.wasm
    wasm-opt -O3 --enable-simd runtime/engine_bg.wasm -o runtime/engine_bg.wasm

# Builds the worker with the external-agent feature (MCP-driveable) into runtime/
worker-agent:
    cargo build --release -p worker --target wasm32-unknown-unknown --features agent
    wasm-bindgen --target web --out-dir runtime --out-name engine target/wasm32-unknown-unknown/release/worker.wasm
    wasm-opt -O3 --enable-simd runtime/engine_bg.wasm -o runtime/engine_bg.wasm

# Generates the Tailwind stylesheet from public/input.css
tailwind:
    npx tailwindcss -i public/input.css -o public/styles.css

# Builds the worker, the stylesheet, and the Leptos app bundle
build: worker install tailwind
    trunk build

# Builds the web bundle and opens the viewer in a native webview window
run: build
    cargo run -p desktop

# Builds the worker and stylesheet, then serves the app at http://127.0.0.1:8080
run-web: worker install tailwind
    trunk serve

# Like `run`, but builds the worker, the app, and the desktop shell with the
# external-agent feature, so an MCP client can drive the viewer over
# http://127.0.0.1:8788/mcp. See docs/agent-mcp.md.
run-agent: worker-agent install tailwind
    trunk build --features agent
    cargo run -p desktop --features agent

# Serves the already-built app without rebuilding the worker
serve:
    trunk serve

# Produces a production bundle in dist
dist: worker install tailwind
    trunk build --release

# Builds the standalone viewer executable with the web bundle embedded
build-desktop: dist
    cargo build --release -p desktop

# Runs cargo check and a format check across the workspace
check:
    cargo check -p protocol -p worker -p nightshade-viewer --target wasm32-unknown-unknown
    cargo check -p desktop
    cargo check -p desktop --features agent
    cargo fmt --all -- --check

# Runs clippy across the workspace and denies warnings
lint:
    cargo clippy -p protocol -p worker -p nightshade-viewer --target wasm32-unknown-unknown -- -D warnings
    cargo clippy -p desktop -- -D warnings
    cargo clippy -p desktop --features agent -- -D warnings

# Formats the code
format:
    cargo fmt --all

# Removes build artifacts (Windows)
[windows]
clean:
    cargo clean
    Remove-Item -Recurse -Force dist, node_modules, public/styles.css, runtime/engine.js, runtime/engine_bg.wasm, runtime/engine.d.ts -ErrorAction SilentlyContinue

# Removes build artifacts (Unix)
[unix]
clean:
    cargo clean
    rm -rf dist node_modules public/styles.css runtime/engine.js runtime/engine_bg.wasm runtime/engine.d.ts
