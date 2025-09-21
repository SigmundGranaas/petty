// src/parser/xslt/handlers/mod.rs
pub(super) mod block;
pub(super) mod control_flow;
pub(super) mod inline;
pub(super) mod special;
pub(super) mod table;

#[cfg(test)]
pub(super) mod test_helpers {
    use crate::error::PipelineError;
    use crate::idf::IRNode;
    use crate::parser::xslt::builder::TreeBuilder;
    use handlebars::Handlebars;
    use serde_json::Value;

    /// A helper that fully processes an XML fragment and returns the resulting nodes.
    pub fn build_fragment(xml: &str, context: &Value) -> Result<Vec<IRNode>, PipelineError> {
        let handlebars = Handlebars::new();
        let mut builder = TreeBuilder::new(&handlebars);
        builder.build_tree_from_xml_str(xml, context)
    }
}