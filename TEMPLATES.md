# Petty Template Syntax Guide

Complete reference for writing XSLT and JSON templates in Petty.

## Table of Contents

- [Overview](#overview)
- [XSLT Templates](#xslt-templates)
- [JSON Templates](#json-templates)
- [Common Elements](#common-elements)
- [Styling](#styling)
- [Data Binding](#data-binding)
- [Advanced Features](#advanced-features)
- [Best Practices](#best-practices)

---

## Overview

Petty supports two template formats:

| Format | Best For | Syntax Style |
|--------|----------|--------------|
| **XSLT** | XML developers, complex transformations | XSL-FO-like XML |
| **JSON** | Beginners, programmatic generation | Declarative JSON |

Both formats compile to the same intermediate representation (`IRNode`), so feature parity is maintained.

---

## XSLT Templates

XSLT templates use an XSL-FO-inspired syntax with Handlebars for data binding.

### Basic Structure

```xml
<xsl:stylesheet version="1.0"
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:fo="http://www.w3.org/1999/XSL/Format">

    <!-- Page setup -->
    <fo:simple-page-master master-name="A4"
                           page-width="210mm"
                           page-height="297mm"
                           margin="20mm"/>

    <!-- Main template -->
    <xsl:template match="/">
        <!-- Document content -->
    </xsl:template>

    <!-- Styles -->
    <xsl:attribute-set name="style-name">
        <xsl:attribute name="property">value</xsl:attribute>
    </xsl:attribute-set>

</xsl:stylesheet>
```

### Page Setup

#### Simple Page Master

Define page dimensions and margins:

```xml
<fo:simple-page-master master-name="A4"
                       page-width="210mm"
                       page-height="297mm"
                       margin="20mm"/>
```

**Attributes:**
- `master-name` - Identifier (default: first master is used)
- `page-width` - Page width (units: pt, mm, cm, in)
- `page-height` - Page height
- `margin` - All margins (shorthand)
- `margin-top`, `margin-right`, `margin-bottom`, `margin-left` - Individual margins

**Named page sizes:**
```xml
<!-- Use predefined sizes -->
<fo:simple-page-master master-name="letter" page-size="Letter"/>
<fo:simple-page-master master-name="legal" page-size="Legal"/>
<fo:simple-page-master master-name="a4" page-size="A4"/>
```

#### Headers and Footers

```xml
<fo:simple-page-master page-width="8.5in" page-height="11in" margin="1in">
    <fo:header>
        <p>Company Name - {{document_title}}</p>
    </fo:header>
    <fo:footer>
        <p>Page {{page_num}} of {{total_pages}}</p>
    </fo:footer>
</fo:simple-page-master>
```

### Templates

#### Root Template

Entry point for document generation:

```xml
<xsl:template match="/">
    <fo:block>
        <!-- Your content here -->
    </fo:block>
</xsl:template>
```

#### Named Templates

Reusable template blocks:

```xml
<xsl:template name="invoice-header">
    <p use-attribute-sets="h1">Invoice</p>
    <p><xsl:value-of select="invoiceNumber"/></p>
</xsl:template>

<!-- Call the template -->
<xsl:call-template name="invoice-header"/>
```

#### Template Matching

Match specific elements:

```xml
<xsl:template match="customer">
    <p>Customer: <xsl:value-of select="name"/></p>
</xsl:template>

<!-- Apply templates -->
<xsl:apply-templates select="customers/customer"/>
```

### Control Flow

#### Iteration

```xml
<!-- for-each -->
<xsl:for-each select="items/item">
    <p><xsl:value-of select="name"/>: $<xsl:value-of select="price"/></p>
</xsl:for-each>
```

#### Conditionals

```xml
<!-- if -->
<xsl:if test="total &gt; 100">
    <p>Discount applied!</p>
</xsl:if>

<!-- choose/when/otherwise -->
<xsl:choose>
    <xsl:when test="status = 'paid'">
        <p>Payment received</p>
    </xsl:when>
    <xsl:when test="status = 'pending'">
        <p>Payment pending</p>
    </xsl:when>
    <xsl:otherwise>
        <p>Unknown status</p>
    </xsl:otherwise>
</xsl:choose>
```

### Data Binding

#### XSL Value-Of

```xml
<!-- Extract value from data -->
<xsl:value-of select="customer/name"/>
<xsl:value-of select="items/item[1]/price"/>
```

#### Handlebars Expressions

```xml
<!-- In text content -->
<p>Hello, {{customer/name}}!</p>

<!-- With helpers -->
<p>Total: {{formatCurrency total}}</p>
<p>Date: {{formatDate orderDate}}</p>

<!-- Conditionals -->
<p>{{#if isPaid}}PAID{{else}}UNPAID{{/if}}</p>

<!-- Iteration -->
{{#each items}}
  <p>{{name}}: {{price}}</p>
{{/each}}
```

#### Attribute Value Templates

```xml
<!-- Use curly braces in attributes -->
<a href="{website}">Visit Website</a>
<img src="{logo}" width="{logoWidth}"/>
```

### Styling

#### Attribute Sets

Define reusable styles:

```xml
<xsl:attribute-set name="h1">
    <xsl:attribute name="font-size">24pt</xsl:attribute>
    <xsl:attribute name="font-weight">bold</xsl:attribute>
    <xsl:attribute name="margin-bottom">12pt</xsl:attribute>
    <xsl:attribute name="color">#333</xsl:attribute>
</xsl:attribute-set>

<!-- Apply to elements -->
<p use-attribute-sets="h1">My Heading</p>
```

#### Style Inheritance

```xml
<!-- Base style -->
<xsl:attribute-set name="th">
    <xsl:attribute name="font-weight">bold</xsl:attribute>
    <xsl:attribute name="padding">4pt</xsl:attribute>
</xsl:attribute-set>

<!-- Extend base style -->
<xsl:attribute-set name="th-right" use-attribute-sets="th">
    <xsl:attribute name="text-align">right</xsl:attribute>
</xsl:attribute-set>
```

#### Inline Styles

```xml
<p font-size="14pt" color="#666" margin-top="10pt">
    Styled paragraph
</p>
```

### Elements Reference

#### Block Elements

**fo:block** - Generic block container:
```xml
<fo:block>
    <p>Paragraph 1</p>
    <p>Paragraph 2</p>
</fo:block>
```

**p** - Paragraph:
```xml
<p>Simple paragraph</p>
<p use-attribute-sets="body-text">Styled paragraph</p>
```

**heading** - Heading with TOC support:
```xml
<heading level="1" id="chapter1">Chapter 1</heading>
<heading level="2" id="section1">Section 1.1</heading>
```

**flex-container** - Flexbox layout:
```xml
<flex-container>
    <fo:block width="60%">Left column</fo:block>
    <fo:block width="40%">Right column</fo:block>
</flex-container>
```

#### Inline Elements

**span** - Inline styled text:
```xml
<p>This is <span font-weight="bold">bold</span> text</p>
```

**a** - Hyperlink:
```xml
<!-- External link -->
<a href="https://example.com">Visit Example</a>

<!-- Internal link to anchor -->
<hyperlink target-id="section2">Go to Section 2</hyperlink>
```

#### Lists

```xml
<list>
    <list-item>
        <p>First item</p>
    </list-item>
    <list-item>
        <p>Second item</p>
    </list-item>
</list>
```

**With custom bullet:**
```xml
<list use-attribute-sets="bullet-list">
    <list-item bullet="•">
        <p>Bulleted item</p>
    </list-item>
</list>
```

#### Tables

```xml
<table>
    <columns>
        <column width="50%"/>
        <column width="25%"/>
        <column width="25%"/>
    </columns>
    <header>
        <row>
            <cell use-attribute-sets="th"><p>Product</p></cell>
            <cell use-attribute-sets="th"><p>Qty</p></cell>
            <cell use-attribute-sets="th"><p>Price</p></cell>
        </row>
    </header>
    <tbody>
        <xsl:for-each select="items/item">
            <row>
                <cell><p><xsl:value-of select="product"/></p></cell>
                <cell><p><xsl:value-of select="quantity"/></p></cell>
                <cell><p><xsl:value-of select="price"/></p></cell>
            </row>
        </xsl:for-each>
    </tbody>
</table>
```

#### Images

```xml
<image src="logo.png" width="200pt" height="100pt"/>
<image src="{{companyLogo}}" width="150pt"/>
```

#### Page Breaks

```xml
<p>Content on page 1</p>
<page-break/>
<p>Content on page 2</p>
```

#### Special Elements

**toc-entries** - Table of contents:
```xml
<toc-entries/>
```

**index-marker** - Mark term for index:
```xml
<p>This document covers <index-marker term="Rust">Rust</index-marker>.</p>
```

**index-entries** - Render index:
```xml
<index-entries/>
```

---

## JSON Templates

JSON templates use a declarative structure with nested objects.

### Basic Structure

```json
{
  "_stylesheet": {
    "page": {
      "size": "A4",
      "margins": "20mm",
      "footer_text": "Page {{page_num}}"
    },
    "styles": {
      "style-name": {
        "property": "value"
      }
    }
  },
  "_template": {
    "type": "Block",
    "children": [
      /* ... */
    ]
  }
}
```

### Stylesheet

```json
{
  "_stylesheet": {
    "defaultPageMaster": "default",
    "pageMasters": {
      "default": {
        "size": "Letter",
        "margins": "1in",
        "marginTop": "0.75in",
        "marginBottom": "1.25in"
      }
    },
    "styles": {
      "h1": {
        "fontSize": "24pt",
        "fontWeight": "bold",
        "marginBottom": "12pt"
      },
      "body": {
        "fontSize": "11pt",
        "lineHeight": "15pt"
      }
    }
  }
}
```

### Template Structure

```json
{
  "_template": {
    "type": "Block",
    "styleNames": ["optional-style"],
    "children": [
      {
        "type": "Paragraph",
        "children": [
          {"type": "Text", "content": "Hello, {{name}}!"}
        ]
      }
    ]
  }
}
```

### Elements Reference

#### Block Elements

**Block:**
```json
{
  "type": "Block",
  "styleNames": ["container"],
  "children": [/* ... */]
}
```

**Paragraph:**
```json
{
  "type": "Paragraph",
  "styleNames": ["body-text"],
  "children": [
    {"type": "Text", "content": "Paragraph text"}
  ]
}
```

**Heading:**
```json
{
  "type": "Heading",
  "level": 1,
  "id": "chapter1",
  "children": [
    {"type": "Text", "content": "{{chapterTitle}}"}
  ]
}
```

**FlexContainer:**
```json
{
  "type": "FlexContainer",
  "children": [
    {
      "type": "Block",
      "style": {"width": "60%"},
      "children": [/* left column */]
    },
    {
      "type": "Block",
      "style": {"width": "40%"},
      "children": [/* right column */]
    }
  ]
}
```

#### Inline Elements

**Text:**
```json
{"type": "Text", "content": "Plain text"}
{"type": "Text", "content": "{{variable}}"}
```

**Span:**
```json
{
  "type": "Span",
  "styleNames": ["bold"],
  "children": [
    {"type": "Text", "content": "Styled text"}
  ]
}
```

**Hyperlink:**
```json
{
  "type": "Hyperlink",
  "href": "https://example.com",
  "children": [
    {"type": "Text", "content": "Visit Example"}
  ]
}
```

**InternalLink:**
```json
{
  "type": "InternalLink",
  "targetId": "section2",
  "children": [
    {"type": "Text", "content": "Go to Section 2"}
  ]
}
```

#### Lists

```json
{
  "type": "List",
  "styleNames": ["bullet-list"],
  "children": [
    {
      "type": "ListItem",
      "children": [
        {"type": "Paragraph", "children": [
          {"type": "Text", "content": "Item 1"}
        ]}
      ]
    },
    {
      "type": "ListItem",
      "children": [
        {"type": "Paragraph", "children": [
          {"type": "Text", "content": "Item 2"}
        ]}
      ]
    }
  ]
}
```

#### Tables

```json
{
  "type": "Table",
  "columns": [
    {"width": "50%"},
    {"width": "25%"},
    {"width": "25%"}
  ],
  "header": {
    "rows": [
      {
        "type": "TableRow",
        "children": [
          {"type": "TableCell", "children": [
            {"type": "Paragraph", "children": [
              {"type": "Text", "content": "Product"}
            ]}
          ]},
          /* ... more cells */
        ]
      }
    ]
  },
  "body": {
    "rows": [
      {
        "each": "items",
        "template": {
          "type": "TableRow",
          "children": [
            {"type": "TableCell", "children": [
              {"type": "Paragraph", "children": [
                {"type": "Text", "content": "{{name}}"}
              ]}
            ]}
          ]
        }
      }
    ]
  }
}
```

#### Images

```json
{
  "type": "Image",
  "src": "logo.png",
  "width": "200pt",
  "height": "100pt"
}
```

#### Special Elements

**PageBreak:**
```json
{"type": "PageBreak"}
```

**TocEntries:**
```json
{"type": "TocEntries"}
```

**IndexMarker:**
```json
{
  "type": "IndexMarker",
  "term": "Rust",
  "children": [
    {"type": "Text", "content": "Rust"}
  ]
}
```

**IndexEntries:**
```json
{"type": "IndexEntries"}
```

### Iteration

Use the `"each"` property:

```json
{
  "each": "items",
  "template": {
    "type": "Paragraph",
    "children": [
      {"type": "Text", "content": "{{name}}: {{price}}"}
    ]
  }
}
```

**With index:**
```json
{
  "each": "items",
  "template": {
    "type": "Paragraph",
    "children": [
      {"type": "Text", "content": "{{@index}}. {{name}}"}
    ]
  }
}
```

### Conditionals

Use Handlebars helpers in style names:

```json
{
  "type": "Paragraph",
  "styleNames": [
    "base-style",
    "{{#if isPaid}}paid-style{{/if}}",
    "{{#if isOverdue}}overdue-style{{/if}}"
  ],
  "children": [
    {"type": "Text", "content": "{{status}}"}
  ]
}
```

---

## Common Elements

### Styling Properties

Available CSS-like properties:

#### Typography
- `font-family` - Font name (e.g., "Helvetica", "Times New Roman")
- `font-size` - Size (e.g., "12pt", "16px")
- `font-weight` - Weight ("normal", "bold", 100-900)
- `font-style` - Style ("normal", "italic")
- `color` - Text color (hex: "#333", name: "red")
- `text-align` - Alignment ("left", "center", "right", "justify")
- `line-height` - Line spacing (e.g., "15pt", "1.5")

#### Spacing
- `margin` - All margins (e.g., "10pt", "5mm 10mm")
- `margin-top`, `margin-right`, `margin-bottom`, `margin-left` - Individual margins
- `padding` - All padding
- `padding-top`, `padding-right`, `padding-bottom`, `padding-left` - Individual padding

#### Borders
- `border` - All borders (e.g., "1pt solid #000")
- `border-top`, `border-right`, `border-bottom`, `border-left` - Individual borders
- `border-width` - Border width
- `border-color` - Border color
- `border-style` - Border style ("solid", "dashed", "dotted")

#### Background
- `background-color` - Background color (hex or name)

#### Layout
- `width` - Element width (e.g., "50%", "200pt")
- `height` - Element height
- `display` - Display type ("block", "inline", "flex")

### Units

Supported units:
- `pt` - Points (1/72 inch) - default
- `px` - Pixels (same as pt)
- `mm` - Millimeters
- `cm` - Centimeters
- `in` - Inches
- `%` - Percentage (relative to parent)

---

## Data Binding

### Handlebars Syntax

Petty uses Handlebars for data binding in both template formats.

#### Variables

```handlebars
{{variable}}
{{object.property}}
{{array.[0]}}
{{nested.object.property}}
```

#### Helpers

**Built-in helpers:**

```handlebars
{{#if condition}}
  True branch
{{else}}
  False branch
{{/if}}

{{#each array}}
  {{this}} or {{property}}
  {{@index}} - Current index
  {{@first}} - Is first item?
  {{@last}} - Is last item?
{{/each}}

{{#with object}}
  {{property}} - Access object properties
{{/with}}

{{#unless condition}}
  Inverted if
{{/unless}}
```

**Comparison helpers:**

```handlebars
{{#if (eq status "paid")}}Paid{{/if}}
{{#if (ne status "pending")}}Not pending{{/if}}
{{#if (gt total 100)}}Over 100{{/if}}
{{#if (lt total 50)}}Under 50{{/if}}
```

**Custom helpers:**

You can register custom helpers (requires code-level integration):

```rust
// In your application
handlebars.register_helper("formatCurrency", Box::new(|h: &Helper, _: &Handlebars, _: &Context, _rc: &mut RenderContext, out: &mut dyn Output| -> HelperResult {
    let value = h.param(0).and_then(|v| v.value().as_f64()).unwrap_or(0.0);
    out.write(&format!("${:.2}", value))?;
    Ok(())
}));
```

Then in templates:
```handlebars
{{formatCurrency total}}
```

### Special Variables

Available in all templates:

- `{{page_num}}` - Current page number
- `{{total_pages}}` - Total page count (MetadataGenerating mode only)

---

## Advanced Features

### Table of Contents

Requires `MetadataGenerating` mode.

**XSLT:**
```xml
<xsl:template match="/">
    <!-- Render TOC -->
    <toc-entries/>
    <page-break/>

    <!-- Content with headings -->
    <heading level="1" id="intro">Introduction</heading>
    <p>Content...</p>

    <heading level="2" id="background">Background</heading>
    <p>More content...</p>
</xsl:template>
```

**JSON:**
```json
{
  "_template": {
    "type": "Block",
    "children": [
      {"type": "TocEntries"},
      {"type": "PageBreak"},
      {
        "type": "Heading",
        "level": 1,
        "id": "intro",
        "children": [{"type": "Text", "content": "Introduction"}]
      }
    ]
  }
}
```

**Styling the TOC:**
```xml
<xsl:attribute-set name="toc-level-1">
    <xsl:attribute name="font-weight">bold</xsl:attribute>
    <xsl:attribute name="margin-top">8pt</xsl:attribute>
</xsl:attribute-set>

<xsl:attribute-set name="toc-level-2">
    <xsl:attribute name="margin-left">15pt</xsl:attribute>
    <xsl:attribute name="margin-top">4pt</xsl:attribute>
</xsl:attribute-set>
```

### Index Generation

**Mark terms:**
```xml
<p>
    This document covers <index-marker term="Rust">Rust programming</index-marker>
    and <index-marker term="PDF">PDF generation</index-marker>.
</p>
```

**Render index:**
```xml
<page-break/>
<heading level="1">Index</heading>
<index-entries/>
```

### Internal Hyperlinks

**Create anchor:**
```xml
<heading level="1" id="chapter2">Chapter 2</heading>
```

**Link to anchor:**
```xml
<hyperlink target-id="chapter2">
    <p>See Chapter 2 for details</p>
</hyperlink>
```

### Responsive Layouts

Use flex containers for responsive layouts:

```xml
<flex-container>
    <fo:block width="60%" padding-right="10pt">
        <p>Main content area</p>
    </fo:block>
    <fo:block width="40%">
        <p>Sidebar content</p>
    </fo:block>
</flex-container>
```

---

## Best Practices

### 1. Organize Styles

Group related styles:

```xml
<!-- Headers -->
<xsl:attribute-set name="h1">...</xsl:attribute-set>
<xsl:attribute-set name="h2">...</xsl:attribute-set>

<!-- Body -->
<xsl:attribute-set name="body">...</xsl:attribute-set>
<xsl:attribute-set name="body-small">...</xsl:attribute-set>

<!-- Tables -->
<xsl:attribute-set name="table">...</xsl:attribute-set>
<xsl:attribute-set name="th">...</xsl:attribute-set>
<xsl:attribute-set name="td">...</xsl:attribute-set>
```

### 2. Use Named Templates

Extract reusable components:

```xml
<xsl:template name="invoice-header">
    <!-- Reusable header -->
</xsl:template>

<xsl:template name="invoice-footer">
    <!-- Reusable footer -->
</xsl:template>
```

### 3. Avoid Deep Nesting

Break complex structures into templates:

```xml
<!-- BAD: Deep nesting -->
<xsl:template match="/">
    <fo:block>
        <xsl:for-each select="orders">
            <xsl:for-each select="items">
                <xsl:for-each select="details">
                    <!-- Too deep -->
                </xsl:for-each>
            </xsl:for-each>
        </xsl:for-each>
    </fo:block>
</xsl:template>

<!-- GOOD: Separate templates -->
<xsl:template match="order">
    <xsl:apply-templates select="items/item"/>
</xsl:template>

<xsl:template match="item">
    <xsl:apply-templates select="details/detail"/>
</xsl:template>
```

### 4. Use Handlebars for Simple Cases

```xml
<!-- Simple variable -->
<p>{{customer_name}}</p>

<!-- Better than -->
<p><xsl:value-of select="customer_name"/></p>

<!-- But use XSL for complex logic -->
<xsl:choose>
    <xsl:when test="...">...</xsl:when>
</xsl:choose>
```

### 5. Test with Sample Data

Always test templates with realistic data:

```json
{
  "customer": "ACME Corp",
  "items": [
    {"name": "Widget", "qty": 10, "price": 9.99},
    {"name": "Gadget", "qty": 5, "price": 19.99}
  ],
  "total": 199.85
}
```

### 6. Consider Mobile/Print

Design for various output contexts:

```xml
<!-- Readable font sizes -->
<xsl:attribute name="font-size">11pt</xsl:attribute>

<!-- Good contrast -->
<xsl:attribute name="color">#333</xsl:attribute>
<xsl:attribute name="background-color">#FFF</xsl:attribute>

<!-- Adequate spacing -->
<xsl:attribute name="line-height">1.5</xsl:attribute>
```

### 7. Document Your Templates

```xml
<!--
  Invoice Template
  Version: 2.0
  Author: Your Name

  Data structure:
  {
    "invoiceNumber": "INV-001",
    "customer": {"name": "...", "address": "..."},
    "items": [{"product": "...", "quantity": N, "price": N}],
    "total": N
  }
-->
```

---

## Troubleshooting

### Common Issues

**Fonts not rendering:**
```xml
<!-- Ensure font is available -->
<xsl:attribute name="font-family">Helvetica</xsl:attribute>

<!-- Or use system fonts -->
pipeline.with_system_fonts()?
```

**Layout overflow:**
```xml
<!-- Use flexible widths -->
<column width="*"/>  <!-- Flexible -->
<column width="200pt"/>  <!-- Fixed -->

<!-- Or percentages -->
<fo:block width="50%">...</fo:block>
```

**Handlebars not expanding:**
```xml
<!-- Ensure double curly braces -->
{{variable}}  <!-- Correct -->
{variable}    <!-- Wrong -->

<!-- Check data structure matches -->
{{customer.name}}  <!-- Requires {"customer": {"name": "..."}} -->
```

**Page breaks not working:**
```xml
<!-- Use dedicated element -->
<page-break/>

<!-- Not a CSS property -->
<fo:block page-break-after="always">...</fo:block>  <!-- Won't work -->
```

---

## Next Steps

- [USAGE.md](USAGE.md) - API guide and examples
- [EXAMPLES.md](EXAMPLES.md) - Common patterns and recipes
- [ARCHITECTURE.md](ARCHITECTURE.md) - How templates are processed internally
