#[derive(Debug, Clone, PartialEq)]
pub struct QName {
    pub prefix: Option<String>,
    pub local_name: String,
    pub namespace_uri: Option<String>,
}

impl QName {
    pub fn new(local_name: impl Into<String>) -> Self {
        Self {
            prefix: None,
            local_name: local_name.into(),
            namespace_uri: None,
        }
    }

    pub fn with_namespace(
        prefix: Option<String>,
        local_name: impl Into<String>,
        namespace_uri: Option<String>,
    ) -> Self {
        Self {
            prefix,
            local_name: local_name.into(),
            namespace_uri,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Attribute {
    pub name: QName,
    pub value: String,
}

#[derive(Debug, Clone)]
pub enum StreamEvent {
    StartDocument,
    EndDocument,
    StartElement {
        name: QName,
        attributes: Vec<Attribute>,
    },
    EndElement {
        name: QName,
    },
    Text(String),
    Comment(String),
    ProcessingInstruction {
        target: String,
        data: String,
    },
}

impl StreamEvent {
    pub fn is_start_element(&self) -> bool {
        matches!(self, StreamEvent::StartElement { .. })
    }

    pub fn is_end_element(&self) -> bool {
        matches!(self, StreamEvent::EndElement { .. })
    }

    pub fn is_text(&self) -> bool {
        matches!(self, StreamEvent::Text(_))
    }

    pub fn element_name(&self) -> Option<&QName> {
        match self {
            StreamEvent::StartElement { name, .. } => Some(name),
            StreamEvent::EndElement { name } => Some(name),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AncestorInfo {
    pub name: QName,
    pub attributes: Vec<Attribute>,
    pub position: usize,
}

pub trait StreamEventHandler {
    type Output;
    type Error;

    fn start_document(&mut self) -> Result<(), Self::Error>;
    fn end_document(&mut self) -> Result<Self::Output, Self::Error>;

    fn start_element(&mut self, name: &QName, attributes: &[Attribute]) -> Result<(), Self::Error>;
    fn end_element(&mut self, name: &QName) -> Result<(), Self::Error>;

    fn text(&mut self, content: &str) -> Result<(), Self::Error>;
    fn comment(&mut self, content: &str) -> Result<(), Self::Error>;
    fn processing_instruction(&mut self, target: &str, data: &str) -> Result<(), Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qname_simple() {
        let qname = QName::new("element");
        assert_eq!(qname.local_name, "element");
        assert!(qname.prefix.is_none());
        assert!(qname.namespace_uri.is_none());
    }

    #[test]
    fn test_qname_with_namespace() {
        let qname = QName::with_namespace(
            Some("xsl".to_string()),
            "template",
            Some("http://www.w3.org/1999/XSL/Transform".to_string()),
        );
        assert_eq!(qname.prefix, Some("xsl".to_string()));
        assert_eq!(qname.local_name, "template");
        assert_eq!(
            qname.namespace_uri,
            Some("http://www.w3.org/1999/XSL/Transform".to_string())
        );
    }

    #[test]
    fn test_stream_event_classification() {
        let start = StreamEvent::StartElement {
            name: QName::new("div"),
            attributes: vec![],
        };
        assert!(start.is_start_element());
        assert!(!start.is_end_element());
        assert!(!start.is_text());

        let end = StreamEvent::EndElement {
            name: QName::new("div"),
        };
        assert!(!end.is_start_element());
        assert!(end.is_end_element());

        let text = StreamEvent::Text("hello".to_string());
        assert!(text.is_text());
    }

    #[test]
    fn test_attribute_creation() {
        let attr = Attribute {
            name: QName::new("class"),
            value: "container".to_string(),
        };
        assert_eq!(attr.name.local_name, "class");
        assert_eq!(attr.value, "container");
    }

    #[test]
    fn test_ancestor_info() {
        let ancestor = AncestorInfo {
            name: QName::new("parent"),
            attributes: vec![Attribute {
                name: QName::new("id"),
                value: "p1".to_string(),
            }],
            position: 1,
        };
        assert_eq!(ancestor.name.local_name, "parent");
        assert_eq!(ancestor.position, 1);
        assert_eq!(ancestor.attributes.len(), 1);
    }
}
