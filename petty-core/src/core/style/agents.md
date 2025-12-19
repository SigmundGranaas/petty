AI Agent Rules: core Module

This module is the foundational layer of the project, defining primitive data types. Adherence to these rules is critical for project stability.
1. Purpose & Scope

   Mission: The core module defines stable, self-contained, primitive data types for styling and layout (e.g., Color, Dimension, Border, Margins).

   The Styling "Language": These types form the basic vocabulary used by the parser, stylesheet, layout, and render modules.

2. Core Rules

   Rule 1: No Internal Dependencies: The core module must not have any dependencies on other modules within the petty crate (e.g., parser, layout, idf). It sits at the bottom of the dependency graph.

   Rule 2: Data-Oriented: Types in this module should primarily be data containers. They should contain minimal logic. Complex parsing or computation logic belongs in other modules.

   Rule 3: Serialization is Key: All public types in this module must implement serde::Serialize and serde::Deserialize.

   Rule 4: Support Shorthands: Use custom Deserialize implementations to provide ergonomic, string-based shorthands for complex types, such as "1pt solid #000" for Border or "10pt 20pt" for Margins. The actual parsing logic for these shorthands should be delegated to functions in src/parser/style.rs.

   Rule 5: Stability: These types should be changed infrequently. Any modification has a cascading effect on the entire codebase.