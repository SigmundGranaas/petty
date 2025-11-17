// src/parser/json/processor.rs
// src/parser/json/processor.rs
// src/parser/json/processor.rs
use super::{ast, compiler};
use super::executor;
use crate::core::idf::IRNode;
use crate::core::style::stylesheet::Stylesheet;
use crate::error::PipelineError;
use crate::parser::processor::{CompiledTemplate, ExecutionConfig, TemplateParser, TemplateFeatures, TemplateFlags};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug)]
pub struct JsonTemplate {
    instructions: Vec<compiler::JsonInstruction>,
    definitions: HashMap<String, Vec<compiler::JsonInstruction>>,
    stylesheet: Arc<Stylesheet>,
    resource_base_path: PathBuf,
    features: TemplateFlags,
}

/// Scans the JSON template AST for features requiring special handling.
fn detect_features_from_json_ast(node: &ast::TemplateNode, definitions: &HashMap<String, ast::TemplateNode>) -> TemplateFlags {
    let mut features = TemplateFlags::default();
    scan_json_node_for_features(node, &mut features, definitions);
    features
}

fn scan_json_node_for_features(
    node: &ast::TemplateNode,
    features: &mut TemplateFlags,
    definitions: &HashMap<String, ast::TemplateNode>,
) {
    match node {
        ast::TemplateNode::Static(static_node) => {
            match static_node {
                ast::JsonNode::TableOfContents(_) => features.has_table_of_contents = true,
                ast::JsonNode::PageReference { .. } => features.has_page_number_placeholders = true,
                ast::JsonNode::IndexMarker { .. } => features.uses_index_function = true,
                ast::JsonNode::RenderTemplate { name } => {
                    if let Some(def_node) = definitions.get(name) {
                        scan_json_node_for_features(def_node, features, definitions);
                    }
                }
                _ => {}
            }

            // Recurse into children. Each variant with children has a different struct type,
            // so we must handle them in separate match arms.
            let children: Option<&Vec<ast::TemplateNode>> = match static_node {
                ast::JsonNode::Block(c) => Some(&c.children),
                ast::JsonNode::FlexContainer(c) => Some(&c.children),
                ast::JsonNode::List(c) => Some(&c.children),
                ast::JsonNode::ListItem(c) => Some(&c.children),
                ast::JsonNode::TableOfContents(c) => Some(&c.children),
                ast::JsonNode::StyledSpan(c) => Some(&c.children),
                ast::JsonNode::Paragraph(p) => Some(&p.children),
                ast::JsonNode::Heading(h) => Some(&h.children),
                ast::JsonNode::Hyperlink(h) => Some(&h.children),
                ast::JsonNode::Table(t) => {
                    if let Some(header) = &t.header {
                        for row in &header.rows {
                            scan_json_node_for_features(row, features, definitions);
                        }
                    }
                    for row in &t.body.rows {
                        scan_json_node_for_features(row, features, definitions);
                    }
                    None // Table has no single `children` vector, recursion is handled above.
                }
                _ => None,
            };

            if let Some(children_vec) = children {
                for child in children_vec {
                    scan_json_node_for_features(child, features, definitions);
                }
            }
        }
        ast::TemplateNode::Control(control_node) => {
            match control_node {
                ast::ControlNode::Each { template, .. } => {
                    scan_json_node_for_features(template, features, definitions);
                }
                ast::ControlNode::If { then, else_branch, .. } => {
                    scan_json_node_for_features(then, features, definitions);
                    if let Some(else_node) = else_branch {
                        scan_json_node_for_features(else_node, features, definitions);
                    }
                }
            }
        }
    }
}


impl CompiledTemplate for JsonTemplate {
    fn execute(
        &self,
        data_source: &str,
        _config: ExecutionConfig,
    ) -> Result<Vec<IRNode>, PipelineError> {
        let context: serde_json::Value =
            serde_json::from_str(data_source).map_err(|e| PipelineError::Parse(e.into()))?;
        let mut executor = executor::TemplateExecutor::new(&self.stylesheet, &self.definitions);
        let ir_tree = executor
            .build_tree(&self.instructions, &context)
            .map_err(PipelineError::Parse)?;
        Ok(ir_tree)
    }

    fn stylesheet(&self) -> Arc<Stylesheet> {
        Arc::clone(&self.stylesheet)
    }

    fn resource_base_path(&self) -> &Path {
        &self.resource_base_path
    }

    fn features(&self) -> TemplateFlags {
        self.features
    }
}

pub struct JsonParser;

impl TemplateParser for JsonParser {
    fn parse(
        &self,
        source: &str,
        resource_base_path: PathBuf,
    ) -> Result<TemplateFeatures, PipelineError> {
        let file_ast: ast::JsonTemplateFile = serde_json::from_str(source)?;

        let mut stylesheet = Stylesheet {
            page_masters: file_ast._stylesheet.page_masters,
            styles: file_ast
                ._stylesheet
                .styles
                .into_iter()
                .map(|(k, v)| (k, Arc::new(v)))
                .collect(),
            default_page_master_name: file_ast._stylesheet.default_page_master,
            ..Default::default()
        };

        // If no default is specified, first look for one named "default",
        // then fall back to the first one available. This mirrors the XSLT compiler's behavior
        // where a master without a name is implicitly named "default", and ensures a default
        // is always present if any masters are defined.
        if stylesheet.default_page_master_name.is_none() {
            if stylesheet.page_masters.contains_key("default") {
                stylesheet.default_page_master_name = Some("default".to_string());
            } else {
                stylesheet.default_page_master_name = stylesheet.page_masters.keys().next().cloned();
            }
        }

        let definitions_ast = file_ast._stylesheet.definitions;

        // The compiler for definitions needs an empty set of definitions to avoid recursion issues.
        let empty_defs = HashMap::new();
        let def_compiler = compiler::Compiler::new(&stylesheet, &empty_defs);
        let compiled_definitions: HashMap<String, Vec<compiler::JsonInstruction>> = definitions_ast
            .clone()
            .into_iter()
            .map(|(name, node)| def_compiler.compile(&node).map(|instr| (name, instr)))
            .collect::<Result<_, _>>()?;

        let main_compiler = compiler::Compiler::new(&stylesheet, &compiled_definitions);
        let main_instructions = main_compiler.compile(&file_ast._template)?;
        let main_features = detect_features_from_json_ast(&file_ast._template, &definitions_ast);

        let main_template = Arc::new(JsonTemplate {
            instructions: main_instructions,
            definitions: compiled_definitions.clone(),
            stylesheet: Arc::new(stylesheet.clone()),
            resource_base_path: resource_base_path.clone(),
            features: main_features,
        });

        // --- Compile Role Templates ---
        let mut role_templates = HashMap::new();
        if !file_ast._roles.is_empty() {
            let role_compiler = compiler::Compiler::new(&stylesheet, &compiled_definitions);
            for (role_name, role_node) in file_ast._roles {
                let role_instructions = role_compiler.compile(&role_node)?;
                let role_features = detect_features_from_json_ast(&role_node, &definitions_ast);
                let role_template: Arc<dyn CompiledTemplate> = Arc::new(JsonTemplate {
                    instructions: role_instructions,
                    definitions: compiled_definitions.clone(),
                    stylesheet: Arc::new(stylesheet.clone()),
                    resource_base_path: resource_base_path.clone(),
                    features: role_features,
                });
                role_templates.insert(role_name, role_template);
            }
        }

        Ok(TemplateFeatures {
            main_template,
            role_templates,
        })
    }
}