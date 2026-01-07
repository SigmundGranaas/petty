use crate::ast::{
    Accumulator, CompiledStylesheet3, Function3, GlobalParam, GlobalVariable, NamedTemplate3,
    TemplateRule3, UsePackage, Visibility,
};
use crate::error::Xslt3Error;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Package {
    pub name: Option<String>,
    pub version: String,
    pub package_version: Option<String>,
    pub visibility_defaults: VisibilityDefaults,
    pub components: PackageComponents,
    pub used_packages: Vec<ResolvedPackageRef>,
}

#[derive(Debug, Clone, Default)]
pub struct VisibilityDefaults {
    pub template: Visibility,
    pub function: Visibility,
    pub variable: Visibility,
    pub mode: Visibility,
    pub accumulator: Visibility,
}

#[derive(Debug, Clone)]
pub struct ResolvedPackageRef {
    pub name: String,
    pub version: Option<String>,
    pub package: Arc<Package>,
    pub accept_rules: Vec<AcceptRule>,
}

#[derive(Debug, Clone)]
pub struct AcceptRule {
    pub component_type: ComponentType,
    pub names: NamePattern,
    pub visibility: Visibility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentType {
    Template,
    Function,
    Variable,
    Mode,
    Accumulator,
    AttributeSet,
}

#[derive(Debug, Clone)]
pub enum NamePattern {
    All,
    Specific(Vec<String>),
    Wildcard(String),
}

#[derive(Debug, Clone, Default)]
pub struct PackageComponents {
    pub template_rules: HashMap<Option<String>, Vec<TemplateRule3>>,
    pub named_templates: HashMap<String, Arc<NamedTemplate3>>,
    pub functions: HashMap<String, Arc<Function3>>,
    pub variables: HashMap<String, GlobalVariable>,
    pub params: HashMap<String, GlobalParam>,
    pub accumulators: HashMap<String, Accumulator>,
    pub modes: HashMap<String, ModeDefinition>,
}

#[derive(Debug, Clone)]
pub struct ModeDefinition {
    pub name: String,
    pub visibility: Visibility,
    pub streamable: bool,
    pub on_no_match: OnNoMatch,
    pub on_multiple_match: OnMultipleMatch,
    pub warning_on_no_match: bool,
    pub warning_on_multiple_match: bool,
}

impl Default for ModeDefinition {
    fn default() -> Self {
        Self {
            name: String::new(),
            visibility: Visibility::Private,
            streamable: false,
            on_no_match: OnNoMatch::TextOnlyCopy,
            on_multiple_match: OnMultipleMatch::UseLast,
            warning_on_no_match: false,
            warning_on_multiple_match: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OnNoMatch {
    #[default]
    TextOnlyCopy,
    ShallowCopy,
    DeepCopy,
    ShallowSkip,
    DeepSkip,
    Fail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OnMultipleMatch {
    #[default]
    UseLast,
    Fail,
}

pub struct PackageRegistry {
    packages: HashMap<String, HashMap<String, Arc<Package>>>,
}

impl PackageRegistry {
    pub fn new() -> Self {
        Self {
            packages: HashMap::new(),
        }
    }

    pub fn register(&mut self, package: Package) -> Result<(), Xslt3Error> {
        let name = package
            .name
            .clone()
            .ok_or_else(|| Xslt3Error::package("Package must have a name to be registered"))?;

        let version = package.version.clone();

        let versions = self.packages.entry(name.clone()).or_default();
        if versions.contains_key(&version) {
            return Err(Xslt3Error::package(format!(
                "Package {}#{} is already registered",
                name, version
            )));
        }
        versions.insert(version, Arc::new(package));
        Ok(())
    }

    pub fn resolve(&self, name: &str, version: Option<&str>) -> Result<Arc<Package>, Xslt3Error> {
        let versions = self
            .packages
            .get(name)
            .ok_or_else(|| Xslt3Error::package(format!("Package not found: {}", name)))?;

        if let Some(v) = version {
            versions
                .get(v)
                .cloned()
                .ok_or_else(|| Xslt3Error::package(format!("Package {}#{} not found", name, v)))
        } else {
            versions
                .values()
                .last()
                .cloned()
                .ok_or_else(|| Xslt3Error::package(format!("No versions of package: {}", name)))
        }
    }
}

impl Default for PackageRegistry {
    fn default() -> Self {
        Self::new()
    }
}

pub struct PackageComposer<'a> {
    registry: &'a PackageRegistry,
}

impl<'a> PackageComposer<'a> {
    pub fn new(registry: &'a PackageRegistry) -> Self {
        Self { registry }
    }

    pub fn compose(
        &self,
        stylesheet: &CompiledStylesheet3,
    ) -> Result<ComposedStylesheet, Xslt3Error> {
        let mut composed = ComposedStylesheet::from_stylesheet(stylesheet);

        for use_pkg in &stylesheet.use_packages {
            self.apply_use_package(&mut composed, use_pkg)?;
        }

        Ok(composed)
    }

    fn apply_use_package(
        &self,
        composed: &mut ComposedStylesheet,
        use_pkg: &UsePackage,
    ) -> Result<(), Xslt3Error> {
        let package = self
            .registry
            .resolve(&use_pkg.name, use_pkg.version.as_deref())?;

        self.import_visible_templates(composed, &package, use_pkg);
        self.import_visible_functions(composed, &package, use_pkg);
        self.import_visible_variables(composed, &package, use_pkg);
        self.import_accumulators(composed, &package);
        self.import_template_rules(composed, &package, use_pkg);

        Ok(())
    }

    fn import_visible_templates(
        &self,
        composed: &mut ComposedStylesheet,
        package: &Package,
        use_pkg: &UsePackage,
    ) {
        for (name, template) in &package.components.named_templates {
            if !self.is_visible(template.visibility, use_pkg) {
                continue;
            }
            let overridden = use_pkg
                .overrides
                .iter()
                .any(|o| matches!(o, crate::ast::Override::Template { name: n, .. } if n == name));
            if !overridden {
                composed
                    .named_templates
                    .insert(name.clone(), template.clone());
            }
        }
    }

    fn import_visible_functions(
        &self,
        composed: &mut ComposedStylesheet,
        package: &Package,
        use_pkg: &UsePackage,
    ) {
        for (name, function) in &package.components.functions {
            if !self.is_visible(function.visibility, use_pkg) {
                continue;
            }
            let overridden = use_pkg
                .overrides
                .iter()
                .any(|o| matches!(o, crate::ast::Override::Function { name: n, .. } if n == name));
            if !overridden {
                composed.functions.insert(name.clone(), function.clone());
            }
        }
    }

    fn import_visible_variables(
        &self,
        composed: &mut ComposedStylesheet,
        package: &Package,
        use_pkg: &UsePackage,
    ) {
        for (name, variable) in &package.components.variables {
            if !self.is_visible(variable.visibility, use_pkg) {
                continue;
            }
            let overridden = use_pkg
                .overrides
                .iter()
                .any(|o| matches!(o, crate::ast::Override::Variable { name: n, .. } if n == name));
            if !overridden {
                composed.variables.insert(name.clone(), variable.clone());
            }
        }
    }

    fn import_accumulators(&self, composed: &mut ComposedStylesheet, package: &Package) {
        for (name, accumulator) in &package.components.accumulators {
            composed
                .accumulators
                .insert(name.clone(), accumulator.clone());
        }
    }

    fn import_template_rules(
        &self,
        composed: &mut ComposedStylesheet,
        package: &Package,
        use_pkg: &UsePackage,
    ) {
        for (mode, rules) in &package.components.template_rules {
            for rule in rules {
                if self.is_visible(rule.visibility, use_pkg) {
                    composed
                        .template_rules
                        .entry(mode.clone())
                        .or_default()
                        .push(rule.clone());
                }
            }
        }
    }

    fn is_visible(&self, visibility: Visibility, _use_pkg: &UsePackage) -> bool {
        matches!(visibility, Visibility::Public | Visibility::Final)
    }

    // TODO(P2.4): Replace is_visible() calls with this method to enforce accept rules
    #[allow(dead_code)]
    fn is_visible_with_rules(
        &self,
        visibility: Visibility,
        component_name: &str,
        component_type: ComponentType,
        accept_rules: &[AcceptRule],
    ) -> bool {
        if !matches!(visibility, Visibility::Public | Visibility::Final) {
            return false;
        }

        for rule in accept_rules {
            if rule.component_type != component_type {
                continue;
            }

            let name_matches = match &rule.names {
                NamePattern::All => true,
                NamePattern::Specific(names) => names.contains(&component_name.to_string()),
                NamePattern::Wildcard(pattern) => wildcard_match(pattern, component_name),
            };

            if name_matches {
                return !matches!(rule.visibility, Visibility::Private);
            }
        }

        true
    }
}

// TODO(P2.5): Used by is_visible_with_rules for accept rule pattern matching
#[allow(dead_code)]
fn wildcard_match(pattern: &str, name: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(inner) = pattern.strip_prefix('*').and_then(|p| p.strip_suffix('*')) {
        return name.contains(inner);
    }
    if let Some(suffix) = pattern.strip_prefix('*') {
        return name.ends_with(suffix);
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return name.starts_with(prefix);
    }
    pattern == name
}

#[derive(Debug, Clone)]
pub struct ComposedStylesheet {
    pub version: String,
    pub default_mode: Option<String>,
    pub expand_text: bool,
    pub template_rules: HashMap<Option<String>, Vec<TemplateRule3>>,
    pub named_templates: HashMap<String, Arc<NamedTemplate3>>,
    pub functions: HashMap<String, Arc<Function3>>,
    pub variables: HashMap<String, GlobalVariable>,
    pub params: HashMap<String, GlobalParam>,
    pub accumulators: HashMap<String, Accumulator>,
}

impl ComposedStylesheet {
    pub fn from_stylesheet(stylesheet: &CompiledStylesheet3) -> Self {
        Self {
            version: stylesheet.version.clone(),
            default_mode: stylesheet.default_mode.clone(),
            expand_text: stylesheet.expand_text,
            template_rules: stylesheet.template_rules.clone(),
            named_templates: stylesheet.named_templates.clone(),
            functions: stylesheet.functions.clone(),
            variables: stylesheet.global_variables.clone(),
            params: stylesheet.global_params.clone(),
            accumulators: stylesheet.accumulators.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::PreparsedTemplate;

    #[test]
    fn test_package_registry() {
        let mut registry = PackageRegistry::new();

        let package = Package {
            name: Some("http://example.com/utils".to_string()),
            version: "1.0".to_string(),
            package_version: None,
            visibility_defaults: VisibilityDefaults::default(),
            components: PackageComponents::default(),
            used_packages: Vec::new(),
        };

        registry.register(package).unwrap();

        let resolved = registry
            .resolve("http://example.com/utils", Some("1.0"))
            .unwrap();
        assert_eq!(resolved.name, Some("http://example.com/utils".to_string()));
    }

    #[test]
    fn test_visibility_import() {
        let mut registry = PackageRegistry::new();

        let mut components = PackageComponents::default();
        components.named_templates.insert(
            "public-template".to_string(),
            Arc::new(NamedTemplate3 {
                params: Vec::new(),
                body: PreparsedTemplate(Vec::new()),
                visibility: Visibility::Public,
                as_type: None,
            }),
        );
        components.named_templates.insert(
            "private-template".to_string(),
            Arc::new(NamedTemplate3 {
                params: Vec::new(),
                body: PreparsedTemplate(Vec::new()),
                visibility: Visibility::Private,
                as_type: None,
            }),
        );

        let package = Package {
            name: Some("http://example.com/test".to_string()),
            version: "1.0".to_string(),
            package_version: None,
            visibility_defaults: VisibilityDefaults::default(),
            components,
            used_packages: Vec::new(),
        };

        registry.register(package).unwrap();

        let stylesheet = CompiledStylesheet3 {
            use_packages: vec![UsePackage {
                name: "http://example.com/test".to_string(),
                version: Some("1.0".to_string()),
                overrides: Vec::new(),
            }],
            ..Default::default()
        };

        let composer = PackageComposer::new(&registry);
        let composed = composer.compose(&stylesheet).unwrap();

        assert!(composed.named_templates.contains_key("public-template"));
        assert!(!composed.named_templates.contains_key("private-template"));
    }

    #[test]
    fn test_package_override() {
        let mut registry = PackageRegistry::new();

        let mut components = PackageComponents::default();
        components.named_templates.insert(
            "overridable".to_string(),
            Arc::new(NamedTemplate3 {
                params: Vec::new(),
                body: PreparsedTemplate(vec![crate::ast::Xslt3Instruction::Text(
                    "original".to_string(),
                )]),
                visibility: Visibility::Public,
                as_type: None,
            }),
        );

        let package = Package {
            name: Some("http://example.com/base".to_string()),
            version: "1.0".to_string(),
            package_version: None,
            visibility_defaults: VisibilityDefaults::default(),
            components,
            used_packages: Vec::new(),
        };

        registry.register(package).unwrap();

        let stylesheet = CompiledStylesheet3 {
            use_packages: vec![UsePackage {
                name: "http://example.com/base".to_string(),
                version: Some("1.0".to_string()),
                overrides: vec![crate::ast::Override::Template {
                    name: "overridable".to_string(),
                    body: PreparsedTemplate(vec![crate::ast::Xslt3Instruction::Text(
                        "overridden".to_string(),
                    )]),
                }],
            }],
            ..Default::default()
        };

        let composer = PackageComposer::new(&registry);
        let composed = composer.compose(&stylesheet).unwrap();

        assert!(!composed.named_templates.contains_key("overridable"));
    }

    #[test]
    fn test_wildcard_match_star() {
        assert!(wildcard_match("*", "anything"));
        assert!(wildcard_match("*", ""));
    }

    #[test]
    fn test_wildcard_match_prefix() {
        assert!(wildcard_match("util-*", "util-format"));
        assert!(wildcard_match("util-*", "util-"));
        assert!(!wildcard_match("util-*", "other-format"));
    }

    #[test]
    fn test_wildcard_match_suffix() {
        assert!(wildcard_match("*-helper", "format-helper"));
        assert!(!wildcard_match("*-helper", "format-util"));
    }

    #[test]
    fn test_wildcard_match_contains() {
        assert!(wildcard_match("*util*", "my-util-func"));
        assert!(wildcard_match("*util*", "util"));
        assert!(!wildcard_match("*util*", "other"));
    }

    #[test]
    fn test_wildcard_match_exact() {
        assert!(wildcard_match("exact-name", "exact-name"));
        assert!(!wildcard_match("exact-name", "other-name"));
    }

    #[test]
    fn test_visibility_with_accept_rules_hide() {
        let registry = PackageRegistry::new();
        let composer = PackageComposer::new(&registry);

        let accept_rules = vec![AcceptRule {
            component_type: ComponentType::Template,
            names: NamePattern::Specific(vec!["hidden-template".to_string()]),
            visibility: Visibility::Private,
        }];

        assert!(!composer.is_visible_with_rules(
            Visibility::Public,
            "hidden-template",
            ComponentType::Template,
            &accept_rules,
        ));
    }

    #[test]
    fn test_visibility_with_accept_rules_allow() {
        let registry = PackageRegistry::new();
        let composer = PackageComposer::new(&registry);

        let accept_rules = vec![AcceptRule {
            component_type: ComponentType::Template,
            names: NamePattern::Specific(vec!["visible-template".to_string()]),
            visibility: Visibility::Public,
        }];

        assert!(composer.is_visible_with_rules(
            Visibility::Public,
            "visible-template",
            ComponentType::Template,
            &accept_rules,
        ));
    }

    #[test]
    fn test_visibility_with_accept_rules_wildcard() {
        let registry = PackageRegistry::new();
        let composer = PackageComposer::new(&registry);

        let accept_rules = vec![AcceptRule {
            component_type: ComponentType::Function,
            names: NamePattern::Wildcard("internal-*".to_string()),
            visibility: Visibility::Private,
        }];

        assert!(!composer.is_visible_with_rules(
            Visibility::Public,
            "internal-helper",
            ComponentType::Function,
            &accept_rules,
        ));

        assert!(composer.is_visible_with_rules(
            Visibility::Public,
            "public-helper",
            ComponentType::Function,
            &accept_rules,
        ));
    }

    #[test]
    fn test_visibility_with_accept_rules_different_type() {
        let registry = PackageRegistry::new();
        let composer = PackageComposer::new(&registry);

        let accept_rules = vec![AcceptRule {
            component_type: ComponentType::Template,
            names: NamePattern::All,
            visibility: Visibility::Private,
        }];

        assert!(composer.is_visible_with_rules(
            Visibility::Public,
            "any-function",
            ComponentType::Function,
            &accept_rules,
        ));
    }
}
