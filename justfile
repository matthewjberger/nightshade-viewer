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
    wasm-opt -Oz runtime/engine_bg.wasm -o runtime/engine_bg.wasm

# Generates the Tailwind stylesheet from public/input.css
tailwind:
    npx tailwindcss -i public/input.css -o public/styles.css

# Builds the worker, the stylesheet, and the Leptos app bundle
build: worker install tailwind
    trunk build

# Builds the worker and stylesheet, then serves the app at http://127.0.0.1:8080
run: worker install tailwind
    trunk serve

# Serves the already-built app without rebuilding the worker
serve:
    trunk serve

# Produces a production bundle in dist
dist: worker install tailwind
    trunk build --release

# Runs cargo check and a format check across the workspace
check:
    cargo check --workspace --target wasm32-unknown-unknown
    cargo fmt --all -- --check

# Runs clippy across the workspace and denies warnings
lint:
    cargo clippy --workspace --target wasm32-unknown-unknown -- -D warnings

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
