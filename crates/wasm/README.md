# Petty WASM

WebAssembly bindings for Petty PDF generation library.

## WASM-Specific Implementation Details

### Time Measurement

The layout engine uses the `instant` crate for time measurement instead of `std::time::Instant` because the standard library's time APIs are not available in the `wasm32-unknown-unknown` target.

However, profiling is **disabled by default** in WASM builds. The `instant` crate is only included when the `profiling` feature is enabled on the `petty-layout` crate. This keeps the WASM bundle size smaller and eliminates unnecessary overhead.

### Random Number Generation

The crate uses `getrandom` with the `wasm_js` feature to provide WASM-compatible random number generation for PDF object IDs.

### Rendering Backend

The WASM build uses `LopdfRenderer` for PDF generation. The renderer writes to an in-memory buffer (`Cursor<Vec<u8>>`) which is then returned as a JavaScript `Uint8Array`.

## Building

Build the WASM package using wasm-pack:

```bash
# Development build
wasm-pack build crates/wasm --dev --target bundler

# Production build
wasm-pack build crates/wasm --release --target bundler
```

## Testing

Run tests in headless browsers:

```bash
# Chrome
wasm-pack test --headless --chrome crates/wasm

# Firefox
wasm-pack test --headless --firefox crates/wasm

# All browsers
wasm-pack test --headless crates/wasm
```

### Local Integration Tests

For more comprehensive integration tests with Node.js:

```bash
cd crates/wasm/integration-tests
npm install
npm test
```

## Browser Compatibility

This WASM module requires:
- WebAssembly support (all modern browsers)
- JavaScript ES6+ features (for wasm-bindgen glue code)

Tested on:
- Chrome 90+
- Firefox 88+
- Safari 14+
- Edge 90+

## Known Limitations

### No System Fonts

The WASM build does not support system font discovery. You must either:
1. Use embedded fonts (if compiled with the `embedded-fonts` feature)
2. Load fonts from URLs using the builder API
3. Provide font data as `Uint8Array`

### No Async I/O

Unlike the native Rust API, this WASM API does not support async I/O operations during template execution. All data must be prepared before calling `generate()`.

### Memory Constraints

WASM has a 4GB memory limit. Very large documents (thousands of pages) may hit memory constraints in the browser.

## File Structure

```
crates/wasm/
├── src/
│   ├── lib.rs              # Main wasm_bindgen exports
│   ├── builder.rs          # PettyPdf builder (JS API)
│   ├── fonts.rs            # Font loading utilities
│   ├── resources.rs        # Resource provider
│   ├── error.rs            # Error handling
│   ├── pipeline.rs         # Synchronous pipeline
│   └── utils.rs            # Utilities (panic hook, etc)
├── tests/
│   └── web.rs              # Browser-based integration tests
└── integration-tests/      # Node.js integration tests
    ├── test-*.mjs
    └── package.json
```

## Profiling

To enable performance profiling in WASM builds, add the `profiling` feature to `petty-layout`:

```toml
[dependencies]
petty-layout = { version = "0.1", features = ["profiling"] }
```

Note: This will increase bundle size and add runtime overhead. Only use for debugging.
