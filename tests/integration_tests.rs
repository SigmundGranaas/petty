use lopdf::Document as LopdfDocument;
use petty::PipelineBuilder;
use serde_json::json;
use std::collections::BTreeMap;
use std::io::Cursor;

/// Helper function to extract text content from a PDF
fn extract_text_from_pdf(pdf_bytes: &[u8]) -> Result<String, Box<dyn std::error::Error>> {
    let doc = LopdfDocument::load_mem(pdf_bytes)?;
    let mut text = String::new();

    let pages = doc.get_pages();
    for page_num in 1..=pages.len() {
        // Try to extract text using lopdf's built-in method
        match doc.extract_text(&[page_num as u32]) {
            Ok(page_text) => {
                text.push_str(&page_text);
                text.push('\n');
            }
            Err(e) => {
                eprintln!(
                    "Warning: Could not extract text from page {}: {}",
                    page_num, e
                );
            }
        }
    }

    Ok(text)
}

/// Helper function to check if fonts are embedded in the PDF
fn check_font_embedding(pdf_bytes: &[u8]) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let doc = LopdfDocument::load_mem(pdf_bytes)?;
    let mut embedded_fonts = Vec::new();

    // Iterate through all objects to find font dictionaries
    for (_, object) in doc.objects.iter() {
        if let Ok(dict) = object.as_dict()
            && let Ok(type_val) = dict.get(b"Type")
            && let Ok(type_name) = type_val.as_name()
            && type_name == b"Font"
            && let Ok(base_font) = dict.get(b"BaseFont")
            && let Ok(font_name) = base_font.as_name()
        {
            embedded_fonts.push(String::from_utf8_lossy(font_name).to_string());
        }
    }

    Ok(embedded_fonts)
}

/// Helper to get font descriptors from PDF
fn get_font_descriptors(
    pdf_bytes: &[u8],
) -> Result<BTreeMap<String, BTreeMap<String, String>>, Box<dyn std::error::Error>> {
    let doc = LopdfDocument::load_mem(pdf_bytes)?;
    let mut font_info = BTreeMap::new();

    for (obj_id, object) in doc.objects.iter() {
        if let Ok(dict) = object.as_dict()
            && let Ok(type_val) = dict.get(b"Type")
            && let Ok(type_name) = type_val.as_name()
            && type_name == b"Font"
        {
            let mut info = BTreeMap::new();

            // Get subtype
            if let Ok(subtype_val) = dict.get(b"Subtype")
                && let Ok(subtype_name) = subtype_val.as_name()
            {
                info.insert(
                    "Subtype".to_string(),
                    String::from_utf8_lossy(subtype_name).to_string(),
                );
            }

            // Get base font name
            if let Ok(base_font_val) = dict.get(b"BaseFont")
                && let Ok(base_font_name) = base_font_val.as_name()
            {
                info.insert(
                    "BaseFont".to_string(),
                    String::from_utf8_lossy(base_font_name).to_string(),
                );
            }

            // Get encoding
            if let Ok(encoding) = dict.get(b"Encoding") {
                info.insert("Encoding".to_string(), format!("{:?}", encoding));
            }

            // Check for font descriptor
            if let Ok(font_desc_ref) = dict.get(b"FontDescriptor") {
                info.insert("HasDescriptor".to_string(), "true".to_string());

                if let Ok(desc_ref_id) = font_desc_ref.as_reference()
                    && let Ok(desc_obj) = doc.get_object(desc_ref_id)
                    && let Ok(desc_dict) = desc_obj.as_dict()
                    && (desc_dict.has(b"FontFile")
                        || desc_dict.has(b"FontFile2")
                        || desc_dict.has(b"FontFile3"))
                {
                    info.insert("EmbeddedFont".to_string(), "true".to_string());
                }
            }

            font_info.insert(format!("{:?}", obj_id), info);
        }
    }

    Ok(font_info)
}

/// Helper to extract link annotations from PDF pages
fn extract_link_annotations(
    pdf_bytes: &[u8],
) -> Result<Vec<BTreeMap<String, String>>, Box<dyn std::error::Error>> {
    let doc = LopdfDocument::load_mem(pdf_bytes)?;
    let mut annotations = Vec::new();

    let pages = doc.get_pages();
    for (_page_num, page_id) in pages.iter() {
        if let Ok(page_obj) = doc.get_object(*page_id)
            && let Ok(page_dict) = page_obj.as_dict()
            && let Ok(annots_ref) = page_dict.get(b"Annots")
        {
            // Try to resolve as array
            let annots_array = if let Ok(arr) = annots_ref.as_array() {
                arr.clone()
            } else if let Ok(ref_id) = annots_ref.as_reference() {
                if let Ok(annots_obj) = doc.get_object(ref_id)
                    && let Ok(arr) = annots_obj.as_array()
                {
                    arr.clone()
                } else {
                    continue;
                }
            } else {
                continue;
            };

            // Iterate through annotations
            for annot_ref in annots_array {
                if let Ok(annot_id) = annot_ref.as_reference()
                    && let Ok(annot_obj) = doc.get_object(annot_id)
                    && let Ok(annot_dict) = annot_obj.as_dict()
                {
                    let mut info = BTreeMap::new();

                    // Get annotation type
                    if let Ok(subtype) = annot_dict.get(b"Subtype")
                        && let Ok(subtype_name) = subtype.as_name()
                    {
                        info.insert(
                            "Subtype".to_string(),
                            String::from_utf8_lossy(subtype_name).to_string(),
                        );
                    }

                    // Get destination (for Link annotations)
                    if let Ok(dest) = annot_dict.get(b"Dest") {
                        info.insert("Dest".to_string(), format!("{:?}", dest));
                    }

                    // Get action (alternative to Dest)
                    if let Ok(action) = annot_dict.get(b"A") {
                        info.insert("Action".to_string(), format!("{:?}", action));
                    }

                    // Get rect (position)
                    if let Ok(rect) = annot_dict.get(b"Rect") {
                        info.insert("Rect".to_string(), format!("{:?}", rect));
                    }

                    if !info.is_empty() {
                        annotations.push(info);
                    }
                }
            }
        }
    }

    Ok(annotations)
}

/// Helper to count internal links/anchors in PDF
fn count_internal_links(pdf_bytes: &[u8]) -> Result<usize, Box<dyn std::error::Error>> {
    let annotations = extract_link_annotations(pdf_bytes)?;
    let link_count = annotations
        .iter()
        .filter(|a| a.get("Subtype").map(|s| s.as_str()) == Some("Link"))
        .count();
    Ok(link_count)
}

#[test]
fn test_simple_text_rendering() -> Result<(), Box<dyn std::error::Error>> {
    let _ = env_logger::builder().is_test(true).try_init();

    // Create a simple JSON template with text
    let template = json!({
        "_stylesheet": {
            "defaultPageMaster": "default",
            "pageMasters": {
                "default": {
                    "size": "A4",
                    "margins": "2cm"
                }
            },
            "styles": {
                "default": {
                    "font-family": "Helvetica",
                    "font-size": "12pt",
                    "color": "#000000"
                }
            }
        },
        "_template": {
            "type": "Block",
            "children": [
                {
                    "type": "Paragraph",
                    "children": [
                        {
                            "type": "Text",
                            "content": "Hello World! This is a test."
                        }
                    ]
                },
                {
                    "type": "Paragraph",
                    "children": [
                        {
                            "type": "Text",
                            "content": "Second paragraph with more text."
                        }
                    ]
                }
            ]
        }
    });

    let template_str = serde_json::to_string(&template)?;

    // Build pipeline
    let pipeline = PipelineBuilder::new()
        .with_template_source(&template_str, "json")?
        .build()?;

    // Generate PDF
    let data = vec![json!({})];
    let writer = Cursor::new(Vec::new());

    let result = tokio::runtime::Runtime::new()?
        .block_on(async { pipeline.generate(data.into_iter(), writer).await })?;

    let pdf_bytes = result.into_inner();

    // Verify PDF was created and has content
    assert!(
        pdf_bytes.len() > 100,
        "PDF should have substantial content, got {} bytes",
        pdf_bytes.len()
    );

    // Check that it's a valid PDF
    let doc = LopdfDocument::load_mem(&pdf_bytes)?;
    let pages = doc.get_pages();
    assert_eq!(pages.len(), 1, "Should have 1 page");

    // Extract and verify text content
    let extracted_text = extract_text_from_pdf(&pdf_bytes)?;
    println!("Extracted text: {:?}", extracted_text);
    println!("Extracted text length: {}", extracted_text.len());

    // Save PDF for manual inspection
    std::fs::write("test_simple_output.pdf", &pdf_bytes)?;
    println!("Saved PDF to test_simple_output.pdf for inspection");

    // Verify text content is present and correct
    assert!(
        extracted_text.contains("Hello World"),
        "PDF should contain 'Hello World', but got: {:?}",
        extracted_text
    );
    assert!(
        extracted_text.contains("test"),
        "PDF should contain 'test', but got: {:?}",
        extracted_text
    );
    assert!(
        extracted_text.contains("Second paragraph"),
        "PDF should contain 'Second paragraph', but got: {:?}",
        extracted_text
    );

    // Check font embedding (informational only, not critical if detection fails)
    let fonts = check_font_embedding(&pdf_bytes)?;
    println!("Fonts detected in PDF: {:?}", fonts);

    // Get detailed font information
    let font_descriptors = get_font_descriptors(&pdf_bytes)?;
    println!("Font descriptors: {:#?}", font_descriptors);

    Ok(())
}

#[test]
fn test_styled_text_rendering() -> Result<(), Box<dyn std::error::Error>> {
    let _ = env_logger::builder().is_test(true).try_init();

    let template = json!({
        "_stylesheet": {
            "defaultPageMaster": "default",
            "pageMasters": {
                "default": {
                    "size": "A4",
                    "margins": "2cm"
                }
            },
            "styles": {
                "default": {
                    "font-family": "Helvetica",
                    "font-size": "12pt"
                },
                "heading": {
                    "font-family": "Helvetica",
                    "font-size": "18pt",
                    "font-weight": "bold",
                    "color": "#0066cc"
                }
            }
        },
        "_template": {
            "type": "Block",
            "children": [
                {
                    "type": "Paragraph",
                    "style": "heading",
                    "children": [
                        {
                            "type": "Text",
                            "content": "Main Heading"
                        }
                    ]
                },
                {
                    "type": "Paragraph",
                    "children": [
                        {
                            "type": "Text",
                            "content": "Regular paragraph text. "
                        },
                        {
                            "type": "StyledSpan",
                            "style": {
                                "font-weight": "bold"
                            },
                            "children": [
                                {
                                    "type": "Text",
                                    "content": "Bold text"
                                }
                            ]
                        },
                        {
                            "type": "Text",
                            "content": " and "
                        },
                        {
                            "type": "StyledSpan",
                            "style": {
                                "font-style": "italic"
                            },
                            "children": [
                                {
                                    "type": "Text",
                                    "content": "italic text"
                                }
                            ]
                        },
                        {
                            "type": "Text",
                            "content": "."
                        }
                    ]
                }
            ]
        }
    });

    let template_str = serde_json::to_string(&template)?;

    let pipeline = PipelineBuilder::new()
        .with_template_source(&template_str, "json")?
        .with_generation_mode(petty::GenerationMode::ForceStreaming)
        .build()?;

    let data = vec![json!({})];
    let writer = Cursor::new(Vec::new());

    let result = tokio::runtime::Runtime::new()?
        .block_on(async { pipeline.generate(data.into_iter(), writer).await })?;

    let pdf_bytes = result.into_inner();

    // Verify content
    assert!(pdf_bytes.len() > 100);

    let extracted_text = extract_text_from_pdf(&pdf_bytes)?;
    println!("Extracted styled text: {:?}", extracted_text);

    // Check for text content
    assert!(
        extracted_text.contains("Main Heading") || extracted_text.contains("Heading"),
        "Should contain heading text, got: {:?}",
        extracted_text
    );

    assert!(
        extracted_text.contains("Regular paragraph")
            || extracted_text.contains("paragraph")
            || extracted_text.contains("Bold"),
        "Should contain paragraph text, got: {:?}",
        extracted_text
    );

    // Check fonts - should have multiple font variants for bold/italic
    let fonts = check_font_embedding(&pdf_bytes)?;
    println!("Fonts for styled text: {:?}", fonts);

    let font_descriptors = get_font_descriptors(&pdf_bytes)?;
    println!("Styled text font descriptors: {:#?}", font_descriptors);

    Ok(())
}

#[test]
fn test_xslt_template_rendering() -> Result<(), Box<dyn std::error::Error>> {
    let _ = env_logger::builder().is_test(true).try_init();

    // Create a simple XSLT template with correct page master configuration
    let xslt_template = r#"<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:fo="http://www.w3.org/1999/XSL/Format">

    <fo:simple-page-master page-width="210mm" page-height="297mm" margin="2cm"/>

    <xsl:attribute-set name="title">
        <xsl:attribute name="font-family">Helvetica</xsl:attribute>
        <xsl:attribute name="font-size">20pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
    </xsl:attribute-set>

    <xsl:template match="/">
        <fo:block>
            <fo:block xsl:use-attribute-sets="title">
                <xsl:value-of select="document/title"/>
            </fo:block>
            <fo:block>
                <xsl:value-of select="document/content"/>
            </fo:block>
        </fo:block>
    </xsl:template>
</xsl:stylesheet>"#;

    let pipeline = PipelineBuilder::new()
        .with_template_source(xslt_template, "xslt")?
        .build()?;

    // Pass data as JSON - the XSLT processor will convert it to XML structure
    let data = vec![json!({
        "document": {
            "title": "Test Document",
            "content": "This is the content of the test document."
        }
    })];
    let writer = Cursor::new(Vec::new());

    let result = tokio::runtime::Runtime::new()?
        .block_on(async { pipeline.generate(data.into_iter(), writer).await })?;

    let pdf_bytes = result.into_inner();

    assert!(pdf_bytes.len() > 100);

    let extracted_text = extract_text_from_pdf(&pdf_bytes)?;
    println!("Extracted XSLT text: {:?}", extracted_text);

    assert!(
        extracted_text.contains("Test Document") || extracted_text.contains("Document"),
        "Should contain title, got: {:?}",
        extracted_text
    );

    assert!(
        extracted_text.contains("content") || extracted_text.contains("test document"),
        "Should contain content, got: {:?}",
        extracted_text
    );

    Ok(())
}

#[test]
fn test_toc_with_role_template() -> Result<(), Box<dyn std::error::Error>> {
    let _ = env_logger::builder().is_test(true).try_init();

    // Create an XSLT template with TOC role template and anchors
    let xslt_template = r#"<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet version="1.0"
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:fo="http://www.w3.org/1999/XSL/Format"
                xmlns:petty="http://petty.rs/xsl/extensions">

    <fo:simple-page-master page-width="210mm" page-height="297mm" margin="2cm"/>

    <!-- Define a role template for TOC generation -->
    <xsl:template match="/" petty:role="table-of-contents">
        <fo:block font-size="18pt" font-weight="bold" margin-bottom="1cm">
            <fo:text>Table of Contents</fo:text>
        </fo:block>
        <xsl:for-each select="*/headings/item">
            <fo:block margin-left="1cm" margin-bottom="0.5cm">
                <fo:link destination="{id}">
                    <fo:text>• </fo:text>
                    <xsl:value-of select="text"/>
                    <fo:text> (Page </fo:text>
                    <xsl:value-of select="pageNumber"/>
                    <fo:text>)</fo:text>
                </fo:link>
            </fo:block>
        </xsl:for-each>
    </xsl:template>

    <!-- Main content template -->
    <xsl:template match="/">
        <toc/>
        <xsl:for-each select="document/section/item">
            <h2 id="{generate-id(.)}">
                <xsl:value-of select="@title"/>
            </h2>
            <fo:block>
                <xsl:value-of select="."/>
            </fo:block>
        </xsl:for-each>
    </xsl:template>

</xsl:stylesheet>"#;

    // Auto mode will detect TOC role template and use composing renderer
    let pipeline = PipelineBuilder::new()
        .with_template_source(xslt_template, "xslt")?
        .build()?;

    // Data with multiple sections
    let data = vec![json!({
        "document": {
            "section": [
                {
                    "@title": "Introduction",
                    "#text": "This is the introduction section with some content."
                },
                {
                    "@title": "Background",
                    "#text": "This section covers the background information."
                },
                {
                    "@title": "Methodology",
                    "#text": "Here we describe our methodology."
                },
                {
                    "@title": "Results",
                    "#text": "The results are presented in this section."
                },
                {
                    "@title": "Conclusion",
                    "#text": "Finally, we conclude with our findings."
                }
            ]
        }
    })];

    let writer = Cursor::new(Vec::new());

    let result = tokio::runtime::Runtime::new()?
        .block_on(async { pipeline.generate(data.into_iter(), writer).await })?;

    let pdf_bytes = result.into_inner();

    // Save for inspection
    std::fs::write("test_toc_role_template.pdf", &pdf_bytes)?;
    println!("Saved test_toc_role_template.pdf");

    // Verify PDF was created
    assert!(pdf_bytes.len() > 100, "PDF should have content");

    let doc = LopdfDocument::load_mem(&pdf_bytes)?;
    let pages = doc.get_pages();
    println!("Number of pages: {}", pages.len());

    // Extract text
    let extracted_text = extract_text_from_pdf(&pdf_bytes)?;
    println!("Extracted text:\n{}", extracted_text);

    // Verify TOC heading is present
    assert!(
        extracted_text.contains("Table of Contents") || extracted_text.contains("Contents"),
        "PDF should contain TOC heading, got: {:?}",
        extracted_text
    );

    // Verify section titles appear in text
    assert!(
        extracted_text.contains("Introduction"),
        "Should contain 'Introduction'"
    );
    assert!(
        extracted_text.contains("Background"),
        "Should contain 'Background'"
    );
    assert!(
        extracted_text.contains("Methodology"),
        "Should contain 'Methodology'"
    );
    assert!(
        extracted_text.contains("Results"),
        "Should contain 'Results'"
    );
    assert!(
        extracted_text.contains("Conclusion"),
        "Should contain 'Conclusion'"
    );

    // Extract and check link annotations
    let annotations = extract_link_annotations(&pdf_bytes)?;
    println!("Found {} annotations", annotations.len());
    println!("Annotations: {:#?}", annotations);

    let link_count = count_internal_links(&pdf_bytes)?;
    println!("Number of internal links: {}", link_count);

    // Verify we have internal links (should be at least 5 for 5 sections)
    assert!(
        link_count >= 5,
        "Should have at least 5 internal links for TOC entries, but found {}",
        link_count
    );

    Ok(())
}

#[test]
fn test_toc_with_existing_template() -> Result<(), Box<dyn std::error::Error>> {
    let _ = env_logger::builder().is_test(true).try_init();

    // Read the actual toc_template.xsl file
    let template_path = "templates/toc_template.xsl";
    if !std::path::Path::new(template_path).exists() {
        println!("Skipping test: {} not found", template_path);
        return Ok(());
    }

    let xslt_template = std::fs::read_to_string(template_path)?;

    // Auto mode will detect TOC and use composing renderer
    let pipeline = PipelineBuilder::new()
        .with_template_source(&xslt_template, "xslt")?
        .build()?;

    // Create sample data matching the template structure
    let data = vec![json!({
        "documentTitle": "Test Document with TOC",
        "sections": [
            {
                "id": "chapter-1",
                "title": "Chapter 1: Getting Started",
                "content": "This is the first chapter with introductory content.",
                "subsections": [
                    {
                        "id": "chapter-1-1",
                        "title": "1.1 Installation",
                        "content": "How to install the software."
                    }
                ]
            },
            {
                "id": "chapter-2",
                "title": "Chapter 2: Advanced Topics",
                "content": "This chapter covers more advanced topics.",
                "subsections": []
            },
            {
                "id": "chapter-3",
                "title": "Chapter 3: Best Practices",
                "content": "Here we discuss best practices.",
                "subsections": [
                    {
                        "id": "chapter-3-1",
                        "title": "3.1 Code Quality",
                        "content": "Best practices for code quality."
                    },
                    {
                        "id": "chapter-3-2",
                        "title": "3.2 Performance",
                        "content": "Best practices for performance."
                    }
                ]
            }
        ]
    })];

    let writer = Cursor::new(Vec::new());

    let result = tokio::runtime::Runtime::new()?
        .block_on(async { pipeline.generate(data.into_iter(), writer).await })?;

    let pdf_bytes = result.into_inner();

    // Save for inspection
    std::fs::write("test_toc_existing_template.pdf", &pdf_bytes)?;
    println!("Saved test_toc_existing_template.pdf");

    // Verify PDF was created
    assert!(pdf_bytes.len() > 100, "PDF should have content");

    let doc = LopdfDocument::load_mem(&pdf_bytes)?;
    let pages = doc.get_pages();
    println!("Number of pages: {}", pages.len());

    // Extract text
    let extracted_text = extract_text_from_pdf(&pdf_bytes)?;
    println!("Extracted text from existing template:\n{}", extracted_text);

    // Extract and check link annotations
    let annotations = extract_link_annotations(&pdf_bytes)?;
    println!(
        "Found {} annotations in existing template",
        annotations.len()
    );
    println!("Annotations: {:#?}", annotations);

    let link_count = count_internal_links(&pdf_bytes)?;
    println!(
        "Number of internal links in existing template: {}",
        link_count
    );

    // Verify that links were created (should have links for all heading levels)
    // Expected: 3 sections + 3 subsections = 6 headings total
    assert!(
        link_count >= 6,
        "Should have at least 6 internal links for TOC entries (3 sections + 3 subsections), but found {}",
        link_count
    );

    // Verify TOC content appears
    assert!(
        extracted_text.contains("Table of Contents"),
        "Should contain TOC header"
    );
    assert!(
        extracted_text.contains("Chapter 1: Getting Started"),
        "Should contain section 1"
    );
    assert!(
        extracted_text.contains("Chapter 2: Advanced Topics"),
        "Should contain section 2"
    );
    assert!(
        extracted_text.contains("Chapter 3: Best Practices"),
        "Should contain section 3"
    );

    Ok(())
}

#[test]
fn test_anchor_and_link_basic() -> Result<(), Box<dyn std::error::Error>> {
    let _ = env_logger::builder().is_test(true).try_init();

    // Simple test with explicit anchor and link
    let xslt_template = r#"<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet version="1.0"
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:fo="http://www.w3.org/1999/XSL/Format">

    <fo:simple-page-master page-width="210mm" page-height="297mm" margin="2cm"/>

    <xsl:template match="/">
        <fo:block>
            <!-- Link at the top -->
            <fo:block margin-bottom="2cm">
                <fo:link destination="target-section">
                    <fo:text>Click here to go to target section</fo:text>
                </fo:link>
            </fo:block>

            <!-- Some spacer content -->
            <fo:block margin-bottom="3cm">
                <fo:text>Some content in between...</fo:text>
            </fo:block>

            <!-- Target anchor -->
            <fo:block id="target-section" font-weight="bold" margin-top="2cm">
                <fo:text>Target Section</fo:text>
            </fo:block>
            <fo:block>
                <fo:text>This is the content of the target section.</fo:text>
            </fo:block>
        </fo:block>
    </xsl:template>

</xsl:stylesheet>"#;

    // Auto mode will detect link/anchor and use composing renderer if needed
    let pipeline = PipelineBuilder::new()
        .with_template_source(xslt_template, "xslt")?
        .build()?;

    let data = vec![json!({})];
    let writer = Cursor::new(Vec::new());

    let result = tokio::runtime::Runtime::new()?
        .block_on(async { pipeline.generate(data.into_iter(), writer).await })?;

    let pdf_bytes = result.into_inner();

    // Save for inspection
    std::fs::write("test_anchor_link_basic.pdf", &pdf_bytes)?;
    println!("Saved test_anchor_link_basic.pdf");

    // Verify PDF was created
    assert!(pdf_bytes.len() > 100, "PDF should have content");

    // Extract text
    let extracted_text = extract_text_from_pdf(&pdf_bytes)?;
    println!("Extracted text:\n{}", extracted_text);

    // Verify content is present
    assert!(
        extracted_text.contains("Target Section"),
        "Should contain 'Target Section'"
    );
    assert!(
        extracted_text.contains("Click here"),
        "Should contain link text"
    );

    // Check for link annotations
    let link_count = count_internal_links(&pdf_bytes)?;
    println!("Number of internal links: {}", link_count);

    // Should have at least 1 link
    assert!(
        link_count >= 1,
        "Should have at least 1 internal link, but found {}",
        link_count
    );

    Ok(())
}
