set windows-shell := ["powershell.exe"]

export RUST_LOG := "info,wgpu_core=off"
export RUST_BACKTRACE := "1"

[private]
default:
    @just --list

# Build the workspace
build:
    cargo build -r

# Build the smallest possible version of the binary. (Requires `upx` to be installed)
[windows]
build-compact:
    cargo build --profile=release-compact
    just compress-exe

# Build the smallest possible version of the binary. (Requires `upx` to be installed)
[unix]
build-compact:
    cargo build --profile=release-compact
    just compress-app

# Build the app with wgpu + WebGL
build-webgl:
    trunk build --features webgl

# Build the app with wgpu + WebGPU
build-webgpu:
    trunk build --features webgpu

# Compress the final executable with upx
[windows]
compress-exe:
    upx --best --lzma ./target/release-compact/app.exe

# Compress the final executable with upx
[unix]
compress-app:
    upx --best --lzma ./target/release-compact/app

# Check the workspace
check:
    cargo check --all --tests
    cargo fmt --all -- --check

# Show the workspace documentation
docs:
    cargo doc --open -p app

# Fix all automatically resolvable lints with clippy
fix:
    cargo clippy --all --tests --fix

# Autoformat the workspace
format:
    cargo fmt --all

# Install wasm tooling
init-wasm:
  rustup target add wasm32-unknown-unknown
  cargo install --locked trunk

# Lint the workspace
lint:
    cargo clippy --all --tests -- -D warnings

# Run the desktop app in release mode
run:
    cargo run -r

# Serve the app with wgpu + WebGL
run-webgl:
    trunk serve --features webgl

# Serve the app with wgpu + WebGPU
run-webgpu:
    trunk serve --features webgpu --open

# Run the test suite
test:
    cargo test --all -- --nocapture

# Check for unused dependencies with cargo-machete
udeps:
  cargo machete

# Watch for changes and rebuild the app
watch $project="app":
    cargo watch -x 'run -r -p {{project}}'

# Display toolchain versions
@versions:
    rustc --version
    cargo fmt -- --version
    cargo clippy -- --version
