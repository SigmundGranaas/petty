mod common;

use common::fixtures::*;
use common::{generate_pdf_from_json_with_data, generate_pdf_from_xslt, TestResult};
use serde_json::json;

// ============================================================================
// JSON Template Features
// ============================================================================

#[test]
fn test_json_data_binding() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let template = json!({
        "_stylesheet": {
            "defaultPageMaster": "default",
            "pageMasters": { "default": { "size": "A4", "margins": "2cm" } },
            "styles": {}
        },
        "_template": {
            "type": "Paragraph",
            "children": [{ "type": "Text", "content": "Hello {{name}}!" }]
        }
    });

    let data = json!({ "name": "World" });
    let pdf = generate_pdf_from_json_with_data(&template, data)?;

    assert_pdf_contains_text!(pdf, "Hello World!");
    Ok(())
}

#[test]
fn test_json_nested_data_binding() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let template = json!({
        "_stylesheet": {
            "defaultPageMaster": "default",
            "pageMasters": { "default": { "size": "A4", "margins": "2cm" } },
            "styles": {}
        },
        "_template": {
            "type": "Paragraph",
            "children": [
                { "type": "Text", "content": "Name: {{user.name}}, " },
                { "type": "Text", "content": "City: {{user.address.city}}" }
            ]
        }
    });

    let data = json!({
        "user": {
            "name": "Alice",
            "address": { "city": "Wonderland" }
        }
    });
    let pdf = generate_pdf_from_json_with_data(&template, data)?;

    assert_pdf_contains_text!(pdf, "Name: Alice");
    assert_pdf_contains_text!(pdf, "City: Wonderland");
    Ok(())
}

#[test]
fn test_json_each_loop() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let template = json!({
        "_stylesheet": {
            "defaultPageMaster": "default",
            "pageMasters": { "default": { "size": "A4", "margins": "2cm" } },
            "styles": {}
        },
        "_template": {
            "type": "Block",
            "children": [{
                "each": "items",
                "template": {
                    "type": "Paragraph",
                    "children": [{ "type": "Text", "content": "- {{name}}" }]
                }
            }]
        }
    });

    let data = json!({
        "items": [
            { "name": "Apple" },
            { "name": "Banana" },
            { "name": "Cherry" }
        ]
    });
    let pdf = generate_pdf_from_json_with_data(&template, data)?;

    assert_pdf_contains_text!(pdf, "Apple");
    assert_pdf_contains_text!(pdf, "Banana");
    assert_pdf_contains_text!(pdf, "Cherry");
    Ok(())
}

#[test]
fn test_json_if_conditional_true() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let template = json!({
        "_stylesheet": {
            "defaultPageMaster": "default",
            "pageMasters": { "default": { "size": "A4", "margins": "2cm" } },
            "styles": {}
        },
        "_template": {
            "type": "Block",
            "children": [{
                "if": "showMessage",
                "then": paragraph("Message is shown")
            }]
        }
    });

    let data = json!({ "showMessage": true });
    let pdf = generate_pdf_from_json_with_data(&template, data)?;

    assert_pdf_contains_text!(pdf, "Message is shown");
    Ok(())
}

#[test]
fn test_json_if_conditional_false() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let template = json!({
        "_stylesheet": {
            "defaultPageMaster": "default",
            "pageMasters": { "default": { "size": "A4", "margins": "2cm" } },
            "styles": {}
        },
        "_template": {
            "type": "Block",
            "children": [
                paragraph("Always shown"),
                {
                    "if": "showMessage",
                    "then": paragraph("Should not appear")
                }
            ]
        }
    });

    let data = json!({ "showMessage": false });
    let pdf = generate_pdf_from_json_with_data(&template, data)?;

    assert_pdf_contains_text!(pdf, "Always shown");
    assert_pdf_not_contains_text!(pdf, "Should not appear");
    Ok(())
}

#[test]
fn test_json_if_else() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let template = json!({
        "_stylesheet": {
            "defaultPageMaster": "default",
            "pageMasters": { "default": { "size": "A4", "margins": "2cm" } },
            "styles": {}
        },
        "_template": {
            "type": "Block",
            "children": [{
                "if": "hasPermission",
                "then": paragraph("Access granted"),
                "else": paragraph("Access denied")
            }]
        }
    });

    let data = json!({ "hasPermission": false });
    let pdf = generate_pdf_from_json_with_data(&template, data)?;

    assert_pdf_contains_text!(pdf, "Access denied");
    assert_pdf_not_contains_text!(pdf, "Access granted");
    Ok(())
}

// ============================================================================
// XSLT Template Features
// ============================================================================

#[test]
fn test_xslt_value_of() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let template = r#"<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet version="1.0"
    xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
    xmlns:fo="http://www.w3.org/1999/XSL/Format">

    <fo:simple-page-master page-width="210mm" page-height="297mm" margin="2cm"/>

    <xsl:template match="/">
        <fo:block>
            <p>Title: <xsl:value-of select="document/title"/></p>
        </fo:block>
    </xsl:template>
</xsl:stylesheet>"#;

    let data = json!({ "document": { "title": "My Document" } });
    let pdf = generate_pdf_from_xslt(template, data)?;

    assert_pdf_contains_text!(pdf, "Title: My Document");
    Ok(())
}

#[test]
fn test_xslt_for_each() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let template = r#"<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet version="1.0"
    xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
    xmlns:fo="http://www.w3.org/1999/XSL/Format">

    <fo:simple-page-master page-width="210mm" page-height="297mm" margin="2cm"/>

    <xsl:template match="/">
        <fo:block>
            <xsl:for-each select="data/items/item">
                <p><xsl:value-of select="name"/></p>
            </xsl:for-each>
        </fo:block>
    </xsl:template>
</xsl:stylesheet>"#;

    let data = json!({
        "data": {
            "items": [
                { "name": "Item A" },
                { "name": "Item B" },
                { "name": "Item C" }
            ]
        }
    });
    let pdf = generate_pdf_from_xslt(template, data)?;

    assert_pdf_contains_text!(pdf, "Item A");
    assert_pdf_contains_text!(pdf, "Item B");
    assert_pdf_contains_text!(pdf, "Item C");
    Ok(())
}

#[test]
fn test_xslt_if() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let template = r#"<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet version="1.0"
    xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
    xmlns:fo="http://www.w3.org/1999/XSL/Format">

    <fo:simple-page-master page-width="210mm" page-height="297mm" margin="2cm"/>

    <xsl:template match="/">
        <fo:block>
            <xsl:if test="data/active = 'true'">
                <p>Active content</p>
            </xsl:if>
        </fo:block>
    </xsl:template>
</xsl:stylesheet>"#;

    let data = json!({ "data": { "active": "true" } });
    let pdf = generate_pdf_from_xslt(template, data)?;

    assert_pdf_contains_text!(pdf, "Active content");
    Ok(())
}

#[test]
fn test_xslt_choose() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let template = r#"<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet version="1.0"
    xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
    xmlns:fo="http://www.w3.org/1999/XSL/Format">

    <fo:simple-page-master page-width="210mm" page-height="297mm" margin="2cm"/>

    <xsl:template match="/">
        <fo:block>
            <xsl:choose>
                <xsl:when test="data/status = 'active'">
                    <p>Status is active</p>
                </xsl:when>
                <xsl:when test="data/status = 'pending'">
                    <p>Status is pending</p>
                </xsl:when>
                <xsl:otherwise>
                    <p>Status is unknown</p>
                </xsl:otherwise>
            </xsl:choose>
        </fo:block>
    </xsl:template>
</xsl:stylesheet>"#;

    let data = json!({ "data": { "status": "pending" } });
    let pdf = generate_pdf_from_xslt(template, data)?;

    assert_pdf_contains_text!(pdf, "Status is pending");
    Ok(())
}

#[test]
fn test_xslt_apply_templates() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let template = r#"<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet version="1.0"
    xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
    xmlns:fo="http://www.w3.org/1999/XSL/Format">

    <fo:simple-page-master page-width="210mm" page-height="297mm" margin="2cm"/>

    <xsl:template match="sections/item">
        <fo:block font-weight="bold"><xsl:value-of select="title"/></fo:block>
        <p><xsl:value-of select="content"/></p>
    </xsl:template>

    <xsl:template match="/">
        <fo:block>
            <xsl:apply-templates select="document/sections/item"/>
        </fo:block>
    </xsl:template>
</xsl:stylesheet>"#;

    let data = json!({
        "document": {
            "sections": [
                { "title": "Section A", "content": "Content A" },
                { "title": "Section B", "content": "Content B" }
            ]
        }
    });
    let pdf = generate_pdf_from_xslt(template, data)?;

    assert_pdf_contains_text!(pdf, "Section A");
    assert_pdf_contains_text!(pdf, "Content A");
    assert_pdf_contains_text!(pdf, "Section B");
    Ok(())
}
