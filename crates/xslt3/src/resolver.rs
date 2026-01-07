use crate::ast::CompiledStylesheet3;
use crate::compiler::{CompilerBuilder3, StylesheetBuilder3};
use crate::error::Xslt3Error;
use petty_traits::ResourceProvider;
use quick_xml::Reader;
use quick_xml::escape::unescape;
use quick_xml::events::Event;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, RwLock};

const MAX_IMPORT_DEPTH: usize = 100;

pub trait StylesheetResolver: Send + Sync {
    fn resolve(
        &self,
        href: &str,
        base_uri: Option<&str>,
    ) -> Result<CompiledStylesheet3, Xslt3Error>;
}

pub struct CachingStylesheetResolver {
    resource_provider: Arc<dyn ResourceProvider>,
    cache: RwLock<HashMap<String, Arc<CompiledStylesheet3>>>,
    resolving: RwLock<HashSet<String>>,
}

impl std::fmt::Debug for CachingStylesheetResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CachingStylesheetResolver")
            .field(
                "cache_size",
                &self.cache.read().map(|c| c.len()).unwrap_or(0),
            )
            .finish()
    }
}

impl CachingStylesheetResolver {
    pub fn new(resource_provider: Arc<dyn ResourceProvider>) -> Self {
        Self {
            resource_provider,
            cache: RwLock::new(HashMap::new()),
            resolving: RwLock::new(HashSet::new()),
        }
    }

    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.cache.write() {
            cache.clear();
        }
    }

    pub fn cache_size(&self) -> usize {
        self.cache.read().map(|c| c.len()).unwrap_or(0)
    }

    fn resolve_internal(
        &self,
        href: &str,
        base_uri: Option<&str>,
        depth: usize,
    ) -> Result<CompiledStylesheet3, Xslt3Error> {
        if depth > MAX_IMPORT_DEPTH {
            return Err(Xslt3Error::import(
                href,
                format!("Maximum import depth ({}) exceeded", MAX_IMPORT_DEPTH),
            ));
        }

        let absolute_uri = resolve_uri(href, base_uri);

        {
            let resolving = self
                .resolving
                .read()
                .map_err(|_| Xslt3Error::resource("Failed to acquire resolving lock"))?;
            if resolving.contains(&absolute_uri) {
                return Err(Xslt3Error::circular_import(&absolute_uri));
            }
        }

        {
            let cache = self
                .cache
                .read()
                .map_err(|_| Xslt3Error::resource("Failed to acquire cache lock"))?;
            if let Some(cached) = cache.get(&absolute_uri) {
                return Ok((**cached).clone());
            }
        }

        {
            let mut resolving = self
                .resolving
                .write()
                .map_err(|_| Xslt3Error::resource("Failed to acquire resolving lock"))?;
            resolving.insert(absolute_uri.clone());
        }

        let result = self.load_and_resolve(&absolute_uri, depth);

        {
            let mut resolving = self
                .resolving
                .write()
                .map_err(|_| Xslt3Error::resource("Failed to acquire resolving lock"))?;
            resolving.remove(&absolute_uri);
        }

        let stylesheet = result?;

        {
            let mut cache = self
                .cache
                .write()
                .map_err(|_| Xslt3Error::resource("Failed to acquire cache lock"))?;
            cache.insert(absolute_uri, Arc::new(stylesheet.clone()));
        }

        Ok(stylesheet)
    }

    fn load_and_resolve(
        &self,
        absolute_uri: &str,
        depth: usize,
    ) -> Result<CompiledStylesheet3, Xslt3Error> {
        let bytes = self.resource_provider.load(absolute_uri)?;
        let source = std::str::from_utf8(&bytes)
            .map_err(|e| Xslt3Error::import(absolute_uri, format!("Invalid UTF-8: {}", e)))?;

        let mut stylesheet = compile_stylesheet(source)
            .map_err(|e| Xslt3Error::import(absolute_uri, e.to_string()))?;

        let base_for_nested = absolute_uri;
        stylesheet.resolve_imports_includes(|nested_href| {
            self.resolve_internal(nested_href, Some(base_for_nested), depth + 1)
        })?;
        stylesheet.finalize_after_merge();

        Ok(stylesheet)
    }
}

impl StylesheetResolver for CachingStylesheetResolver {
    fn resolve(
        &self,
        href: &str,
        base_uri: Option<&str>,
    ) -> Result<CompiledStylesheet3, Xslt3Error> {
        self.resolve_internal(href, base_uri, 0)
    }
}

pub fn resolve_uri(href: &str, base_uri: Option<&str>) -> String {
    if href.starts_with('/') || href.contains("://") {
        return href.to_string();
    }

    match base_uri {
        Some(base) => {
            let base_path = Path::new(base);
            let base_dir = base_path.parent().unwrap_or(Path::new(""));
            let resolved = base_dir.join(href);
            normalize_path(&resolved.to_string_lossy())
        }
        None => href.to_string(),
    }
}

fn normalize_path(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            _ => parts.push(part),
        }
    }
    if path.starts_with('/') {
        format!("/{}", parts.join("/"))
    } else {
        parts.join("/")
    }
}

pub fn compile_stylesheet(source: &str) -> Result<CompiledStylesheet3, Xslt3Error> {
    let mut builder = CompilerBuilder3::new();
    let mut reader = Reader::from_str(source);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();

    loop {
        let pos = reader.buffer_position() as usize;
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let owned_e = e.to_owned();
                let attrs: Vec<(Vec<u8>, Vec<u8>)> = owned_e
                    .attributes()
                    .filter_map(|a| a.ok())
                    .map(|a| (a.key.as_ref().to_vec(), a.value.to_vec()))
                    .collect();
                builder.start_element(&owned_e, attrs, pos, source)?;
            }
            Ok(Event::Empty(ref e)) => {
                let owned_e = e.to_owned();
                let attrs: Vec<(Vec<u8>, Vec<u8>)> = owned_e
                    .attributes()
                    .filter_map(|a| a.ok())
                    .map(|a| (a.key.as_ref().to_vec(), a.value.to_vec()))
                    .collect();
                builder.empty_element(&owned_e, attrs, pos, source)?;
            }
            Ok(Event::End(ref e)) => {
                builder.end_element(&e.to_owned(), pos, source)?;
            }
            Ok(Event::Text(ref e)) => {
                let raw_text = std::str::from_utf8(e.as_ref())
                    .map_err(|e| Xslt3Error::parse(e.to_string()))?;
                let text = unescape(raw_text)
                    .map_err(|e| Xslt3Error::parse(e.to_string()))?
                    .into_owned();
                builder.text(text)?;
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(Xslt3Error::parse(e.to_string())),
            _ => {}
        }
        buf.clear();
    }

    builder.finalize()
}

#[cfg(test)]
mod tests {
    use super::*;
    use petty_traits::InMemoryResourceProvider;

    #[test]
    fn test_resolve_uri_absolute() {
        assert_eq!(
            resolve_uri("/absolute/path.xsl", None),
            "/absolute/path.xsl"
        );
        assert_eq!(
            resolve_uri("http://example.com/style.xsl", None),
            "http://example.com/style.xsl"
        );
    }

    #[test]
    fn test_resolve_uri_relative() {
        assert_eq!(
            resolve_uri("utils.xsl", Some("/templates/main.xsl")),
            "/templates/utils.xsl"
        );
        assert_eq!(
            resolve_uri("../common/base.xsl", Some("/templates/main.xsl")),
            "/common/base.xsl"
        );
        assert_eq!(
            resolve_uri("lib/helpers.xsl", Some("/templates/main.xsl")),
            "/templates/lib/helpers.xsl"
        );
    }

    #[test]
    fn test_resolve_uri_no_base() {
        assert_eq!(resolve_uri("relative.xsl", None), "relative.xsl");
    }

    #[test]
    fn test_normalize_path() {
        assert_eq!(normalize_path("/a/b/../c"), "/a/c");
        assert_eq!(normalize_path("/a/./b/c"), "/a/b/c");
        assert_eq!(normalize_path("a/b/../c"), "a/c");
    }

    #[test]
    fn test_compile_stylesheet_basic() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <output>Test</output>
                </xsl:template>
            </xsl:stylesheet>
        "#;
        let result = compile_stylesheet(xslt);
        assert!(result.is_ok());
    }

    #[test]
    fn test_caching_resolver_caches() {
        let provider = Arc::new(InMemoryResourceProvider::new());
        provider
            .add(
                "style.xsl",
                br#"<xsl:stylesheet version="3.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                    <xsl:template match="/"><out/></xsl:template>
                </xsl:stylesheet>"#
                    .to_vec(),
            )
            .unwrap();

        let resolver = CachingStylesheetResolver::new(provider);

        assert_eq!(resolver.cache_size(), 0);
        let _ = resolver.resolve("style.xsl", None).unwrap();
        assert_eq!(resolver.cache_size(), 1);
        let _ = resolver.resolve("style.xsl", None).unwrap();
        assert_eq!(resolver.cache_size(), 1);
    }

    #[test]
    fn test_caching_resolver_circular_import() {
        let provider = Arc::new(InMemoryResourceProvider::new());
        provider
            .add(
                "a.xsl",
                br#"<xsl:stylesheet version="3.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                    <xsl:import href="b.xsl"/>
                    <xsl:template match="/"><a/></xsl:template>
                </xsl:stylesheet>"#
                    .to_vec(),
            )
            .unwrap();
        provider
            .add(
                "b.xsl",
                br#"<xsl:stylesheet version="3.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                    <xsl:import href="a.xsl"/>
                    <xsl:template match="/"><b/></xsl:template>
                </xsl:stylesheet>"#
                    .to_vec(),
            )
            .unwrap();

        let resolver = CachingStylesheetResolver::new(provider);
        let result = resolver.resolve("a.xsl", None);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, Xslt3Error::CircularImport(_)));
    }

    #[test]
    fn test_caching_resolver_recursive_imports() {
        let provider = Arc::new(InMemoryResourceProvider::new());
        provider
            .add(
                "main.xsl",
                br#"<xsl:stylesheet version="3.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                    <xsl:import href="base.xsl"/>
                    <xsl:template match="/"><main/></xsl:template>
                </xsl:stylesheet>"#
                    .to_vec(),
            )
            .unwrap();
        provider
            .add(
                "base.xsl",
                br#"<xsl:stylesheet version="3.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                    <xsl:template name="helper">Helper</xsl:template>
                </xsl:stylesheet>"#
                    .to_vec(),
            )
            .unwrap();

        let resolver = CachingStylesheetResolver::new(provider);
        let result = resolver.resolve("main.xsl", None);

        assert!(result.is_ok());
        let stylesheet = result.unwrap();
        assert!(stylesheet.named_templates.contains_key("helper"));
    }

    #[test]
    fn test_caching_resolver_nested_relative_imports() {
        let provider = Arc::new(InMemoryResourceProvider::new());
        provider
            .add(
                "/templates/main.xsl",
                br#"<xsl:stylesheet version="3.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                    <xsl:import href="lib/utils.xsl"/>
                    <xsl:template match="/"><main/></xsl:template>
                </xsl:stylesheet>"#
                    .to_vec(),
            )
            .unwrap();
        provider
            .add(
                "/templates/lib/utils.xsl",
                br#"<xsl:stylesheet version="3.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                    <xsl:import href="../common/base.xsl"/>
                    <xsl:template name="util">Util</xsl:template>
                </xsl:stylesheet>"#
                    .to_vec(),
            )
            .unwrap();
        provider
            .add(
                "/templates/common/base.xsl",
                br#"<xsl:stylesheet version="3.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                    <xsl:template name="base">Base</xsl:template>
                </xsl:stylesheet>"#
                    .to_vec(),
            )
            .unwrap();

        let resolver = CachingStylesheetResolver::new(provider);
        let result = resolver.resolve("/templates/main.xsl", None);

        assert!(result.is_ok());
        let stylesheet = result.unwrap();
        assert!(stylesheet.named_templates.contains_key("util"));
        assert!(stylesheet.named_templates.contains_key("base"));
    }

    #[test]
    fn test_caching_resolver_clear_cache() {
        let provider = Arc::new(InMemoryResourceProvider::new());
        provider
            .add(
                "style.xsl",
                br#"<xsl:stylesheet version="3.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                    <xsl:template match="/"><out/></xsl:template>
                </xsl:stylesheet>"#
                    .to_vec(),
            )
            .unwrap();

        let resolver = CachingStylesheetResolver::new(provider);
        let _ = resolver.resolve("style.xsl", None).unwrap();
        assert_eq!(resolver.cache_size(), 1);

        resolver.clear_cache();
        assert_eq!(resolver.cache_size(), 0);
    }

    #[test]
    fn test_caching_resolver_not_found() {
        let provider = Arc::new(InMemoryResourceProvider::new());
        let resolver = CachingStylesheetResolver::new(provider);

        let result = resolver.resolve("nonexistent.xsl", None);
        assert!(result.is_err());
    }
}
