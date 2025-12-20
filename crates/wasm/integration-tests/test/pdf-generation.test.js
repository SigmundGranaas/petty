/**
 * Integration tests for Petty WASM PDF generation.
 *
 * These tests verify that:
 * 1. The WASM module loads correctly
 * 2. PDFs can be generated with actual content
 * 3. The generated PDFs are valid and contain expected text
 *
 * Run with: npm test
 */

import { readFileSync, writeFileSync, mkdirSync } from 'fs';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';
import pdfParse from 'pdf-parse';

// Get current directory
const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const OUTPUT_DIR = join(__dirname, '../output');

// Ensure output directory exists
mkdirSync(OUTPUT_DIR, { recursive: true });

// Import the WASM module
let pettyWasm;
try {
  pettyWasm = await import('petty-wasm');
  console.log('✓ WASM module loaded successfully');
  console.log(`  Version: ${pettyWasm.getVersion()}`);
} catch (error) {
  console.error('✗ Failed to load WASM module:', error.message);
  console.error('  Make sure to run: wasm-pack build crates/wasm --target nodejs --release');
  process.exit(1);
}

const { PettyPdf } = pettyWasm;

// Test utilities
let testsPassed = 0;
let testsFailed = 0;

function assert(condition, message) {
  if (!condition) {
    throw new Error(`Assertion failed: ${message}`);
  }
}

async function runTest(name, testFn) {
  try {
    console.log(`\nRunning: ${name}`);
    await testFn();
    console.log(`✓ PASSED: ${name}`);
    testsPassed++;
  } catch (error) {
    console.error(`✗ FAILED: ${name}`);
    console.error(`  ${error.message}`);
    if (error.stack) {
      console.error(`  ${error.stack.split('\n').slice(1, 3).join('\n')}`);
    }
    testsFailed++;
  }
}

// Test 1: Basic PDF generation with text
await runTest('Generate basic PDF with text', async () => {
  const template = {
    _stylesheet: {
      pageMasters: {
        default: {
          size: 'A4',
          margins: '1cm'
        }
      },
      styles: {
        default: {
          'font-family': 'Liberation Sans',
          'font-size': '12pt'
        },
        title: {
          'font-size': '24pt',
          'font-weight': 'bold',
          'margin-bottom': '1cm'
        }
      }
    },
    _template: {
      type: 'Block',
      children: [
        {
          type: 'Paragraph',
          styleName: 'title',
          children: [
            { type: 'Text', content: 'Hello from Petty WASM!' }
          ]
        },
        {
          type: 'Paragraph',
          children: [
            { type: 'Text', content: 'This PDF was generated using WebAssembly.' }
          ]
        }
      ]
    }
  };

  // Create PDF with builder pattern (method chaining)
  const pdf = new PettyPdf()
    .withBuiltinFonts()
    .withTemplateObject(template);

  // Generate PDF
  const pdfBytes = await pdf.generate({});

  // Verify we got bytes
  assert(pdfBytes instanceof Uint8Array, 'Should return Uint8Array');
  assert(pdfBytes.length > 0, 'PDF should have content');

  // Check PDF magic number
  const header = new TextDecoder().decode(pdfBytes.slice(0, 8));
  assert(header.startsWith('%PDF-'), 'Should be a valid PDF file');

  // Save to file
  const outputPath = join(OUTPUT_DIR, 'basic-test.pdf');
  writeFileSync(outputPath, pdfBytes);
  console.log(`  Saved to: ${outputPath} (${pdfBytes.length} bytes)`);

  // Parse and verify content
  const pdfData = await pdfParse(Buffer.from(pdfBytes));
  assert(pdfData.text.includes('Hello from Petty WASM'), 'PDF should contain title text');
  assert(pdfData.text.includes('WebAssembly'), 'PDF should contain body text');
  assert(pdfData.numpages === 1, 'PDF should have 1 page');

  console.log(`  Pages: ${pdfData.numpages}`);
  console.log(`  Text extracted: "${pdfData.text.trim()}"`);
});

// Test 2: PDF with dynamic data
await runTest('Generate PDF with dynamic data', async () => {
  const template = {
    _stylesheet: {
      pageMasters: {
        default: { size: 'A4', margins: '1cm' }
      }
    },
    _template: {
      type: 'Block',
      children: [
        {
          type: 'Paragraph',
          children: [
            { type: 'Text', content: 'Name: {{name}}' }
          ]
        },
        {
          type: 'Paragraph',
          children: [
            { type: 'Text', content: 'Email: {{email}}' }
          ]
        }
      ]
    }
  };

  const pdf = new PettyPdf()
    .withBuiltinFonts()
    .withTemplateObject(template);

  const data = {
    name: 'John Doe',
    email: 'john@example.com'
  };

  const pdfBytes = await pdf.generate(data);

  const outputPath = join(OUTPUT_DIR, 'dynamic-data-test.pdf');
  writeFileSync(outputPath, pdfBytes);

  const pdfData = await pdfParse(Buffer.from(pdfBytes));
  assert(pdfData.text.includes('John Doe'), 'PDF should contain name');
  assert(pdfData.text.includes('john@example.com'), 'PDF should contain email');

  console.log(`  Saved to: ${outputPath} (${pdfBytes.length} bytes)`);
});

// Test 3: PDF with multiple pages
await runTest('Generate multi-page PDF', async () => {
  const template = {
    _stylesheet: {
      pageMasters: {
        default: { size: 'A4', margins: '1cm' }
      }
    },
    _template: {
      type: 'Block',
      children: [
        {
          type: 'Paragraph',
          children: [{ type: 'Text', content: 'Page 1 content' }]
        },
        { type: 'PageBreak' },
        {
          type: 'Paragraph',
          children: [{ type: 'Text', content: 'Page 2 content' }]
        }
      ]
    }
  };

  const pdf = new PettyPdf()
    .withBuiltinFonts()
    .withTemplateObject(template);

  const pdfBytes = await pdf.generate({});

  const outputPath = join(OUTPUT_DIR, 'multi-page-test.pdf');
  writeFileSync(outputPath, pdfBytes);

  const pdfData = await pdfParse(Buffer.from(pdfBytes));
  assert(pdfData.numpages === 2, 'PDF should have 2 pages');
  assert(pdfData.text.includes('Page 1'), 'PDF should contain page 1 content');
  assert(pdfData.text.includes('Page 2'), 'PDF should contain page 2 content');

  console.log(`  Saved to: ${outputPath} (${pdfData.numpages} pages, ${pdfBytes.length} bytes)`);
});

// Test 4: PDF with array data
await runTest('Generate PDF from array data', async () => {
  const template = {
    _stylesheet: {
      pageMasters: {
        default: { size: 'A4', margins: '1cm' }
      }
    },
    _template: {
      type: 'Block',
      children: [
        {
          type: 'Paragraph',
          children: [{ type: 'Text', content: 'Item: {{name}} - ${{price}}' }]
        }
      ]
    }
  };

  const pdf = new PettyPdf()
    .withBuiltinFonts()
    .withTemplateObject(template);

  const data = [
    { name: 'Apple', price: '1.99' },
    { name: 'Banana', price: '0.99' },
    { name: 'Orange', price: '2.49' }
  ];

  const pdfBytes = await pdf.generate(data);

  const outputPath = join(OUTPUT_DIR, 'array-data-test.pdf');
  writeFileSync(outputPath, pdfBytes);

  const pdfData = await pdfParse(Buffer.from(pdfBytes));
  assert(pdfData.text.includes('Apple'), 'PDF should contain first item');
  assert(pdfData.text.includes('Banana'), 'PDF should contain second item');
  assert(pdfData.text.includes('Orange'), 'PDF should contain third item');

  console.log(`  Saved to: ${outputPath} (${pdfBytes.length} bytes)`);
});

// Test 5: Template from JSON string
await runTest('Generate PDF from JSON string template', async () => {
  const templateJson = JSON.stringify({
    _stylesheet: {
      pageMasters: { default: { size: 'A4' } }
    },
    _template: {
      type: 'Paragraph',
      children: [
        { type: 'Text', content: 'From JSON string template' }
      ]
    }
  });

  const pdf = new PettyPdf()
    .withBuiltinFonts()
    .withTemplateJson(templateJson);

  const pdfBytes = await pdf.generate({});

  const outputPath = join(OUTPUT_DIR, 'json-string-test.pdf');
  writeFileSync(outputPath, pdfBytes);

  const pdfData = await pdfParse(Buffer.from(pdfBytes));
  assert(pdfData.text.includes('JSON string'), 'PDF should contain text from JSON template');

  console.log(`  Saved to: ${outputPath} (${pdfBytes.length} bytes)`);
});

// Test 6: Error handling
await runTest('Handle invalid template gracefully', async () => {
  const pdf = new PettyPdf()
    .withBuiltinFonts();

  let errorThrown = false;
  try {
    // Try to generate without setting a template
    await pdf.generate({});
  } catch (error) {
    errorThrown = true;
    assert(error.message.includes('template') || error.message.includes('No template'),
      'Error should mention missing template');
  }

  assert(errorThrown, 'Should throw error for missing template');
  console.log('  Correctly threw error for missing template');
});

// Test 7: Font provider
await runTest('Font provider operations', async () => {
  const { WasmFontProvider } = pettyWasm;

  const provider = new WasmFontProvider();
  assert(provider.isEmpty(), 'New provider should be empty');
  assert(provider.count === 0, 'New provider should have count 0');

  provider.loadBuiltinFonts();
  assert(!provider.isEmpty(), 'Provider with fonts should not be empty');
  assert(provider.count > 0, 'Provider should have fonts');

  const families = provider.listFamilies();
  assert(Array.isArray(families), 'Should return array of families');
  assert(families.includes('Liberation Sans'), 'Should include Liberation Sans');

  console.log(`  Loaded ${provider.count} fonts`);
  console.log(`  Font families: ${families.join(', ')}`);
});

// Test 8: Resource provider
await runTest('Resource provider operations', async () => {
  const { WasmResourceProvider } = pettyWasm;

  const provider = new WasmResourceProvider();
  assert(provider.isEmpty(), 'New provider should be empty');

  const fakeImageData = new Uint8Array([0x89, 0x50, 0x4E, 0x47]); // PNG header
  provider.addResource('test.png', fakeImageData);

  assert(!provider.isEmpty(), 'Provider with resources should not be empty');
  assert(provider.exists('test.png'), 'Added resource should exist');
  assert(!provider.exists('nonexistent.png'), 'Non-existent resource should not exist');
  assert(provider.count === 1, 'Should have 1 resource');

  provider.remove('test.png');
  assert(provider.isEmpty(), 'Provider should be empty after removing resource');

  console.log('  Resource operations working correctly');
});

// Print summary
console.log('\n' + '='.repeat(60));
console.log(`Tests completed: ${testsPassed + testsFailed} total`);
console.log(`  ✓ Passed: ${testsPassed}`);
console.log(`  ✗ Failed: ${testsFailed}`);
console.log('='.repeat(60));

if (testsFailed > 0) {
  process.exit(1);
}
