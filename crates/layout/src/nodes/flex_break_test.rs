#[cfg(test)]
mod tests {
    use crate::LayoutElement;
    use crate::test_utils::{create_paragraph, paginate_test_nodes};
    use petty_idf::IRNode;
    use petty_style::dimension::{Margins, PageSize};
    use petty_style::stylesheet::{PageLayout, Stylesheet};
    use std::collections::HashMap;

    fn create_flex_row(children: Vec<IRNode>) -> IRNode {
        IRNode::FlexContainer {
            meta: Default::default(),
            children,
        }
    }

    fn create_block(children: Vec<IRNode>) -> IRNode {
        IRNode::Block {
            meta: Default::default(),
            children,
        }
    }

    #[test]
    fn test_flex_row_splitting_behavior() {
        // Reproduce the CV skills section splitting issue.
        // Page height small.
        // Block (Spacer).
        // Flex (Row) [Label, Value].
        // Expectation: If it doesn't fit, it should move cleanly or split reasonably.
        // Not produce 1 page per word.

        let stylesheet = Stylesheet {
            page_masters: HashMap::from([(
                "master".to_string(),
                PageLayout {
                    size: PageSize::Custom {
                        width: 300.0,
                        height: 100.0,
                    }, // Small page
                    margins: Some(Margins::all(0.0)),
                    ..Default::default()
                },
            )]),
            default_page_master_name: Some("master".to_string()),
            ..Default::default()
        };

        // Create a spacer block that takes up 90% of the page
        let spacer_text = (0..5).map(|_| "Spacer line").collect::<Vec<_>>().join("\n");
        let spacer = create_paragraph(&spacer_text); // ~5 lines * 14.4 = 72pt.
        // 100pt page. 28pt remaining.

        // Create a Flex Row (Skills Item)
        // Label: "Category:"
        // Value: "Skill 1, Skill 2, Skill 3..." (Long enough to wrap)
        let label = create_block(vec![create_paragraph("Category:")]);

        let skills_text = "Skill A, Skill B, Skill C, Skill D, Skill E, Skill F";
        let value = create_block(vec![create_paragraph(skills_text)]);

        let flex_row = create_flex_row(vec![label, value]);

        // Wrap in a container block (like the "Skills" section wrapper)
        let root = create_block(vec![spacer, flex_row.clone(), flex_row.clone()]);

        let nodes = vec![root];

        let (pages, _, _) = paginate_test_nodes(stylesheet, nodes).unwrap();

        println!("Generated {} pages", pages.len());
        for (i, page) in pages.iter().enumerate() {
            println!("--- Page {} ---", i + 1);
            for el in page {
                if let LayoutElement::Text(t) = &el.element {
                    println!("  Text: '{}' at y={}", t.content, el.y);
                }
            }
        }

        // Analyze results
        // We expect reasonable splitting.
        // If we see 7 pages for 2 items, that's the bug.
        assert!(pages.len() < 5, "Too many pages generated: {}", pages.len());
    }
}
