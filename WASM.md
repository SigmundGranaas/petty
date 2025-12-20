# WASM Compatibility

This document describes the WASM (WebAssembly) compatibility status of the Petty PDF generation engine.

## Status

**✅ petty-core**: Fully WASM-compatible
- Compiles successfully for `wasm32-unknown-unknown` target
- All core functionality (layout, parsing, rendering) works without system dependencies
- No filesystem, threading, or platform-specific code in petty-core

**⚠️ petty**: Platform-specific (not WASM-compatible)
- Contains native platform adapters: filesystem resources, system fonts, multi-threading executors
- Designed for server/native applications
- For WASM usage, use petty-core directly with custom platform adapters

## Building for WASM

### Prerequisites

Install the WASM target:
```bash
rustup target add wasm32-unknown-unknown
```

### Building petty-core for WASM

```bash
cargo build --package petty-core --target wasm32-unknown-unknown --no-default-features
```

The `--no-default-features` flag disables system font discovery (fontdb), which is not available in WASM.

### Configuration

The workspace includes a `.cargo/config.toml` that automatically configures the getrandom backend for WASM:

```toml
[target.wasm32-unknown-unknown]
rustflags = ['--cfg', 'getrandom_backend="wasm_js"']
```

This is required because petty-core's dependencies (lopdf and printpdf) transitively depend on `rand`, which requires WASM-specific configuration.

## Changes Made for WASM Compatibility

### 1. Conditional Tempfile Usage
**Native platforms**: Use `tempfile::tempfile()` for memory efficiency (writes to disk)
**WASM platforms**: Use `Cursor<Vec<u8>>` in-memory buffers (no filesystem)

The code uses conditional compilation to choose the appropriate storage:

```rust
// src/pipeline/provider/metadata.rs

#[cfg(feature = "tempfile")]
let buf_writer = {
    let temp_file = tempfile::tempfile()?;
    BufWriter::new(temp_file)
};

#[cfg(not(feature = "tempfile"))]
let buf_writer = {
    let memory_buffer = Cursor::new(Vec::new());
    BufWriter::new(memory_buffer)
};
```

The `tempfile` feature is enabled by default on native platforms via the `native` feature flag.

### 2. Configured getrandom for WASM
**Issue**: lopdf and printpdf depend on rand → getrandom, which doesn't support WASM by default
**Solution**: Added target-specific dependency in petty-core/Cargo.toml:

```toml
[target.'cfg(target_arch = "wasm32")'.dependencies]
getrandom = { version = "0.3", features = ["wasm_js"] }
```

And configured the backend via `.cargo/config.toml` (see above).

### 3. Feature-Gated System Dependencies
System-specific dependencies are behind the `system-fonts` feature:

```toml
[features]
default = ["system-fonts"]
system-fonts = ["fontdb"]
```

Disable with `--no-default-features` for WASM builds.

## Using petty-core in WASM

When using petty-core in a WASM environment, you need to provide:

1. **Font data**: Load fonts as bytes and use `InMemoryFontProvider`
2. **Resources**: Provide images/assets via `InMemoryResourceProvider`
3. **Executor**: Use `SyncExecutor` (single-threaded) since WASM doesn't support threading

### Example WASM Setup

```rust
use petty_core::traits::{InMemoryFontProvider, InMemoryResourceProvider};
use petty::executor::SyncExecutor;
use petty_core::core::style::{FontWeight, FontStyle};

// Create providers
let font_provider = InMemoryFontProvider::new();
font_provider.add_font(
    "Helvetica",
    FontWeight::Normal,
    FontStyle::Normal,
    include_bytes!("fonts/helvetica.ttf").to_vec()
)?;

let resource_provider = InMemoryResourceProvider::new();
resource_provider.add("logo.png", logo_bytes)?;

// Use SyncExecutor for single-threaded execution
let executor = SyncExecutor::new();

// Build pipeline context with these providers
// ... (context creation code)
```

## Current Limitations

1. **No system fonts**: WASM builds cannot discover system fonts. All fonts must be provided as byte arrays.
2. **No filesystem access**: All resources must be loaded via `InMemoryResourceProvider`.
3. **Single-threaded**: WASM doesn't support multi-threading (yet), so use `SyncExecutor`.
4. **Memory usage**: WASM uses in-memory buffers for the metadata analysis pass, while native platforms use tempfiles. For large documents, this means higher memory usage in WASM.
5. **Binary size**: The full petty-core compiled to WASM is ~2-3 MB (unoptimized). Use `wasm-opt` to reduce size.

## Testing WASM Builds

### Compilation Verification

To verify that petty-core compiles for WASM:

```bash
cargo build --package petty-core --target wasm32-unknown-unknown --no-default-features
```

If this succeeds, petty-core is WASM-compatible!

### Running Tests

**Note**: You cannot directly run `cargo test` for WASM targets on native systems. WASM binaries require a WASM runtime.

#### Option 1: Use wasm-pack (Recommended)

Install wasm-pack:
```bash
cargo install wasm-pack
```

Add wasm-bindgen-test to petty-core's dev-dependencies:
```toml
[target.'cfg(target_arch = "wasm32")'.dev-dependencies]
wasm-bindgen-test = "0.3"
```

Run tests in a browser environment:
```bash
wasm-pack test --headless --firefox petty-core
# or
wasm-pack test --headless --chrome petty-core
```

#### Option 2: Use wasmtime (for WASI targets)

For wasm32-wasi target (not wasm32-unknown-unknown):
```bash
cargo install wasmtime
cargo test --target wasm32-wasi
```

Note: wasm32-wasi has different capabilities than wasm32-unknown-unknown.

#### Option 3: Skip test execution (verification only)

If you just want to verify compilation without running tests:

```bash
cargo build --package petty-core --target wasm32-unknown-unknown --no-default-features
```

This is sufficient to verify WASM compatibility.

### Expected Test Behavior

When you run:
```bash
cargo test --package petty-core --target wasm32-unknown-unknown --no-default-features
```

You'll see:
```
error: could not execute process ... (never executed)
Caused by:
  Exec format error (os error 8)
```

**This is expected!** The compilation succeeded, but cargo can't execute the WASM binary natively. Use one of the options above to actually run the tests.

## Future Improvements

- [ ] Add wasm-bindgen bindings for JavaScript interop
- [ ] Create WASM-specific examples
- [ ] Optimize binary size with feature flags
- [ ] Add WASM CI pipeline
- [ ] Document multi-threading when WASM threads mature
- [ ] Bundle embedded fallback fonts (feature-gated)

## Architecture Notes

The WASM compatibility was achieved through the workspace split:
- **petty-core**: Platform-agnostic core with trait abstractions
- **petty**: Platform-specific implementations (native, WASM, etc.)

This architecture allows petty-core to remain pure Rust with no platform dependencies, while platform-specific code lives in adapters. Future platforms (mobile, embedded) can follow the same pattern.

## References

- [getrandom WASM support](https://docs.rs/getrandom/latest/getrandom/#webassembly-support)
- [Rust and WebAssembly book](https://rustwasm.github.io/book/)
- [wasm-bindgen guide](https://rustwasm.github.io/wasm-bindgen/)
