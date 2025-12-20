# Petty WASM Integration Tests

Integration tests for the Petty WASM PDF generation module.

## Overview

These tests verify that:
- The WASM module loads correctly in Node.js
- PDFs can be generated with actual content
- The generated PDFs are valid and contain expected text
- Font and resource providers work correctly
- Error handling behaves as expected

## Running Tests Locally

### Prerequisites

1. **Node.js 20+** - Required for running tests
2. **wasm-pack** - Required for building the WASM module

   ```bash
   curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
   ```

3. **Rust with wasm32-unknown-unknown target**

   ```bash
   rustup target add wasm32-unknown-unknown
   ```

### Build and Test

From the repository root:

```bash
# 1. Build the WASM module for Node.js
wasm-pack build crates/wasm --target nodejs --release

# 2. Install test dependencies
cd crates/wasm/integration-tests
npm install

# 3. Run the tests
npm test
```

### Test Output

Tests will:
- Print results to the console
- Save generated PDFs to `output/` directory
- Exit with code 0 on success, 1 on failure

Example output:
```
✓ WASM module loaded successfully
  Version: 0.1.0

Running: Generate basic PDF with text
  Saved to: output/basic-test.pdf (15234 bytes)
  Pages: 1
  Text extracted: "Hello from Petty WASM! This PDF was generated using WebAssembly."
✓ PASSED: Generate basic PDF with text

Running: Generate PDF with dynamic data
  Saved to: output/dynamic-data-test.pdf (12456 bytes)
✓ PASSED: Generate PDF with dynamic data

...

============================================================
Tests completed: 8 total
  ✓ Passed: 8
  ✗ Failed: 0
============================================================
```

## Test Files

- `test/pdf-generation.test.js` - Main integration tests
- `package.json` - Test dependencies
- `output/` - Generated PDFs (git-ignored)

## Continuous Integration

Tests run automatically in GitHub Actions on:
- Push to main/master/js-wasm branches
- Pull requests to main/master
- Changes to `crates/wasm/**`

The CI workflow:
1. Builds the WASM module
2. Runs unit tests in headless Chrome
3. Runs integration tests in Node.js
4. Uploads generated PDFs as artifacts
5. Reports bundle size

## Troubleshooting

### "Cannot find module 'petty-wasm'"

Make sure you've built the WASM module first:
```bash
wasm-pack build crates/wasm --target nodejs --release
```

### "pdf-parse" errors

Install dependencies:
```bash
npm install
```

### Tests fail but PDFs are generated

Check the `output/` directory to inspect the generated PDFs manually.

## Adding New Tests

To add a new test, add a `runTest()` call in `test/pdf-generation.test.js`:

```javascript
await runTest('My new test', async () => {
  const pdf = new PettyPdf();
  pdf.withBuiltinFonts();
  pdf.withTemplateObject({
    _stylesheet: { pageMasters: { default: { size: 'A4' } } },
    _template: { type: 'Paragraph', children: [{ type: 'Text', content: 'Test' }] }
  });

  const pdfBytes = await pdf.generate({});

  // Add assertions
  assert(pdfBytes.length > 0, 'Should generate PDF');

  // Optionally save and parse
  writeFileSync(join(OUTPUT_DIR, 'my-test.pdf'), pdfBytes);
  const pdfData = await pdfParse(Buffer.from(pdfBytes));
  assert(pdfData.text.includes('Test'), 'Should contain expected text');
});
```
