//! Contains pure functions for collecting nodes along each XPath axis.

use crate::datasource::DataSourceNode;
use std::collections::HashSet;

fn add_node<'a, N: DataSourceNode<'a>>(node: N, seen: &mut HashSet<N>, results: &mut Vec<N>) {
    if seen.insert(node) {
        results.push(node);
    }
}

pub fn collect_self_nodes<'a, N: DataSourceNode<'a>>(
    node: N,
    seen: &mut HashSet<N>,
    results: &mut Vec<N>,
) {
    add_node(node, seen, results);
}

pub fn collect_child_nodes<'a, N: DataSourceNode<'a>>(
    node: N,
    seen: &mut HashSet<N>,
    results: &mut Vec<N>,
) {
    for child in node.children() {
        add_node(child, seen, results);
    }
}

pub fn collect_attribute_nodes<'a, N: DataSourceNode<'a>>(
    node: N,
    seen: &mut HashSet<N>,
    results: &mut Vec<N>,
) {
    for attr in node.attributes() {
        add_node(attr, seen, results);
    }
}

pub fn collect_descendant_nodes<'a, N: DataSourceNode<'a>>(
    node: N,
    seen: &mut HashSet<N>,
    results: &mut Vec<N>,
) {
    let mut queue: Vec<N> = node.children().collect();
    while let Some(current) = queue.pop() {
        add_node(current, seen, results);
        queue.extend(current.children());
    }
}

pub fn collect_descendant_or_self_nodes<'a, N: DataSourceNode<'a>>(
    node: N,
    seen: &mut HashSet<N>,
    results: &mut Vec<N>,
) {
    add_node(node, seen, results);
    collect_descendant_nodes(node, seen, results);
}

pub fn collect_parent_nodes<'a, N: DataSourceNode<'a>>(
    node: N,
    seen: &mut HashSet<N>,
    results: &mut Vec<N>,
) {
    if let Some(parent) = node.parent() {
        add_node(parent, seen, results);
    }
}

pub fn collect_ancestor_nodes<'a, N: DataSourceNode<'a>>(
    node: N,
    seen: &mut HashSet<N>,
    results: &mut Vec<N>,
) {
    let mut current = node.parent();
    while let Some(p) = current {
        add_node(p, seen, results);
        current = p.parent();
    }
}

pub fn collect_following_sibling_nodes<'a, N: DataSourceNode<'a>>(
    node: N,
    seen: &mut HashSet<N>,
    results: &mut Vec<N>,
) {
    if let Some(parent) = node.parent() {
        let mut found_self = false;
        for sibling in parent.children() {
            if found_self {
                add_node(sibling, seen, results);
            }
            if sibling == node {
                found_self = true;
            }
        }
    }
}

pub fn collect_preceding_sibling_nodes<'a, N: DataSourceNode<'a>>(
    node: N,
    seen: &mut HashSet<N>,
    results: &mut Vec<N>,
) {
    if let Some(parent) = node.parent() {
        let mut siblings = Vec::new();
        for sibling in parent.children() {
            if sibling == node {
                break;
            }
            siblings.push(sibling);
        }
        for sibling in siblings {
            add_node(sibling, seen, results);
        }
    }
}

pub fn collect_following_nodes<'a, N: DataSourceNode<'a>>(
    node: N,
    seen: &mut HashSet<N>,
    results: &mut Vec<N>,
) {
    let mut current = Some(node);
    while let Some(c) = current {
        let parent = c.parent();
        if let Some(p) = parent {
            let mut found_c = false;
            for sibling in p.children() {
                if found_c {
                    collect_descendant_or_self_nodes(sibling, seen, results);
                }
                if sibling == c {
                    found_c = true;
                }
            }
        }
        current = parent;
    }
}

pub fn collect_preceding_nodes<'a, N: DataSourceNode<'a>>(
    node: N,
    seen: &mut HashSet<N>,
    results: &mut Vec<N>,
) {
    let mut current = Some(node);
    while let Some(c) = current {
        let parent = c.parent();
        if let Some(p) = parent {
            for sibling in p.children() {
                if sibling == c {
                    break;
                }
                collect_descendant_or_self_nodes(sibling, seen, results);
            }
        }
        current = parent;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datasource::tests::{MockNode, create_test_tree};

    #[test]
    fn test_collect_child() {
        let tree = create_test_tree();
        let root = MockNode { id: 0, tree: &tree };
        let para1 = MockNode { id: 1, tree: &tree };
        let comment = MockNode { id: 8, tree: &tree };
        let div = MockNode { id: 5, tree: &tree };
        let pi = MockNode { id: 9, tree: &tree };
        let para2 = MockNode { id: 6, tree: &tree };
        let mut seen = HashSet::new();
        let mut results = Vec::new();

        collect_child_nodes(root, &mut seen, &mut results);
        assert_eq!(results, vec![para1, comment, div, pi, para2]);
    }

    #[test]
    fn test_collect_ancestor() {
        let tree = create_test_tree();
        let root = MockNode { id: 0, tree: &tree };
        let para = MockNode { id: 1, tree: &tree };
        let text = MockNode { id: 4, tree: &tree };
        let mut seen = HashSet::new();
        let mut results = Vec::new();

        collect_ancestor_nodes(text, &mut seen, &mut results);
        assert_eq!(results, vec![para, root]);
    }

    #[test]
    fn test_collect_descendant() {
        let tree = create_test_tree();
        let root = MockNode { id: 0, tree: &tree };
        let para1 = MockNode { id: 1, tree: &tree };
        let text1 = MockNode { id: 4, tree: &tree };
        let div = MockNode { id: 5, tree: &tree };
        let para2 = MockNode { id: 6, tree: &tree };
        let text2 = MockNode { id: 7, tree: &tree };
        let comment = MockNode { id: 8, tree: &tree };
        let pi = MockNode { id: 9, tree: &tree };
        let mut seen = HashSet::new();
        let mut results = Vec::new();

        collect_descendant_nodes(root, &mut seen, &mut results);
        results.sort();
        assert_eq!(results, vec![para1, text1, div, para2, text2, comment, pi]);
    }

    #[test]
    fn test_collect_siblings() {
        let tree = create_test_tree();
        let para1 = MockNode { id: 1, tree: &tree };
        let div = MockNode { id: 5, tree: &tree };
        let para2 = MockNode { id: 6, tree: &tree };
        let comment = MockNode { id: 8, tree: &tree };
        let pi = MockNode { id: 9, tree: &tree };

        let mut seen = HashSet::new();
        let mut following = Vec::new();
        collect_following_sibling_nodes(para1, &mut seen, &mut following);
        assert_eq!(following, vec![comment, div, pi, para2]);

        seen.clear();
        let mut preceding = Vec::new();
        collect_preceding_sibling_nodes(para2, &mut seen, &mut preceding);
        assert_eq!(preceding, vec![para1, comment, div, pi]);
    }

    #[test]
    fn test_collect_following_preceding() {
        let tree = create_test_tree();
        let text1 = MockNode { id: 4, tree: &tree };
        let div = MockNode { id: 5, tree: &tree };
        let para2 = MockNode { id: 6, tree: &tree };
        let text2 = MockNode { id: 7, tree: &tree };
        let comment = MockNode { id: 8, tree: &tree };
        let pi = MockNode { id: 9, tree: &tree };

        let mut seen = HashSet::new();
        let mut following = Vec::new();
        // The following of the text node "Hello" (id 4) are all its parent's following siblings and their descendants.
        collect_following_nodes(text1, &mut seen, &mut following);
        following.sort();
        assert_eq!(following, vec![div, para2, text2, comment, pi]);

        seen.clear();
        let para1 = MockNode { id: 1, tree: &tree };
        let mut preceding = Vec::new();
        // The preceding of the div (id 5) are all its preceding siblings and their descendants.
        collect_preceding_nodes(div, &mut seen, &mut preceding);
        preceding.sort();
        assert_eq!(preceding, vec![para1, text1, comment]);
    }
}
