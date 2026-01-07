#![allow(clippy::too_many_arguments)]

use crate::ast::{ForkBranch, MergeAction, MergeKey, MergeSource};
use crate::executor::{ExecutionError, TemplateExecutor3};
use petty_xpath1::datasource::DataSourceNode;
use petty_xpath31::types::XdmValue;
use petty_xslt::idf_builder::IdfBuilder;
use petty_xslt::output::OutputBuilder;
use std::cmp::Ordering;

#[derive(Debug, Clone)]
struct MergeItem<N> {
    source_index: usize,
    node: N,
    keys: Vec<String>,
}

#[derive(Debug, Clone)]
struct MergeSourceState<N> {
    nodes: Vec<N>,
    current_index: usize,
    sort_orders: Vec<bool>,
}

impl<N: Clone> MergeSourceState<N> {
    fn current(&self) -> Option<&N> {
        self.nodes.get(self.current_index)
    }

    fn advance(&mut self) {
        self.current_index += 1;
    }
}

impl<'s, 'a, N: DataSourceNode<'a> + 'a> TemplateExecutor3<'s, 'a, N> {
    pub(crate) fn handle_merge(
        &mut self,
        sources: &[MergeSource],
        action: &MergeAction,
        context_node: N,
        context_position: usize,
        context_size: usize,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        if sources.is_empty() {
            return Ok(());
        }

        let mut source_states: Vec<MergeSourceState<N>> = Vec::new();

        for source in sources {
            let nodes = self.evaluate_xpath31_nodes(
                &source.select,
                context_node,
                context_position,
                context_size,
            )?;

            let sort_orders: Vec<bool> = source
                .sort_keys
                .iter()
                .map(|k| matches!(k.order, petty_xslt::ast::SortOrder::Descending))
                .collect();

            source_states.push(MergeSourceState {
                nodes,
                current_index: 0,
                sort_orders,
            });
        }

        loop {
            let next_items = self.collect_next_merge_group(
                &mut source_states,
                sources,
                context_position,
                context_size,
            )?;

            if next_items.is_empty() {
                break;
            }

            self.push_scope();

            let merge_key = next_items
                .first()
                .map(|item| item.keys.join("|"))
                .unwrap_or_default();
            self.current_merge_key = Some(merge_key);

            self.current_merge_group = next_items.iter().map(|item| item.node).collect();

            for (i, source) in sources.iter().enumerate() {
                if let Some(name) = &source.name {
                    let source_nodes: Vec<N> = next_items
                        .iter()
                        .filter(|item| item.source_index == i)
                        .map(|item| item.node)
                        .collect();

                    self.set_variable(
                        format!("current-merge-group-{}", name),
                        XdmValue::from_items(
                            source_nodes
                                .iter()
                                .map(|n| petty_xpath31::types::XdmItem::Node(*n))
                                .collect(),
                        ),
                    );
                }
            }

            if let Some(first_item) = next_items.first() {
                if let Some(source) = sources.get(first_item.source_index) {
                    self.current_merge_source = source.name.clone();
                }

                self.execute_template(
                    &action.body,
                    first_item.node,
                    context_position,
                    context_size,
                    builder,
                )?;
            }

            self.current_merge_key = None;
            self.current_merge_group.clear();
            self.current_merge_source = None;

            self.pop_scope();
        }

        Ok(())
    }

    fn collect_next_merge_group(
        &self,
        source_states: &mut [MergeSourceState<N>],
        sources: &[MergeSource],
        context_position: usize,
        context_size: usize,
    ) -> Result<Vec<MergeItem<N>>, ExecutionError> {
        let mut candidates: Vec<(usize, MergeItem<N>)> = Vec::new();

        for (i, state) in source_states.iter().enumerate() {
            if let Some(node) = state.current() {
                let keys = self.evaluate_merge_keys(
                    &sources[i].sort_keys,
                    *node,
                    context_position,
                    context_size,
                )?;

                candidates.push((
                    i,
                    MergeItem {
                        source_index: i,
                        node: *node,
                        keys,
                    },
                ));
            }
        }

        if candidates.is_empty() {
            return Ok(Vec::new());
        }

        candidates.sort_by(|(i_a, a), (_, b)| {
            let orders_a = &source_states[*i_a].sort_orders;
            self.compare_merge_keys(&a.keys, &b.keys, orders_a)
        });

        let min_keys = candidates
            .first()
            .map(|(_, item)| item.keys.clone())
            .unwrap_or_default();

        let mut group: Vec<MergeItem<N>> = Vec::new();

        for (source_idx, item) in candidates {
            if item.keys == min_keys {
                group.push(item);
                source_states[source_idx].advance();
            }
        }

        Ok(group)
    }

    fn evaluate_merge_keys(
        &self,
        keys: &[MergeKey],
        node: N,
        context_position: usize,
        context_size: usize,
    ) -> Result<Vec<String>, ExecutionError> {
        let mut result = Vec::new();
        for key in keys {
            let value = self.evaluate_xpath31(&key.select, node, context_position, context_size)?;
            result.push(value);
        }
        Ok(result)
    }

    fn compare_merge_keys(&self, a: &[String], b: &[String], descending: &[bool]) -> Ordering {
        for (i, (key_a, key_b)) in a.iter().zip(b.iter()).enumerate() {
            let desc = descending.get(i).copied().unwrap_or(false);

            let cmp = if let (Ok(num_a), Ok(num_b)) = (key_a.parse::<f64>(), key_b.parse::<f64>()) {
                num_a.partial_cmp(&num_b).unwrap_or(Ordering::Equal)
            } else {
                key_a.cmp(key_b)
            };

            if cmp != Ordering::Equal {
                return if desc { cmp.reverse() } else { cmp };
            }
        }
        Ordering::Equal
    }

    pub(crate) fn handle_fork(
        &mut self,
        branches: &[ForkBranch],
        context_node: N,
        context_position: usize,
        context_size: usize,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        if branches.is_empty() {
            return Ok(());
        }

        if branches.len() == 1 {
            return self.execute_template(
                &branches[0].body,
                context_node,
                context_position,
                context_size,
                builder,
            );
        }

        let mut branch_results: Vec<Vec<petty_idf::IRNode>> = Vec::new();

        for branch in branches {
            let mut branch_builder = IdfBuilder::new();
            self.execute_template(
                &branch.body,
                context_node,
                context_position,
                context_size,
                &mut branch_builder,
            )?;
            branch_results.push(branch_builder.get_result());
        }

        for nodes in branch_results {
            for node in nodes {
                self.output_ir_node(&node, builder);
            }
        }

        Ok(())
    }

    fn output_ir_node(&self, node: &petty_idf::IRNode, builder: &mut dyn OutputBuilder) {
        let default_styles = petty_xslt::ast::PreparsedStyles::default();

        match node {
            petty_idf::IRNode::Root(children) => {
                for child in children {
                    self.output_ir_node(child, builder);
                }
            }
            petty_idf::IRNode::Block { children, .. } => {
                builder.start_block(&default_styles);
                for child in children {
                    self.output_ir_node(child, builder);
                }
                builder.end_block();
            }
            petty_idf::IRNode::FlexContainer { children, .. } => {
                builder.start_flex_container(&default_styles);
                for child in children {
                    self.output_ir_node(child, builder);
                }
                builder.end_flex_container();
            }
            petty_idf::IRNode::Paragraph { children, .. } => {
                builder.start_paragraph(&default_styles);
                for child in children {
                    self.output_inline_node(child, builder);
                }
                builder.end_paragraph();
            }
            petty_idf::IRNode::Heading {
                level, children, ..
            } => {
                builder.start_heading(&default_styles, *level);
                for child in children {
                    self.output_inline_node(child, builder);
                }
                builder.end_heading();
            }
            petty_idf::IRNode::List { children, .. } => {
                builder.start_list(&default_styles);
                for child in children {
                    self.output_ir_node(child, builder);
                }
                builder.end_list();
            }
            petty_idf::IRNode::ListItem { children, .. } => {
                builder.start_list_item(&default_styles);
                for child in children {
                    self.output_ir_node(child, builder);
                }
                builder.end_list_item();
            }
            petty_idf::IRNode::Table { body, .. } => {
                builder.start_table(&default_styles);
                for row in &body.rows {
                    builder.start_table_row(&default_styles);
                    for cell in &row.cells {
                        builder.start_table_cell(&default_styles);
                        for child in &cell.children {
                            self.output_ir_node(child, builder);
                        }
                        builder.end_table_cell();
                    }
                    builder.end_table_row();
                }
                builder.end_table();
            }
            petty_idf::IRNode::Image { .. } => {
                builder.start_image(&default_styles);
                builder.end_image();
            }
            petty_idf::IRNode::PageBreak { master_name } => {
                builder.add_page_break(master_name.clone());
            }
            petty_idf::IRNode::IndexMarker { .. } => {}
        }
    }

    fn output_inline_node(&self, node: &petty_idf::InlineNode, builder: &mut dyn OutputBuilder) {
        let default_styles = petty_xslt::ast::PreparsedStyles::default();

        match node {
            petty_idf::InlineNode::Text(text) => {
                builder.add_text(text);
            }
            petty_idf::InlineNode::StyledSpan { children, .. } => {
                builder.start_styled_span(&default_styles);
                for child in children {
                    self.output_inline_node(child, builder);
                }
                builder.end_styled_span();
            }
            petty_idf::InlineNode::Hyperlink { children, .. } => {
                builder.start_hyperlink(&default_styles);
                for child in children {
                    self.output_inline_node(child, builder);
                }
                builder.end_hyperlink();
            }
            petty_idf::InlineNode::PageReference { children, .. } => {
                for child in children {
                    self.output_inline_node(child, builder);
                }
            }
            petty_idf::InlineNode::Image { .. } => {
                builder.start_image(&default_styles);
                builder.end_image();
            }
            petty_idf::InlineNode::LineBreak => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compare_keys(a: &[String], b: &[String], descending: &[bool]) -> Ordering {
        for (i, (key_a, key_b)) in a.iter().zip(b.iter()).enumerate() {
            let desc = descending.get(i).copied().unwrap_or(false);

            let cmp = if let (Ok(num_a), Ok(num_b)) = (key_a.parse::<f64>(), key_b.parse::<f64>()) {
                num_a.partial_cmp(&num_b).unwrap_or(Ordering::Equal)
            } else {
                key_a.cmp(key_b)
            };

            if cmp != Ordering::Equal {
                return if desc { cmp.reverse() } else { cmp };
            }
        }
        Ordering::Equal
    }

    #[test]
    fn test_compare_merge_keys_ascending() {
        let a = vec!["1".to_string(), "apple".to_string()];
        let b = vec!["2".to_string(), "banana".to_string()];
        let desc = vec![false, false];

        assert_eq!(compare_keys(&a, &b, &desc), Ordering::Less);
    }

    #[test]
    fn test_compare_merge_keys_descending() {
        let a = vec!["1".to_string()];
        let b = vec!["2".to_string()];
        let desc = vec![true];

        assert_eq!(compare_keys(&a, &b, &desc), Ordering::Greater);
    }

    #[test]
    fn test_compare_merge_keys_equal() {
        let a = vec!["same".to_string()];
        let b = vec!["same".to_string()];
        let desc = vec![false];

        assert_eq!(compare_keys(&a, &b, &desc), Ordering::Equal);
    }

    #[test]
    fn test_compare_merge_keys_numeric() {
        let a = vec!["10".to_string()];
        let b = vec!["2".to_string()];
        let desc = vec![false];

        assert_eq!(compare_keys(&a, &b, &desc), Ordering::Greater);
    }

    #[test]
    fn test_compare_merge_keys_string() {
        let a = vec!["apple".to_string()];
        let b = vec!["banana".to_string()];
        let desc = vec![false];

        assert_eq!(compare_keys(&a, &b, &desc), Ordering::Less);
    }
}
