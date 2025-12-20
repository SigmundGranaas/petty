// Processor that implements TemplateParser trait for JSON templates
use crate::ast::JsonTemplateFile;
use crate::compiler::{Compiler, JsonInstruction};
use crate::executor::TemplateExecutor;
use petty_idf::IRNode;
use petty_style::stylesheet::Stylesheet;
use petty_template_core::{
    CompiledTemplate, ExecutionConfig, TemplateError, TemplateFeatures, TemplateFlags,
    TemplateParser,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// JSON template parser
pub struct JsonParser;

/// Compiled JSON template
pub struct CompiledJsonTemplate {
    instructions: Vec<JsonInstruction>,
    definitions: HashMap<String, Vec<JsonInstruction>>,
    stylesheet: Arc<Stylesheet>,
    features: TemplateFlags,
    resource_base_path: PathBuf,
}

impl CompiledTemplate for CompiledJsonTemplate {
    fn execute(
        &self,
        data_source: &str,
        _config: ExecutionConfig,
    ) -> Result<Vec<IRNode>, TemplateError> {
        let data: serde_json::Value = serde_json::from_str(data_source)
            .map_err(|e| TemplateError::ParseError(format!("JSON parse error: {}", e)))?;

        let mut executor = TemplateExecutor::new(&self.stylesheet, &self.definitions);
        executor
            .build_tree(&self.instructions, &data)
            .map_err(|e: crate::error::JsonTemplateError| -> TemplateError { e.into() })
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

impl TemplateParser for JsonParser {
    fn parse(
        &self,
        template_source: &str,
        resource_base_path: PathBuf,
    ) -> Result<TemplateFeatures, TemplateError> {
        let template_file: JsonTemplateFile = serde_json::from_str(template_source)
            .map_err(|e| TemplateError::ParseError(format!("JSON parse error: {}", e)))?;

        // Build stylesheet from the template file
        let mut stylesheet = Stylesheet::default();

        // Set default page master
        if let Some(default_master) = &template_file._stylesheet.default_page_master {
            stylesheet.default_page_master_name = Some(default_master.clone());
        }

        // Add page masters
        for (name, layout) in template_file._stylesheet.page_masters {
            stylesheet.page_masters.insert(name, layout);
        }

        // Add styles
        for (name, style) in template_file._stylesheet.styles {
            stylesheet.styles.insert(name, Arc::new(style));
        }

        let stylesheet = Arc::new(stylesheet);

        // Extract definitions from stylesheet
        let mut definitions = HashMap::new();
        let empty_defs = HashMap::new();
        for (name, template_node) in template_file._stylesheet.definitions {
            let compiler = Compiler::new(&stylesheet, &empty_defs);
            let instructions = compiler
                .compile(&template_node)
                .map_err(|e: crate::error::JsonTemplateError| -> TemplateError { e.into() })?;
            definitions.insert(name, instructions);
        }

        // Compile the main template into executable instructions
        let compiler = Compiler::new(&stylesheet, &definitions);
        let instructions = compiler
            .compile(&template_file._template)
            .map_err(|e: crate::error::JsonTemplateError| -> TemplateError { e.into() })?;

        // Detect features from the compiled instructions
        let features = detect_features(&instructions);

        // Compile role templates
        let mut role_templates = HashMap::new();
        for (role_name, role_template_node) in template_file._roles {
            let role_compiler = Compiler::new(&stylesheet, &definitions);
            let role_instructions = role_compiler
                .compile(&role_template_node)
                .map_err(|e: crate::error::JsonTemplateError| -> TemplateError { e.into() })?;

            role_templates.insert(
                role_name,
                Arc::new(CompiledJsonTemplate {
                    instructions: role_instructions,
                    definitions: definitions.clone(),
                    stylesheet: Arc::clone(&stylesheet),
                    features,
                    resource_base_path: resource_base_path.clone(),
                }) as Arc<dyn CompiledTemplate>,
            );
        }

        Ok(TemplateFeatures {
            main_template: Arc::new(CompiledJsonTemplate {
                instructions,
                definitions,
                stylesheet,
                features,
                resource_base_path,
            }),
            role_templates,
        })
    }
}

/// Detect template features by scanning the compiled instructions
fn detect_features(instructions: &[JsonInstruction]) -> TemplateFlags {
    let mut flags = TemplateFlags::default();
    scan_instructions_for_features(instructions, &mut flags);
    flags
}

/// Recursively scan instructions to detect features
fn scan_instructions_for_features(instructions: &[JsonInstruction], flags: &mut TemplateFlags) {
    for instruction in instructions {
        match instruction {
            JsonInstruction::TableOfContents { .. } => {
                flags.has_table_of_contents = true;
            }
            JsonInstruction::PageReference { .. } => {
                flags.has_page_number_placeholders = true;
            }
            JsonInstruction::Hyperlink { children, .. } => {
                flags.has_internal_links = true;
                scan_instructions_for_features(children, flags);
            }
            JsonInstruction::IndexMarker { .. } => {
                // Index markers might require metadata pass
            }
            // Recursively scan container children
            JsonInstruction::Block { children, .. }
            | JsonInstruction::FlexContainer { children, .. }
            | JsonInstruction::List { children, .. }
            | JsonInstruction::ListItem { children, .. }
            | JsonInstruction::Paragraph { children, .. }
            | JsonInstruction::Heading { children, .. }
            | JsonInstruction::StyledSpan { children, .. } => {
                scan_instructions_for_features(children, flags);
            }
            JsonInstruction::Table(table) => {
                if let Some(header) = &table.header {
                    scan_instructions_for_features(header, flags);
                }
                scan_instructions_for_features(&table.body, flags);
            }
            JsonInstruction::ForEach { body, .. } => {
                scan_instructions_for_features(body, flags);
            }
            JsonInstruction::If {
                then_branch,
                else_branch,
                ..
            } => {
                scan_instructions_for_features(then_branch, flags);
                scan_instructions_for_features(else_branch, flags);
            }
            _ => {}
        }
    }
}
