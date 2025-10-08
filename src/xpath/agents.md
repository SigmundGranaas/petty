# AI Agent Rules: `xpath` Module

This module provides a simplified, XPath-like syntax for selecting data from a `serde_json::Value` structure. It is a utility used heavily by the `parser` module.

## 1. Purpose & Scope
- **Mission:** To provide a simple but powerful way to select data from a JSON object using path expressions, boolean conditions, and pattern matching.
- **Important:** This is **not** a full XPath 1.0 implementation. It is a purpose-built utility for JSON that borrows familiar XPath syntax.

## 2. Core Rules
- **Rule 1: JSON-Centric Design:** The implementation must be tailored to the structure of `serde_json::Value`.
  - Simple paths like `user.name` are translated to JSON Pointers (`/user/name`).
  - The `$` prefix is used for variable selection (e.g., `$myVar`).
- **Rule 2: Maintain Separation of Concerns:** The module's functionality is divided into three distinct parts. Keep logic separated accordingly.
  1.  **Selection (`Selection` enum):** Handles selecting one or more values from the JSON context. This includes path selection, current node (`.`), and variables (`$var`).
  2.  **Condition (`Condition` enum):** Handles evaluating boolean expressions, such as path existence (`path`), equality (`path = 'literal'`), and logical operators (`or`).
  3.  **Matching (`matches` function):** Handles XSLT-style pattern matching against a JSON node. This is used for `xsl:template match="..."` rules and supports patterns like `*` (any object/array), `text()` (any primitive), and `name-test`.
- **Rule 3: Compile Expressions:** The `parse_*` functions are a critical part of the API. They act as a "compiler," converting string expressions into the efficient, pre-parsed `Selection` and `Condition` enums. This avoids re-parsing the same string repeatedly in a loop.
- **Rule 4: Immutability:** All evaluation functions (`select`, `evaluate`, `matches`) must be pure. They must take immutable references to the data context and not modify it in any way.