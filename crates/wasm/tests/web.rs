//! WebAssembly integration tests.
//!
//! These tests run in a headless browser using wasm-bindgen-test.
//!
//! Run with: wasm-pack test --headless --chrome crates/wasm

use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

/// Test that the module initializes correctly.
#[wasm_bindgen_test]
fn test_init() {
    // Just verify that we can import the module
    let version = petty_wasm::get_version();
    assert!(!version.is_empty());
}

/// Test creating a PettyPdf builder.
#[wasm_bindgen_test]
fn test_create_builder() {
    let pdf = petty_wasm::PettyPdf::new();
    // Just verify it doesn't panic
    drop(pdf);
}

/// Test creating a font provider.
#[wasm_bindgen_test]
fn test_font_provider() {
    let provider = petty_wasm::WasmFontProvider::new();
    assert!(provider.is_empty());
    assert_eq!(provider.count(), 0);
}

/// Test creating a resource provider.
#[wasm_bindgen_test]
fn test_resource_provider() {
    let provider = petty_wasm::WasmResourceProvider::new();
    assert!(provider.is_empty());
    assert_eq!(provider.count(), 0);
}

/// Test loading built-in fonts.
#[wasm_bindgen_test]
fn test_builtin_fonts() {
    let provider = petty_wasm::WasmFontProvider::new();
    provider
        .load_builtin_fonts()
        .expect("Should load builtin fonts");

    // Should have Liberation Sans (4 variants) + Liberation Mono (1) + aliases (2)
    assert!(provider.count() > 0, "Should have loaded fonts");

    let families = provider.list_families();
    assert!(families.contains(&"Liberation Sans".to_string()));
}

/// Test adding a font from bytes.
#[wasm_bindgen_test]
fn test_add_font_from_bytes() {
    let provider = petty_wasm::WasmFontProvider::new();

    // Use a minimal fake font data for testing (won't actually render correctly)
    let fake_font = vec![0u8; 100];
    provider
        .add_font_from_bytes("TestFont", &fake_font, None, None)
        .expect("Should add font");

    assert_eq!(provider.count(), 1);
}

/// Test adding a resource.
#[wasm_bindgen_test]
fn test_add_resource() {
    let provider = petty_wasm::WasmResourceProvider::new();

    let image_data = vec![0u8; 100];
    provider
        .add_resource("test.png", &image_data)
        .expect("Should add resource");

    assert!(provider.exists("test.png"));
    assert!(!provider.exists("nonexistent.png"));
    assert_eq!(provider.count(), 1);
}

// Note: Full PDF generation tests require a real template and fonts,
// which we'll add in a more comprehensive test suite.
