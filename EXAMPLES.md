# Petty Examples and Patterns

Practical examples and recipes for common PDF generation tasks.

## Table of Contents

- [Simple Documents](#simple-documents)
- [Business Documents](#business-documents)
- [Reports and Analytics](#reports-and-analytics)
- [Books and Publications](#books-and-publications)
- [Advanced Patterns](#advanced-patterns)
- [Performance Optimization](#performance-optimization)

---

## Simple Documents

### Hello World

The simplest possible PDF generation.

**Template (hello.xsl):**
```xml
<xsl:stylesheet version="1.0"
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:fo="http://www.w3.org/1999/XSL/Format">

    <fo:simple-page-master page-width="8.5in" page-height="11in" margin="1in"/>

    <xsl:template match="/">
        <fo:block>
            <p font-size="24pt" font-weight="bold">Hello, {{name}}!</p>
            <p>Welcome to Petty PDF generation.</p>
        </fo:block>
    </xsl:template>

</xsl:stylesheet>
```

**Code:**
```rust
use petty::{PipelineBuilder, PipelineError};
use serde_json::json;

fn main() -> Result<(), PipelineError> {
    let pipeline = PipelineBuilder::new()
        .with_template_file("hello.xsl")?
        .build()?;

    let data = vec![json!({"name": "World"})];
    pipeline.generate_to_file(data, "hello.pdf")?;

    Ok(())
}
```

### Letter Template

Professional business letter.

**Template:**
```xml
<xsl:stylesheet version="1.0"
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:fo="http://www.w3.org/1999/XSL/Format">

    <fo:simple-page-master page-width="8.5in" page-height="11in" margin="1in"/>

    <xsl:template match="/">
        <fo:block>
            <!-- Letterhead -->
            <flex-container margin-bottom="20pt">
                <fo:block width="70%">
                    <p use-attribute-sets="company-name">{{company/name}}</p>
                    <p use-attribute-sets="company-address">
                        {{company/address}}<br/>
                        {{company/city}}, {{company/state}} {{company/zip}}
                    </p>
                </fo:block>
                <fo:block width="30%">
                    <image src="{{company/logo}}" width="100pt" align="right"/>
                </fo:block>
            </flex-container>

            <!-- Date -->
            <p margin-bottom="20pt">{{date}}</p>

            <!-- Recipient -->
            <p margin-bottom="20pt">
                {{recipient/name}}<br/>
                {{recipient/address}}<br/>
                {{recipient/city}}, {{recipient/state}} {{recipient/zip}}
            </p>

            <!-- Salutation -->
            <p margin-bottom="15pt">Dear {{recipient/name}},</p>

            <!-- Body -->
            {{#each paragraphs}}
            <p margin-bottom="10pt">{{this}}</p>
            {{/each}}

            <!-- Closing -->
            <p margin-top="20pt">Sincerely,</p>
            <p margin-top="40pt">{{sender/name}}</p>
            <p>{{sender/title}}</p>
        </fo:block>
    </xsl:template>

    <!-- Styles -->
    <xsl:attribute-set name="company-name">
        <xsl:attribute name="font-size">16pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="company-address">
        <xsl:attribute name="font-size">10pt</xsl:attribute>
        <xsl:attribute name="color">#666</xsl:attribute>
    </xsl:attribute-set>

</xsl:stylesheet>
```

---

## Business Documents

### Invoice with Calculations

Complete invoice with line items and totals.

**Template (invoice.xsl):**
```xml
<xsl:stylesheet version="1.0"
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:fo="http://www.w3.org/1999/XSL/Format">

    <fo:simple-page-master page-width="8.5in" page-height="11in" margin="0.75in">
        <fo:header>
            <flex-container border-bottom="2pt solid #333" padding-bottom="10pt">
                <fo:block width="60%">
                    <p font-size="24pt" font-weight="bold">INVOICE</p>
                </fo:block>
                <fo:block width="40%" text-align="right">
                    <image src="logo.png" width="120pt"/>
                </fo:block>
            </flex-container>
        </fo:header>
        <fo:footer>
            <p text-align="center" font-size="9pt" color="#666">
                Page {{page_num}} | {{company_name}} | {{company_phone}}
            </p>
        </fo:footer>
    </fo:simple-page-master>

    <xsl:template match="/">
        <fo:block>
            <!-- Invoice Info -->
            <flex-container margin-bottom="20pt">
                <fo:block width="50%">
                    <p use-attribute-sets="label">Invoice Number:</p>
                    <p use-attribute-sets="value">{{invoiceNumber}}</p>
                    <p use-attribute-sets="label" margin-top="10pt">Date:</p>
                    <p use-attribute-sets="value">{{date}}</p>
                    <p use-attribute-sets="label" margin-top="10pt">Due Date:</p>
                    <p use-attribute-sets="value">{{dueDate}}</p>
                </fo:block>
                <fo:block width="50%">
                    <p use-attribute-sets="section-header">Bill To:</p>
                    <p>{{customer/name}}</p>
                    <p>{{customer/address}}</p>
                    <p>{{customer/city}}, {{customer/state}} {{customer/zip}}</p>
                    <p>{{customer/email}}</p>
                </fo:block>
            </flex-container>

            <!-- Line Items Table -->
            <table margin-top="20pt">
                <columns>
                    <column width="50%"/>
                    <column width="15%"/>
                    <column width="15%"/>
                    <column width="20%"/>
                </columns>
                <header>
                    <row>
                        <cell use-attribute-sets="th"><p>Description</p></cell>
                        <cell use-attribute-sets="th-right"><p>Quantity</p></cell>
                        <cell use-attribute-sets="th-right"><p>Rate</p></cell>
                        <cell use-attribute-sets="th-right"><p>Amount</p></cell>
                    </row>
                </header>
                <tbody>
                    <xsl:for-each select="items/item">
                        <row>
                            <cell use-attribute-sets="td">
                                <p font-weight="bold">{{description}}</p>
                                {{#if details}}
                                <p font-size="9pt" color="#666">{{details}}</p>
                                {{/if}}
                            </cell>
                            <cell use-attribute-sets="td-right"><p>{{quantity}}</p></cell>
                            <cell use-attribute-sets="td-right"><p>${{rate}}</p></cell>
                            <cell use-attribute-sets="td-right"><p>${{amount}}</p></cell>
                        </row>
                    </xsl:for-each>
                </tbody>
            </table>

            <!-- Totals -->
            <flex-container margin-top="20pt">
                <fo:block width="60%">
                    {{#if notes}}
                    <p use-attribute-sets="section-header">Notes:</p>
                    <p font-size="9pt">{{notes}}</p>
                    {{/if}}
                </fo:block>
                <fo:block width="40%">
                    <flex-container>
                        <fo:block width="60%" text-align="right"><p>Subtotal:</p></fo:block>
                        <fo:block width="40%" text-align="right"><p>${{subtotal}}</p></fo:block>
                    </flex-container>
                    <flex-container margin-top="5pt">
                        <fo:block width="60%" text-align="right"><p>Tax ({{taxRate}}%):</p></fo:block>
                        <fo:block width="40%" text-align="right"><p>${{tax}}</p></fo:block>
                    </flex-container>
                    <flex-container margin-top="5pt" border-top="1pt solid #333" padding-top="5pt">
                        <fo:block width="60%" text-align="right"><p font-weight="bold">Total:</p></fo:block>
                        <fo:block width="40%" text-align="right"><p font-weight="bold" font-size="14pt">${{total}}</p></fo:block>
                    </flex-container>
                </fo:block>
            </flex-container>

            <!-- Payment Terms -->
            <p use-attribute-sets="section-header" margin-top="30pt">Payment Terms:</p>
            <p font-size="9pt">{{paymentTerms}}</p>
        </fo:block>
    </xsl:template>

    <!-- Styles -->
    <xsl:attribute-set name="label">
        <xsl:attribute name="font-size">9pt</xsl:attribute>
        <xsl:attribute name="color">#666</xsl:attribute>
        <xsl:attribute name="text-transform">uppercase</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="value">
        <xsl:attribute name="font-size">11pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="section-header">
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="margin-bottom">8pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="th">
        <xsl:attribute name="background-color">#333</xsl:attribute>
        <xsl:attribute name="color">#FFF</xsl:attribute>
        <xsl:attribute name="padding">8pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="th-right" use-attribute-sets="th">
        <xsl:attribute name="text-align">right</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="td">
        <xsl:attribute name="padding">8pt</xsl:attribute>
        <xsl:attribute name="border-bottom">1pt solid #DDD</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="td-right" use-attribute-sets="td">
        <xsl:attribute name="text-align">right</xsl:attribute>
    </xsl:attribute-set>

</xsl:stylesheet>
```

**Sample Data:**
```json
{
  "invoiceNumber": "INV-2024-001",
  "date": "2024-03-15",
  "dueDate": "2024-04-15",
  "customer": {
    "name": "ACME Corporation",
    "address": "123 Business St",
    "city": "San Francisco",
    "state": "CA",
    "zip": "94105",
    "email": "billing@acme.com"
  },
  "items": [
    {
      "description": "Professional Services",
      "details": "Software development - March 2024",
      "quantity": "40",
      "rate": "150.00",
      "amount": "6000.00"
    },
    {
      "description": "Server Hosting",
      "quantity": "1",
      "rate": "200.00",
      "amount": "200.00"
    }
  ],
  "subtotal": "6200.00",
  "taxRate": "8.5",
  "tax": "527.00",
  "total": "6727.00",
  "paymentTerms": "Net 30. Payment due within 30 days of invoice date.",
  "notes": "Thank you for your business!"
}
```

### Receipt Template

Point-of-sale receipt.

**Template (receipt.xsl):**
```xml
<xsl:stylesheet version="1.0"
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:fo="http://www.w3.org/1999/XSL/Format">

    <!-- Narrow receipt format -->
    <fo:simple-page-master page-width="3in" page-height="11in" margin="0.25in"/>

    <xsl:template match="/">
        <fo:block>
            <!-- Store Header -->
            <p text-align="center" font-weight="bold" font-size="14pt">{{store_name}}</p>
            <p text-align="center" font-size="9pt">{{store_address}}</p>
            <p text-align="center" font-size="9pt" margin-bottom="10pt">{{store_phone}}</p>

            <p text-align="center" border-top="1pt dashed #000" border-bottom="1pt dashed #000" padding="5pt 0">
                Receipt #{{receipt_number}}
            </p>
            <p text-align="center" font-size="9pt" margin-bottom="10pt">{{date_time}}</p>

            <!-- Items -->
            <xsl:for-each select="items/item">
                <flex-container margin-bottom="5pt">
                    <fo:block width="65%">
                        <p font-size="10pt">{{name}}</p>
                        <p font-size="8pt" color="#666">{{quantity}} x ${{price}}</p>
                    </fo:block>
                    <fo:block width="35%" text-align="right">
                        <p font-size="10pt">${{total}}</p>
                    </fo:block>
                </flex-container>
            </xsl:for-each>

            <!-- Totals -->
            <p border-top="1pt dashed #000" margin-top="10pt" padding-top="10pt"></p>
            <flex-container>
                <fo:block width="50%"><p>Subtotal:</p></fo:block>
                <fo:block width="50%" text-align="right"><p>${{subtotal}}</p></fo:block>
            </flex-container>
            <flex-container>
                <fo:block width="50%"><p>Tax:</p></fo:block>
                <fo:block width="50%" text-align="right"><p>${{tax}}</p></fo:block>
            </flex-container>
            <flex-container margin-top="5pt" font-weight="bold" font-size="12pt">
                <fo:block width="50%"><p>Total:</p></fo:block>
                <fo:block width="50%" text-align="right"><p>${{total}}</p></fo:block>
            </flex-container>

            <!-- Payment Info -->
            <p border-top="1pt dashed #000" margin-top="10pt" padding-top="10pt"></p>
            <flex-container>
                <fo:block width="50%"><p>Payment Method:</p></fo:block>
                <fo:block width="50%" text-align="right"><p>{{payment_method}}</p></fo:block>
            </flex-container>
            {{#if change}}
            <flex-container>
                <fo:block width="50%"><p>Change:</p></fo:block>
                <fo:block width="50%" text-align="right"><p>${{change}}</p></fo:block>
            </flex-container>
            {{/if}}

            <!-- Footer -->
            <p text-align="center" margin-top="20pt" font-size="9pt">Thank you for your business!</p>
            <p text-align="center" font-size="8pt" color="#666">{{return_policy}}</p>
        </fo:block>
    </xsl:template>

</xsl:stylesheet>
```

---

## Reports and Analytics

### Monthly Report with Charts

Financial report with placeholder for chart images.

**Template (monthly-report.xsl):**
```xml
<xsl:stylesheet version="1.0"
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:fo="http://www.w3.org/1999/XSL/Format">

    <fo:simple-page-master page-width="8.5in" page-height="11in" margin="0.75in">
        <fo:header>
            <flex-container border-bottom="1pt solid #CCC" padding-bottom="10pt">
                <fo:block width="70%">
                    <p font-size="18pt" font-weight="bold">{{report_title}}</p>
                    <p font-size="10pt" color="#666">{{company_name}} | {{report_period}}</p>
                </fo:block>
                <fo:block width="30%" text-align="right">
                    <image src="{{company_logo}}" width="100pt"/>
                </fo:block>
            </flex-container>
        </fo:header>
        <fo:footer>
            <p text-align="center" border-top="1pt solid #CCC" padding-top="5pt" font-size="9pt" color="#666">
                Generated {{generated_date}} | Page {{page_num}} of {{total_pages}} | Confidential
            </p>
        </fo:footer>
    </fo:simple-page-master>

    <xsl:template match="/">
        <fo:block>
            <!-- Executive Summary -->
            <p use-attribute-sets="section-header">Executive Summary</p>
            <p use-attribute-sets="body-text">{{executive_summary}}</p>

            <!-- Key Metrics -->
            <p use-attribute-sets="section-header" margin-top="20pt">Key Metrics</p>
            <flex-container>
                <xsl:for-each select="key_metrics/metric">
                    <fo:block use-attribute-sets="metric-card" width="32%">
                        <p use-attribute-sets="metric-label">{{label}}</p>
                        <p use-attribute-sets="metric-value">{{value}}</p>
                        <p use-attribute-sets="metric-change" color="{{change_color}}">
                            {{change}} ({{change_percent}}%)
                        </p>
                    </fo:block>
                </xsl:for-each>
            </flex-container>

            <!-- Revenue Chart -->
            <page-break/>
            <p use-attribute-sets="section-header">Revenue Trend</p>
            <image src="{{revenue_chart}}" width="100%"/>

            <!-- Detailed Table -->
            <p use-attribute-sets="section-header" margin-top="30pt">Detailed Breakdown</p>
            <table>
                <columns>
                    <column width="30%"/>
                    <column width="20%"/>
                    <column width="20%"/>
                    <column width="15%"/>
                    <column width="15%"/>
                </columns>
                <header>
                    <row>
                        <cell use-attribute-sets="th"><p>Category</p></cell>
                        <cell use-attribute-sets="th-right"><p>Q1</p></cell>
                        <cell use-attribute-sets="th-right"><p>Q2</p></cell>
                        <cell use-attribute-sets="th-right"><p>Q3</p></cell>
                        <cell use-attribute-sets="th-right"><p>Q4</p></cell>
                    </row>
                </header>
                <tbody>
                    <xsl:for-each select="categories/category">
                        <row>
                            <cell use-attribute-sets="td"><p>{{name}}</p></cell>
                            <cell use-attribute-sets="td-right"><p>${{q1}}</p></cell>
                            <cell use-attribute-sets="td-right"><p>${{q2}}</p></cell>
                            <cell use-attribute-sets="td-right"><p>${{q3}}</p></cell>
                            <cell use-attribute-sets="td-right"><p>${{q4}}</p></cell>
                        </row>
                    </xsl:for-each>
                    <!-- Total Row -->
                    <row>
                        <cell use-attribute-sets="td-total"><p>Total</p></cell>
                        <cell use-attribute-sets="td-total-right"><p>${{total_q1}}</p></cell>
                        <cell use-attribute-sets="td-total-right"><p>${{total_q2}}</p></cell>
                        <cell use-attribute-sets="td-total-right"><p>${{total_q3}}</p></cell>
                        <cell use-attribute-sets="td-total-right"><p>${{total_q4}}</p></cell>
                    </row>
                </tbody>
            </table>

            <!-- Recommendations -->
            <page-break/>
            <p use-attribute-sets="section-header">Recommendations</p>
            <list>
                <xsl:for-each select="recommendations/item">
                    <list-item bullet="•">
                        <p use-attribute-sets="body-text" margin-bottom="8pt">{{this}}</p>
                    </list-item>
                </xsl:for-each>
            </list>
        </fo:block>
    </xsl:template>

    <!-- Styles -->
    <xsl:attribute-set name="section-header">
        <xsl:attribute name="font-size">16pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="color">#2a4d69</xsl:attribute>
        <xsl:attribute name="margin-bottom">10pt</xsl:attribute>
        <xsl:attribute name="border-bottom">2pt solid #4b86b4</xsl:attribute>
        <xsl:attribute name="padding-bottom">4pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="body-text">
        <xsl:attribute name="font-size">11pt</xsl:attribute>
        <xsl:attribute name="line-height">16pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="metric-card">
        <xsl:attribute name="background-color">#F5F5F5</xsl:attribute>
        <xsl:attribute name="padding">15pt</xsl:attribute>
        <xsl:attribute name="margin">5pt</xsl:attribute>
        <xsl:attribute name="border-radius">4pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="metric-label">
        <xsl:attribute name="font-size">10pt</xsl:attribute>
        <xsl:attribute name="color">#666</xsl:attribute>
        <xsl:attribute name="text-transform">uppercase</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="metric-value">
        <xsl:attribute name="font-size">24pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="margin-top">5pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="metric-change">
        <xsl:attribute name="font-size">11pt</xsl:attribute>
        <xsl:attribute name="margin-top">5pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="th">
        <xsl:attribute name="background-color">#2a4d69</xsl:attribute>
        <xsl:attribute name="color">#FFF</xsl:attribute>
        <xsl:attribute name="padding">8pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="th-right" use-attribute-sets="th">
        <xsl:attribute name="text-align">right</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="td">
        <xsl:attribute name="padding">8pt</xsl:attribute>
        <xsl:attribute name="border-bottom">1pt solid #DDD</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="td-right" use-attribute-sets="td">
        <xsl:attribute name="text-align">right</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="td-total">
        <xsl:attribute name="padding">8pt</xsl:attribute>
        <xsl:attribute name="border-top">2pt solid #333</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="td-total-right" use-attribute-sets="td-total">
        <xsl:attribute name="text-align">right</xsl:attribute>
    </xsl:attribute-set>

</xsl:stylesheet>
```

---

## Books and Publications

### Book with Table of Contents

Multi-chapter book with automatic TOC.

**Template (book.xsl):**
```xml
<xsl:stylesheet version="1.0"
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:fo="http://www.w3.org/1999/XSL/Format">

    <fo:simple-page-master page-width="6in" page-height="9in" margin="0.75in">
        <fo:header>
            <p text-align="center" font-size="9pt" color="#666" border-bottom="1pt solid #EEE" padding-bottom="5pt">
                {{book_title}}
            </p>
        </fo:header>
        <fo:footer>
            <p text-align="center" font-size="9pt" color="#666">
                {{page_num}}
            </p>
        </fo:footer>
    </fo:simple-page-master>

    <xsl:template match="/">
        <fo:block>
            <!-- Title Page -->
            <p text-align="center" font-size="36pt" font-weight="bold" margin-top="150pt">
                {{book_title}}
            </p>
            <p text-align="center" font-size="18pt" margin-top="20pt">
                {{author}}
            </p>

            <!-- Table of Contents -->
            <page-break/>
            <p use-attribute-sets="chapter-title">Table of Contents</p>
            <toc-entries/>

            <!-- Chapters -->
            <xsl:for-each select="chapters/chapter">
                <page-break/>

                <heading level="1" id="chapter{{position()}}">
                    Chapter {{position()}}: {{title}}
                </heading>

                <xsl:for-each select="sections/section">
                    <heading level="2" id="chapter{{ancestor::chapter/position()}}-section{{position()}}">
                        {{title}}
                    </heading>

                    <xsl:for-each select="paragraphs/p">
                        <p use-attribute-sets="body">{{this}}</p>
                    </xsl:for-each>
                </xsl:for-each>
            </xsl:for-each>

            <!-- Index -->
            <page-break/>
            <p use-attribute-sets="chapter-title">Index</p>
            <index-entries/>
        </fo:block>
    </xsl:template>

    <!-- Styles -->
    <xsl:attribute-set name="chapter-title">
        <xsl:attribute name="font-size">24pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="margin-bottom">30pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="body">
        <xsl:attribute name="font-size">11pt</xsl:attribute>
        <xsl:attribute name="line-height">16pt</xsl:attribute>
        <xsl:attribute name="text-align">justify</xsl:attribute>
        <xsl:attribute name="margin-bottom">12pt</xsl:attribute>
        <xsl:attribute name="text-indent">20pt</xsl:attribute>
    </xsl:attribute-set>

</xsl:stylesheet>
```

**Code (with MetadataGenerating mode):**
```rust
use petty::{PipelineBuilder, PipelineError, pipeline::GenerationMode};
use serde_json::json;

fn main() -> Result<(), PipelineError> {
    let pipeline = PipelineBuilder::new()
        .with_template_file("book.xsl")?
        .with_generation_mode(GenerationMode::MetadataGenerating)  // Required for TOC
        .build()?;

    let data = vec![json!({
        "book_title": "The Rust Programming Language",
        "author": "Example Author",
        "chapters": [
            {
                "title": "Getting Started",
                "sections": [
                    {
                        "title": "Installation",
                        "paragraphs": ["...", "..."]
                    }
                ]
            }
        ]
    })];

    pipeline.generate_to_file(data, "book.pdf")?;

    Ok(())
}
```

---

## Advanced Patterns

### Conditional Styling

Apply styles based on data conditions.

**XSLT:**
```xml
<!-- Status-based styling -->
<p>
    <xsl:attribute name="color">
        <xsl:choose>
            <xsl:when test="status = 'success'">#28a745</xsl:when>
            <xsl:when test="status = 'warning'">#ffc107</xsl:when>
            <xsl:when test="status = 'error'">#dc3545</xsl:when>
            <xsl:otherwise>#6c757d</xsl:otherwise>
        </xsl:choose>
    </xsl:attribute>
    Status: <xsl:value-of select="status"/>
</p>

<!-- Or using Handlebars -->
<p color="{{#if (eq status 'success')}}#28a745{{else if (eq status 'error')}}#dc3545{{else}}#6c757d{{/if}}">
    Status: {{status}}
</p>
```

**JSON:**
```json
{
  "type": "Paragraph",
  "styleNames": [
    "base-status",
    "{{#if (eq status 'success')}}status-success{{/if}}",
    "{{#if (eq status 'error')}}status-error{{/if}}"
  ],
  "children": [
    {"type": "Text", "content": "Status: {{status}}"}
  ]
}
```

### Dynamic Tables

Generate tables from variable-length data.

```xml
<table>
    <columns>
        <!-- Dynamic column generation -->
        <xsl:for-each select="columns/column">
            <column width="{{width}}"/>
        </xsl:for-each>
    </columns>
    <header>
        <row>
            <xsl:for-each select="columns/column">
                <cell use-attribute-sets="th">
                    <p>{{label}}</p>
                </cell>
            </xsl:for-each>
        </row>
    </header>
    <tbody>
        <xsl:for-each select="rows/row">
            <row>
                <xsl:for-each select="cells/cell">
                    <cell use-attribute-sets="td">
                        <p>{{value}}</p>
                    </cell>
                </xsl:for-each>
            </row>
        </xsl:for-each>
    </tbody>
</table>
```

### Custom Pagination

Control page breaks based on content.

```xml
<xsl:for-each select="items/item">
    <p>{{description}}</p>

    <!-- Page break every 10 items -->
    <xsl:if test="position() mod 10 = 0 and position() != last()">
        <page-break/>
    </xsl:if>
</xsl:for-each>
```

### Watermarks

Add conditional watermarks.

```xml
<fo:simple-page-master page-width="8.5in" page-height="11in" margin="1in">
    {{#if is_draft}}
    <fo:watermark>
        <p text-align="center" font-size="72pt" color="#EEEEEE" opacity="0.3" rotation="45">
            DRAFT
        </p>
    </fo:watermark>
    {{/if}}
</fo:simple-page-master>
```

---

## Performance Optimization

### Batch Processing

Process large datasets efficiently.

```rust
use petty::{PipelineBuilder, PipelineError, executor::{RayonExecutor, ExecutorImpl}};
use serde_json::Value;

fn main() -> Result<(), PipelineError> {
    // Use Rayon for parallel processing
    let pipeline = PipelineBuilder::new()
        .with_template_file("invoice.xsl")?
        .with_executor(ExecutorImpl::Rayon(RayonExecutor::new()))
        .build()?;

    // Load thousands of records
    let invoices: Vec<Value> = load_invoices_from_database()?;

    println!("Processing {} invoices...", invoices.len());

    // Generate in one call - Petty handles parallelism
    pipeline.generate_to_file(invoices, "all_invoices.pdf")?;

    println!("Done!");
    Ok(())
}
```

### Chunked Generation

Generate multiple PDF files for very large datasets.

```rust
use petty::{PipelineBuilder, PipelineError};
use serde_json::Value;

fn main() -> Result<(), PipelineError> {
    let pipeline = PipelineBuilder::new()
        .with_template_file("invoice.xsl")?
        .build()?;

    let all_invoices: Vec<Value> = load_all_invoices()?;

    // Process in chunks of 1000
    for (i, chunk) in all_invoices.chunks(1000).enumerate() {
        let filename = format!("invoices_batch_{}.pdf", i + 1);
        println!("Generating {}...", filename);

        pipeline.generate_to_file(chunk.to_vec(), &filename)?;
    }

    Ok(())
}
```

### Streaming Output

Generate directly to response stream (web server).

```rust
use petty::{PipelineBuilder, PipelineError};
use std::io::Cursor;

fn generate_pdf_response(data: Vec<Value>) -> Result<Vec<u8>, PipelineError> {
    let pipeline = PipelineBuilder::new()
        .with_template_file("report.xsl")?
        .build()?;

    let buffer = Cursor::new(Vec::new());
    let result = pipeline.generate_to_writer(data, buffer)?;

    Ok(result.into_inner())
}

// In your web handler
fn handle_request() -> Response {
    let pdf_bytes = generate_pdf_response(data)?;

    Response::builder()
        .header("Content-Type", "application/pdf")
        .header("Content-Disposition", "attachment; filename=\"report.pdf\"")
        .body(pdf_bytes)
        .unwrap()
}
```

---

## Common Recipes

### Two-Column Layout

```xml
<flex-container>
    <fo:block width="48%" padding-right="2%">
        <p>Left column content</p>
    </fo:block>
    <fo:block width="48%" padding-left="2%">
        <p>Right column content</p>
    </fo:block>
</flex-container>
```

### Alternating Row Colors

```xml
<xsl:for-each select="items/item">
    <row>
        <xsl:attribute name="background-color">
            <xsl:choose>
                <xsl:when test="position() mod 2 = 0">#F9F9F9</xsl:when>
                <xsl:otherwise>#FFFFFF</xsl:otherwise>
            </xsl:choose>
        </xsl:attribute>
        <!-- cells -->
    </row>
</xsl:for-each>
```

### Page Numbering with Section Names

```xml
<fo:footer>
    <flex-container>
        <fo:block width="33%"><p>{{section_name}}</p></fo:block>
        <fo:block width="34%" text-align="center"><p>Page {{page_num}}</p></fo:block>
        <fo:block width="33%" text-align="right"><p>{{date}}</p></fo:block>
    </flex-container>
</fo:footer>
```

### Responsive Font Sizing

```xml
<xsl:attribute-set name="responsive-text">
    <xsl:attribute name="font-size">
        <xsl:choose>
            <xsl:when test="$page-width &gt; 8">14pt</xsl:when>
            <xsl:otherwise>11pt</xsl:otherwise>
        </xsl:choose>
    </xsl:attribute>
</xsl:attribute-set>
```

---

## Next Steps

- [USAGE.md](USAGE.md) - Complete API reference
- [TEMPLATES.md](TEMPLATES.MD) - Full template syntax guide
- [ARCHITECTURE.md](ARCHITECTURE.md) - How Petty works internally
