mod common;

use common::{generate_pdf_from_xslt, TestResult};
use serde_json::json;

#[test]
fn test_toc_generation_xslt() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let template = r#"<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet version="1.0"
    xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
    xmlns:fo="http://www.w3.org/1999/XSL/Format"
    xmlns:petty="http://petty.rs/xsl/extensions">

    <fo:simple-page-master page-width="210mm" page-height="297mm" margin="2cm"/>

    <xsl:template match="/" petty:role="table-of-contents">
        <fo:block font-size="18pt" font-weight="bold">Table of Contents</fo:block>
        <xsl:for-each select="*/headings/item">
            <fo:block margin-left="1cm">
                <fo:link destination="{id}">
                    <xsl:value-of select="text"/>
                    <fo:text> (Page </fo:text>
                    <xsl:value-of select="pageNumber"/>
                    <fo:text>)</fo:text>
                </fo:link>
            </fo:block>
        </xsl:for-each>
    </xsl:template>

    <xsl:template match="/">
        <toc/>
        <xsl:for-each select="document/sections/item">
            <h2 id="{id}"><xsl:value-of select="title"/></h2>
            <p><xsl:value-of select="content"/></p>
        </xsl:for-each>
    </xsl:template>

</xsl:stylesheet>"#;

    let data = json!({
        "document": {
            "sections": [
                { "id": "intro", "title": "Introduction", "content": "Intro content" },
                { "id": "body", "title": "Main Body", "content": "Body content" },
                { "id": "conclusion", "title": "Conclusion", "content": "Conclusion content" }
            ]
        }
    });

    let pdf = generate_pdf_from_xslt(template, data)?;

    assert_pdf_contains_text!(pdf, "Table of Contents");
    assert_pdf_contains_text!(pdf, "Introduction");
    assert_pdf_contains_text!(pdf, "Main Body");
    assert_pdf_contains_text!(pdf, "Conclusion");

    // Verify internal links exist
    let link_count = common::pdf_assertions::count_internal_links(&pdf.doc);
    assert!(
        link_count >= 3,
        "Should have at least 3 links for TOC entries, found {}",
        link_count
    );

    Ok(())
}

#[test]
fn test_toc_with_nested_headings() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let template = r#"<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet version="1.0"
    xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
    xmlns:fo="http://www.w3.org/1999/XSL/Format"
    xmlns:petty="http://petty.rs/xsl/extensions">

    <fo:simple-page-master page-width="210mm" page-height="297mm" margin="2cm"/>

    <xsl:template match="/" petty:role="table-of-contents">
        <fo:block font-size="16pt" font-weight="bold">Contents</fo:block>
        <xsl:for-each select="*/headings/item">
            <fo:block>
                <fo:link destination="{id}">
                    <xsl:value-of select="text"/>
                </fo:link>
            </fo:block>
        </xsl:for-each>
    </xsl:template>

    <xsl:template match="/*">
        <fo:block>
            <toc/>
            <h1 id="ch1">Chapter 1</h1>
            <p>Chapter 1 content</p>
            <h2 id="ch1-1">Section 1.1</h2>
            <p>Section 1.1 content</p>
            <h3 id="ch1-1-1">Subsection 1.1.1</h3>
            <p>Subsection content</p>
        </fo:block>
    </xsl:template>

</xsl:stylesheet>"#;

    let pdf = generate_pdf_from_xslt(template, json!({}))?;

    assert_pdf_contains_text!(pdf, "Chapter 1");
    assert_pdf_contains_text!(pdf, "Section 1.1");
    assert_pdf_contains_text!(pdf, "Subsection 1.1.1");

    Ok(())
}

#[test]
fn test_toc_link_count() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let template = r#"<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet version="1.0"
    xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
    xmlns:fo="http://www.w3.org/1999/XSL/Format"
    xmlns:petty="http://petty.rs/xsl/extensions">

    <fo:simple-page-master page-width="210mm" page-height="297mm" margin="2cm"/>

    <xsl:template match="/" petty:role="table-of-contents">
        <fo:block font-size="18pt" font-weight="bold">Contents</fo:block>
        <xsl:for-each select="*/headings/item">
            <fo:block>
                <fo:link destination="{id}">
                    <xsl:value-of select="text"/>
                </fo:link>
            </fo:block>
        </xsl:for-each>
    </xsl:template>

    <xsl:template match="/">
        <toc/>
        <xsl:for-each select="data/items/item">
            <h2 id="{@id}"><xsl:value-of select="@title"/></h2>
            <p><xsl:value-of select="."/></p>
        </xsl:for-each>
    </xsl:template>

</xsl:stylesheet>"#;

    let data = json!({
        "data": {
            "items": [
                { "@id": "s1", "@title": "Section 1", "#text": "Content 1" },
                { "@id": "s2", "@title": "Section 2", "#text": "Content 2" },
                { "@id": "s3", "@title": "Section 3", "#text": "Content 3" },
                { "@id": "s4", "@title": "Section 4", "#text": "Content 4" },
                { "@id": "s5", "@title": "Section 5", "#text": "Content 5" }
            ]
        }
    });

    let pdf = generate_pdf_from_xslt(template, data)?;

    // Verify we have all sections
    for i in 1..=5 {
        assert_pdf_contains_text!(pdf, &format!("Section {}", i));
    }

    // Verify we have the expected number of internal links
    assert_pdf_min_internal_links!(pdf, 5);

    Ok(())
}
