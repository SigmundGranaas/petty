#![cfg(test)]
use crate::core::idf::{IRNode, NodeMetadata};
use crate::core::layout::test_utils::{create_paragraph, paginate_test_nodes};
use crate::core::style::dimension::{Margins, PageSize};
use crate::core::style::stylesheet::{PageLayout, Stylesheet};
use std::collections::HashMap;

fn create_index_marker(term: &str) -> IRNode {
    IRNode::IndexMarker {
        meta: NodeMetadata::default(),
        term: term.to_string(),
    }
}

#[test]
fn test_index_marker_collection() {
    let stylesheet = Stylesheet {
        page_masters: HashMap::from([(
            "master".to_string(),
            PageLayout {
                size: PageSize::Custom { width: 500.0, height: 100.0 },
                margins: Some(Margins::all(10.0)),
                ..Default::default()
            },
        )]),
        default_page_master_name: Some("master".to_string()),
        ..Default::default()
    };

    let nodes = vec![
        create_paragraph("Some text."), // height 14.4
        create_index_marker("first"),   // at y=10+14.4
        create_paragraph("More text."),  // height 14.4
        create_index_marker("second"),  // at y=10+14.4+14.4
        create_index_marker("first"),   // at y=10+14.4+14.4
    ];

    let (_, _, index_entries) = paginate_test_nodes(stylesheet, nodes).unwrap();

    assert_eq!(index_entries.len(), 2, "Should have collected two unique index terms.");

    let first_entries = index_entries.get("first").unwrap();
    assert_eq!(first_entries.len(), 2, "Term 'first' should have two entries.");
    assert_eq!(first_entries[0].local_page_index, 0);
    assert!((first_entries[0].y_pos - (10.0 + 14.4)).abs() < 0.1);
    assert_eq!(first_entries[1].local_page_index, 0);
    assert!((first_entries[1].y_pos - (10.0 + 14.4 + 14.4)).abs() < 0.1);

    let second_entries = index_entries.get("second").unwrap();
    assert_eq!(second_entries.len(), 1, "Term 'second' should have one entry.");
    assert_eq!(second_entries[0].local_page_index, 0);
    assert!((second_entries[0].y_pos - (10.0 + 14.4 + 14.4)).abs() < 0.1);
}