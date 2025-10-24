# Floppy parsers

This document provides a detailed overview of the two template parsing engines available in the project: the **JSON Template Parser** and the **XSLT 1.0 Parser**. Each parser transforms a template and a data source into an intermediate representation (`IRNode` tree) for further processing.

## At a Glance: Choosing a Parser

| Feature              | JSON Template Parser                                 | XSLT 1.0 Parser                                                |
| -------------------- | ---------------------------------------------------- | -------------------------------------------------------------- |
| **Primary Use Case** | Simple, direct transformations of JSON data.         | Complex, rule-based transformations of XML or JSON data.       |
| **Template Format**  | JSON                                                 | XML (XSLT)                                                     |
| **Data Format(s)**   | JSON                                                 | **XML** and **JSON**                                           |
| **Expression Lang.** | JPath (custom, simplified)                           | XPath 1.0 (W3C Standard)                                       |
| **Learning Curve**   | Low; familiar to web developers.                     | Moderate to High; powerful but complex.                        |
| **Best For**         | Mail-merge style documents, invoices from JSON APIs. | Documents with conditional logic, complex structures, or XML data. |

---

## 1. JSON Template Parser

The JSON parser offers a native, straightforward approach to generating documents from JSON data. The template itself is a JSON file that mirrors the desired output structure, enhanced with special keys for control flow and a simple expression language (`JPath`) for data binding.

### 1.1. Usability

-   **Familiar Syntax:** The entire template is a valid JSON document, making it easy to author and validate with standard tools.
-   **Direct Mapping:** The template structure directly corresponds to the output document structure, which is intuitive for simple documents.
-   **Verbosity:** Due to JSON's syntax, templates can be more verbose than text-based templating languages like Handlebars or Liquid.

### 1.2. Features

#### a. JSON-based Template Structure

The template uses special object keys to define logic:
-   `"each": "jpath-to-array"`: Iterates over an array. The `template` key defines the structure to be rendered for each item. Inside the loop, the context (`.`) shifts to the current array item.
-   `"if": "jpath-expression"`: Conditionally renders a block. It requires a `then` branch and supports an optional `else` branch.

#### b. Data Binding

Dynamic values are inserted into strings (like `"content"`, `"src"`, `"href"`) using a `{{...}}` syntax.
-   Example: `"content": "Report for {{ customer.name }}"`
-   The expression inside the braces is evaluated using the **JPath engine**.

#### c. JPath Expression Language

JPath is a purpose-built language for selecting data from the JSON context. It is inspired by, but **is not** an implementation of, the full JSONPath standard.

**Syntax:**

-   **Object Property Access:** `customer.name`
-   **Array Index Access:** `orders[0]`
-   **Chained Access:** `customer.orders[0].id`
-   **Current Context:** `.` (refers to the current node, especially useful in `each` loops). The parser provides a convenience where `{{ name }}` inside a loop is automatically treated as `{{ .name }}`. The `this` keyword (`{{ this.name }}`) is also supported for compatibility and is translated to `{{ .name }}`.
-   **Variable Access:** `$myVar` (Note: The JSON template system does not currently populate variables, but the engine supports the syntax).

**Literals:**

JPath supports JSON-like literals, primarily for use as arguments in function calls.
-   **Strings:** `'hello world'` (single quotes only)
-   **Numbers:** `123`, `45.6`
-   **Booleans:** `true`, `false`
-   **Null:** `null`

**Built-in Functions:**

| Function                        | Description                                                              |
| ------------------------------- | ------------------------------------------------------------------------ |
| `upper(string)`                 | Converts a string to uppercase.                                          |
| `lower(string)`                 | Converts a string to lowercase.                                          |
| `concat(val1, val2, ...)`       | Concatenates multiple values into a single string.                       |
| `contains(haystack, needle)`    | Returns `true` if the first string contains the second.                  |
| `count(array)`                  | Returns the number of items in an array.                                 |
| `position()`                    | Returns the 1-based index of the current item in a loop.                 |
| `equals(val1, val2)`            | Returns `true` if the two values are equal (string-based comparison).    |

**Truthiness:**

When a JPath expression is used in a conditional context (`if`), its result is coerced to a boolean using the following rules:
-   **Falsy:** `false`, `null`, `0`, `""` (empty string), `[]` (empty array), `{}` (empty object).
-   **Truthy:** All other values.

### 1.3. Limitations

-   **Simplified Data Selection:** JPath only supports simple, downward traversal of the JSON tree. It lacks advanced selection features like:
    -   Wildcards (`*`)
    -   Deep scanning (`..`)
    -   Parent (`..`) or sibling selection.
    -   Filtering expressions within a path.
-   **Limited Reusability:** There is no equivalent to XSLT's `<xsl:call-template>` or `<xsl:apply-templates>`. Template logic cannot be easily broken down into reusable partials, though the `RenderTemplate` node allows calling predefined template snippets from the stylesheet.
-   **Limited Functionality:** The built-in function library is small and not extensible by the user.
-   **JSON Data Only:** This parser can only process `serde_json::Value` data sources.

---

## 2. XSLT 1.0 Parser

This is a powerful, standards-based engine that processes XSLT 1.0 stylesheets. Its most significant feature is its ability to operate on both **XML** and **JSON** data sources interchangeably, thanks to a generic data source abstraction.

### 2.1. Usability

-   **Industry Standard:** XSLT and XPath are mature W3C standards, with extensive documentation and community knowledge available.
-   **Declarative Power:** The rule-based matching of `<xsl:template>` allows for incredibly flexible and decoupled transformations. You define rules for how to process different parts of the data, and the engine applies them as it traverses the tree.
-   **Steeper Learning Curve:** The syntax and concepts (context node, axes, template rules) are more complex than the JSON parser and may be unfamiliar to developers without an XML background.

### 2.2. Core Features

#### a. Multi-Format Data Source (`DataSourceNode`)

The entire XSLT and XPath engine is built against a generic `DataSourceNode` trait. This trait abstracts away the underlying data structure, providing a unified interface for tree navigation and data access. The project provides two implementations:
1.  **`XmlDocument`:** A high-performance wrapper around `roxmltree` for processing standard XML.
2.  **`JsonVDocument`:** An in-memory "Virtual DOM" that represents a `serde_json::Value` as an XML-like tree.

This architecture allows you to write a single XSLT stylesheet and use it to transform both XML and JSON data sources.

#### b. JSON-to-XML Data Model Mapping

To allow XPath to query JSON, the `JsonVDocument` maps JSON structures to the XPath data model using a set of conventions. **You must write your XPath expressions according to this mapping:**

| JSON Structure                           | Mapped XML-like Representation                               | Example XPath                                   |
| ---------------------------------------- | ------------------------------------------------------------ | ----------------------------------------------- |
| `{"user": {"name": "A"}}`                | `<user><name>A</name></user>`                                | `user/name`                                     |
| `{"user": {"@id": "u1"}}`                | `<user id="u1"/>`                                            | `user/@id`                                      |
| `{"items": ["A", "B"]}`                  | `<items><item>A</item><item>B</item></items>`                | `items/item`                                    |
| `{"products": [{"name": "X"}, {"name": "Y"}]}` | `<products><item><name>X</name></item><item><name>Y</name></item></products>` | `products/item/name`                            |
| Top-level object with one key `{ "data": ... }` | A document element named `data`.                             | `/data`                                         |
| Any other top-level JSON                 | A synthetic document element named `root`.                   | `/root`                                         |
| `{"b": 1, "a": 2}`                       | `<a>2</a><b>1</b>` (elements are sorted alphabetically by key) | This ensures deterministic processing order.    |

#### c. Supported XSLT 1.0 Instructions

The compiler and executor support a substantial subset of XSLT 1.0, enabling complex transformations:
-   **Template Rules:** `<xsl:template match="...">`, `<xsl:apply-templates select="..." mode="...">`
-   **Named Templates:** `<xsl:template name="...">`, `<xsl:call-template name="...">`
-   **Parameters:** `<xsl:param>`, `<xsl:with-param>`
-   **Control Flow:** `<xsl:if test="...">`, `<xsl:choose>`, `<xsl:when>`, `<xsl:otherwise>`
-   **Iteration:** `<xsl:for-each select="...">`
-   **Variables:** `<xsl:variable name="..." select="...">`
-   **Output Generation:** `<xsl:value-of>`, `<xsl:copy-of>`, `<xsl:copy>`, `<xsl:attribute>`, `<xsl:element>`, `<xsl:text>`
-   **Sorting:** `<xsl:sort select="..." data-type="..." order="...">`
-   **Indexing:** `<xsl:key name="..." match="..." use="...">`
-   **Literal Result Elements:** Standard HTML/FO tags like `<p>`, `<div>`, `<table>`, etc., are directly transformed into the corresponding `IRNode`s.
-   **Attribute Value Templates (AVTs):** Attributes on literal result elements can contain XPath expressions in curly braces, e.g., `<img src="images/{@id}.jpg"/>`.

#### d. XPath 1.0 Engine

The engine features a robust, standards-compliant implementation of XPath 1.0.

-   **Full Axis Support:** All 11 user-facing axes are implemented (`child`, `descendant`, `parent`, `ancestor`, `following-sibling`, `preceding-sibling`, `following`, `preceding`, `attribute`, `self`, `descendant-or-self`), allowing for complex navigation of the data source tree in any direction.
-   **Comprehensive Function Library:** Includes most of the XPath 1.0 core function library for:
    -   **Node-Set:** `position()`, `last()`, `count()`, `id()`, `name()`, `local-name()`, `key()`
    -   **String:** `concat()`, `contains()`, `starts-with()`, `substring()`, `string-length()`, etc.
    -   **Boolean:** `not()`, `true()`, `false()`, `lang()`
    -   **Number:** `sum()`, `round()`, `floor()`, `ceiling()`
-   **Complete Operator Set:** All standard operators are supported with correct precedence, including arithmetic (`+`, `-`, `*`, `div`, `mod`), logical (`and`, `or`), relational (`=`, `!=`, `<`, `>`), and the node-set union (`|`).

### 2.3. Limitations & Known Behaviors

-   **XPath 1.0 Only:** The engine does not support features from XPath 2.0 or newer, such as schema types, sequences, or the expanded function library.
-   **Single-File Stylesheets:** `<xsl:import>` and `<xsl:include>` are not supported. The entire stylesheet must be contained within a single source file.
-   **Implicit Whitespace Stripping (XML):** When processing an XML data source, the parser behaves as if `xsl:strip-space elements="*"` was declared. This means text nodes containing only whitespace are automatically removed from the tree, which is a common and often desirable behavior but is not the XSLT default.
-   **Incomplete AVT Support:** While supported on literal result element attributes, Attribute Value Templates (AVTs) are not yet implemented for all XSLT instructions where they are allowed by the specification (e.g., `<xsl:attribute name="{$name}">`).
-   **Optional Strict Mode:** By default, the processor follows XSLT 1.0's lenient error handling (e.g., referencing an undeclared variable evaluates to an empty string). A `strict` mode can be enabled in the `ExecutionConfig` to turn these cases into hard errors, which is highly recommended for development.