use crate::node::TemplateBuilder;
use petty_json_template::ast::{JsonTemplateFile, StylesheetDef};
use petty_style::stylesheet::{ElementStyle, PageLayout};
use std::collections::HashMap;

/// The top-level container for a programmatically-defined template.
///
/// This struct holds the root of the layout tree and all stylesheet definitions
/// (styles, page masters, etc.). Its primary purpose is to be serialized into the
/// JSON template format via the `.to_json()` method.
#[derive(Clone)]
pub struct Template {
    stylesheet: StylesheetDef,
    root: Box<dyn TemplateBuilder>,
    roles: HashMap<String, Box<dyn TemplateBuilder>>,
}

impl Template {
    /// Creates a new template with a given root builder node.
    pub fn new(root: impl TemplateBuilder + 'static) -> Self {
        Self {
            stylesheet: StylesheetDef::default(),
            root: Box::new(root),
            roles: HashMap::new(),
        }
    }

    /// Adds a named style to the template's stylesheet.
    pub fn add_style(mut self, name: &str, style: ElementStyle) -> Self {
        self.stylesheet.styles.insert(name.to_string(), style);
        self
    }

    /// Adds a named page layout (master) to the template's stylesheet.
    pub fn add_page_master(mut self, name: &str, layout: PageLayout) -> Self {
        self.stylesheet
            .page_masters
            .insert(name.to_string(), layout);
        self
    }

    /// Adds a reusable template definition (a "partial").
    pub fn add_definition(
        mut self,
        name: &str,
        definition: impl TemplateBuilder + 'static,
    ) -> Self {
        self.stylesheet
            .definitions
            .insert(name.to_string(), Box::new(definition).build());
        self
    }

    /// Adds a role-specific template (e.g., for a page header).
    pub fn add_role(mut self, name: &str, template: impl TemplateBuilder + 'static) -> Self {
        self.roles.insert(name.to_string(), Box::new(template));
        self
    }

    /// Consumes the template builder and produces the final JSON AST structure.
    pub fn build(self) -> JsonTemplateFile {
        JsonTemplateFile {
            _stylesheet: self.stylesheet,
            _template: self.root.build(),
            _roles: self
                .roles
                .into_iter()
                .map(|(k, v)| (k, v.build()))
                .collect(),
        }
    }

    /// Serializes the complete template definition to a pretty-printed JSON string.
    ///
    /// This is the primary output of the templating module, producing a string that
    /// can be saved to a file or sent over a network for processing by a `JsonParser`.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        let serializable_template = JsonTemplateFile {
            _stylesheet: self.stylesheet.clone(),
            _template: self.root.clone_box().build(),
            _roles: self
                .roles
                .iter()
                .map(|(k, v)| (k.clone(), v.clone_box().build()))
                .collect(),
        };
        serde_json::to_string_pretty(&serializable_template)
    }
}
