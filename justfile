set windows-shell := ["powershell.exe"]

export RUST_LOG := "info,wgpu_core=off"
export RUST_BACKTRACE := "1"

[private]
default:
    @just --list

# Build the desktop app
build:
    cargo build -r

# Build the desktop app with the smallest possible filesize. (Requires `upx` to be installed)
[windows]
build-compact:
    cargo build --profile=release-compact
    just compress-exe

# Build the desktop app with the smallest possible filesize. (Requires `upx` to be installed)
[unix]
build-compact:
    cargo build --profile=release-compact
    just compress-app

# Build the python bindings
@build-python:
    maturin build -m python_api/Cargo.toml
    echo '{{ style("warning") }}Install the python bindings wheel with:{{ NORMAL }}'
    echo ''
    echo '{{ style("warning") }}pip install python_api/target/wheels/hemlock-*.whl --force-reinstall{{ NORMAL }}'
    echo ''
    echo '{{ style("warning") }}Remember to expand the `*` to use the full path to the wheel!{{ NORMAL }}'

# Build the app as a static site
build-web:
    trunk build --features webgpu

# Compress the final executable with upx
[windows]
compress-exe:
    upx --best --lzma ./target/release-compact/hemlock.exe

# Compress the final executable with upx
[unix]
compress-app:
    upx --best --lzma ./target/release-compact/hemlock

# Check the workspace
check:
    cargo check --all --tests
    cargo fmt --all -- --check

# Show the workspace documentation
docs:
    cargo doc --open -p hemlock

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

# Serve the app with wgpu + WebGPU
run-web:
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
