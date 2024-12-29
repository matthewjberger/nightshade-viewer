#  Hemlock 🌹

Hemlock is a portable graphics engine written in rust, capable of complex interactive visualization of arbitrary data streams 🌃🔭

## Quickstart

### Desktop

```bash
cargo run -r
```

### Web

If you have [trunk](https://trunkrs.dev/) installed,
you can serve the app in a web browser:

```bash
rustup target add wasm32-unknown-unknown
cargo install --locked trunk
trunk serve --features web --open
```

## Justfile

If you have [just](https://github.com/casey/just) installed:

```bash
just run

just init-wasm
just run-web
```

> Run `just` with no arguments to list all commands

## Compact Release

A profile named `release-compact` in the `Cargo.toml` optimizes the build
for space, minimizing the final binary size.

If you have [upx](https://upx.github.io) installed, this can be done with:

```bash
# requires that `upx` is installed
just build-compact
```

The final executable is at `target/release-compact/app` on unix and `target/release-compact/app.exe` on windows.

> The binary size should be significantly smaller than the normal release build executable

## Python Bindings

If you have [maturin]() installed:

```
just build-python
```

Then you can install the generated python wheel with:

```
pip install python_api/target/wheels/hemlock-*.whl --force-reinstall
```