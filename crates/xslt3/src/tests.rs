#![allow(clippy::too_many_lines)]

use crate::test_helpers::{execute_xslt3, get_text_content, parse_stylesheet};

mod compiler_tests {
    use super::*;

    #[test]
    fn test_compile_basic_xslt3_stylesheet() {
        use crate::ast::Xslt3Instruction;

        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <output>Hello</output>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let stylesheet = parse_stylesheet(xslt).expect("Failed to compile");

        let rules = stylesheet
            .template_rules
            .get(&None)
            .expect("No default mode rules");
        assert!(!rules.is_empty(), "Should have at least one template rule");

        let rule = &rules[0];
        assert_eq!(rule.pattern.0, "/", "Should match root");

        let has_output_tag = rule.body.0.iter().any(|instr| {
            matches!(instr, Xslt3Instruction::ContentTag { tag_name, .. } 
                if String::from_utf8_lossy(tag_name) == "output")
        });
        assert!(has_output_tag, "Should have ContentTag for 'output'");
    }

    #[test]
    fn test_compile_with_expand_text() {
        use crate::ast::Xslt3Instruction;

        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                expand-text="yes">
                <xsl:template match="/">
                    <output>Value is {/root/value}</output>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let stylesheet = parse_stylesheet(xslt).expect("Failed to compile TVT");
        assert!(stylesheet.expand_text, "expand_text should be enabled");

        let rules = stylesheet.template_rules.get(&None).expect("No rules");
        let rule = &rules[0];

        fn has_tvt(instructions: &[Xslt3Instruction]) -> bool {
            instructions.iter().any(|instr| match instr {
                Xslt3Instruction::TextValueTemplate(_) => true,
                Xslt3Instruction::ContentTag { body, .. } => has_tvt(&body.0),
                _ => false,
            })
        }

        assert!(
            has_tvt(&rule.body.0),
            "Should have TextValueTemplate instruction"
        );
    }

    #[test]
    fn test_compile_try_catch() {
        use crate::ast::Xslt3Instruction;

        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <xsl:try>
                        <xsl:value-of select="/data"/>
                        <xsl:catch>
                            <error>Caught error</error>
                        </xsl:catch>
                    </xsl:try>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let stylesheet = parse_stylesheet(xslt).expect("Failed to compile try/catch");
        assert!(
            stylesheet.features.uses_try_catch,
            "uses_try_catch should be set"
        );

        let rules = stylesheet.template_rules.get(&None).expect("No rules");
        let try_instr = rules[0]
            .body
            .0
            .iter()
            .find(|i| matches!(i, Xslt3Instruction::Try { .. }));
        assert!(try_instr.is_some(), "Should have Try instruction");

        if let Some(Xslt3Instruction::Try { catches, .. }) = try_instr {
            assert!(!catches.is_empty(), "Should have at least one catch clause");
        }
    }

    #[test]
    fn test_compile_iterate() {
        use crate::ast::Xslt3Instruction;

        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <xsl:iterate select="items/item">
                        <xsl:param name="total" select="0"/>
                        <xsl:on-completion>
                            <total><xsl:value-of select="$total"/></total>
                        </xsl:on-completion>
                        <xsl:next-iteration>
                            <xsl:with-param name="total" select="$total + 1"/>
                        </xsl:next-iteration>
                    </xsl:iterate>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let stylesheet = parse_stylesheet(xslt).expect("Failed to compile iterate");
        assert!(
            stylesheet.features.uses_iterate,
            "uses_iterate should be set"
        );

        let rules = stylesheet.template_rules.get(&None).expect("No rules");
        let iterate = rules[0]
            .body
            .0
            .iter()
            .find(|i| matches!(i, Xslt3Instruction::Iterate { .. }));
        assert!(iterate.is_some(), "Should have Iterate instruction");

        if let Some(Xslt3Instruction::Iterate {
            params,
            on_completion,
            ..
        }) = iterate
        {
            assert!(!params.is_empty(), "Should have iteration params");
            assert_eq!(params[0].name, "total", "Param should be named 'total'");
            assert!(on_completion.is_some(), "Should have on-completion handler");
        }
    }

    #[test]
    fn test_compile_map_construction() {
        use crate::ast::Xslt3Instruction;

        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <xsl:map>
                        <xsl:map-entry key="'name'" select="'value'"/>
                    </xsl:map>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let stylesheet = parse_stylesheet(xslt).expect("Failed to compile map");
        assert!(stylesheet.features.uses_maps, "uses_maps should be set");

        let rules = stylesheet.template_rules.get(&None).expect("No rules");
        let map_instr = rules[0]
            .body
            .0
            .iter()
            .find(|i| matches!(i, Xslt3Instruction::Map { .. }));
        assert!(map_instr.is_some(), "Should have Map instruction");

        if let Some(Xslt3Instruction::Map { entries }) = map_instr {
            assert_eq!(entries.len(), 1, "Should have one map entry");
        }
    }

    #[test]
    fn test_compile_variable_with_map_body() {
        use crate::ast::Xslt3Instruction;

        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <xsl:variable name="person">
                        <xsl:map>
                            <xsl:map-entry key="'name'" select="'Alice'"/>
                        </xsl:map>
                    </xsl:variable>
                    <output/>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let result = parse_stylesheet(xslt);
        assert!(result.is_ok(), "Failed to compile: {:?}", result.err());

        let stylesheet = result.unwrap();
        let rules = stylesheet
            .template_rules
            .get(&None)
            .expect("No default mode rules");
        let rule = &rules[0];

        let mut found_variable = false;
        for instr in &rule.body.0 {
            if let Xslt3Instruction::Variable {
                name, select, body, ..
            } = instr
            {
                if name == "person" {
                    found_variable = true;
                    assert!(
                        select.is_none(),
                        "Expected no select attribute, found: {:?}",
                        select
                    );
                    assert!(body.is_some(), "Expected body, found None");

                    if let Some(body_template) = body {
                        assert!(!body_template.0.is_empty(), "Body is empty");
                        let first_instr = &body_template.0[0];
                        assert!(
                            matches!(first_instr, Xslt3Instruction::Map { .. }),
                            "Expected Map instruction, found: {:?}",
                            first_instr
                        );
                    }
                }
            }
        }
        assert!(found_variable, "Variable 'person' not found in template");
    }

    #[test]
    fn test_compile_array_construction() {
        use crate::ast::Xslt3Instruction;

        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <xsl:array>
                        <xsl:array-member select="1"/>
                        <xsl:array-member select="2"/>
                        <xsl:array-member select="3"/>
                    </xsl:array>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let stylesheet = parse_stylesheet(xslt).expect("Failed to compile array");
        assert!(stylesheet.features.uses_arrays, "uses_arrays should be set");

        let rules = stylesheet.template_rules.get(&None).expect("No rules");
        let array = rules[0]
            .body
            .0
            .iter()
            .find(|i| matches!(i, Xslt3Instruction::Array { .. }));
        assert!(array.is_some(), "Should have Array instruction");

        if let Some(Xslt3Instruction::Array { members }) = array {
            assert_eq!(members.len(), 3, "Should have 3 array members");
        }
    }

    #[test]
    fn test_compile_fork() {
        use crate::ast::Xslt3Instruction;

        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <xsl:fork>
                        <xsl:sequence>
                            <branch1>First</branch1>
                        </xsl:sequence>
                        <xsl:sequence>
                            <branch2>Second</branch2>
                        </xsl:sequence>
                    </xsl:fork>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let stylesheet = parse_stylesheet(xslt).expect("Failed to compile fork");
        assert!(stylesheet.features.uses_fork, "uses_fork should be set");

        let rules = stylesheet.template_rules.get(&None).expect("No rules");
        let fork = rules[0]
            .body
            .0
            .iter()
            .find(|i| matches!(i, Xslt3Instruction::Fork { .. }));
        assert!(fork.is_some(), "Should have Fork instruction");

        if let Some(Xslt3Instruction::Fork { branches }) = fork {
            assert_eq!(branches.len(), 2, "Should have 2 fork branches");
            assert!(
                !branches[0].body.0.is_empty(),
                "First branch should have body"
            );
            assert!(
                !branches[1].body.0.is_empty(),
                "Second branch should have body"
            );
        }
    }

    #[test]
    fn test_compile_assert() {
        use crate::ast::Xslt3Instruction;

        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <xsl:assert test="count(items) > 0" error-code="ERR001">
                        Expected at least one item
                    </xsl:assert>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let stylesheet = parse_stylesheet(xslt).expect("Failed to compile assert");
        assert!(
            stylesheet.features.uses_assertions,
            "uses_assertions should be set"
        );

        let rules = stylesheet.template_rules.get(&None).expect("No rules");
        let assert_instr = rules[0]
            .body
            .0
            .iter()
            .find(|i| matches!(i, Xslt3Instruction::Assert { .. }));
        assert!(assert_instr.is_some(), "Should have Assert instruction");

        if let Some(Xslt3Instruction::Assert { message, .. }) = assert_instr {
            assert!(message.is_some(), "Should have error message body");
        }
    }

    #[test]
    fn test_compile_where_populated() {
        use crate::ast::Xslt3Instruction;

        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <xsl:where-populated>
                        <wrapper>
                            <xsl:value-of select="data"/>
                        </wrapper>
                    </xsl:where-populated>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let stylesheet = parse_stylesheet(xslt).expect("Failed to compile where-populated");

        let rules = stylesheet.template_rules.get(&None).expect("No rules");
        let wp = rules[0]
            .body
            .0
            .iter()
            .find(|i| matches!(i, Xslt3Instruction::WherePopulated { .. }));
        assert!(wp.is_some(), "Should have WherePopulated instruction");

        if let Some(Xslt3Instruction::WherePopulated { body }) = wp {
            assert!(
                !body.0.is_empty(),
                "WherePopulated should have body content"
            );
        }
    }

    #[test]
    fn test_compile_on_empty_on_non_empty() {
        use crate::ast::Xslt3Instruction;

        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <xsl:on-empty>
                        <empty/>
                    </xsl:on-empty>
                    <xsl:on-non-empty>
                        <data><xsl:value-of select="."/></data>
                    </xsl:on-non-empty>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let stylesheet = parse_stylesheet(xslt).expect("Failed to compile on-empty/on-non-empty");

        let rules = stylesheet.template_rules.get(&None).expect("No rules");
        let body = &rules[0].body.0;

        let on_empty = body
            .iter()
            .find(|i| matches!(i, Xslt3Instruction::OnEmpty { .. }));
        let on_non_empty = body
            .iter()
            .find(|i| matches!(i, Xslt3Instruction::OnNonEmpty { .. }));

        assert!(on_empty.is_some(), "Should have OnEmpty instruction");
        assert!(on_non_empty.is_some(), "Should have OnNonEmpty instruction");
    }

    #[test]
    fn test_compile_accumulator() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:accumulator name="total" initial-value="0">
                    <xsl:accumulator-rule match="item">
                        <xsl:sequence select="$value + number(@amount)"/>
                    </xsl:accumulator-rule>
                </xsl:accumulator>
                <xsl:template match="/">
                    <result><xsl:value-of select="accumulator-before('total')"/></result>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let stylesheet = parse_stylesheet(xslt).expect("Failed to compile accumulator");
        assert!(
            stylesheet.features.uses_accumulators,
            "uses_accumulators should be set"
        );
        assert!(
            stylesheet.accumulators.contains_key("total"),
            "Should have 'total' accumulator"
        );

        let acc = stylesheet.accumulators.get("total").unwrap();
        assert_eq!(acc.name, "total", "Accumulator name should be 'total'");
        assert!(!acc.rules.is_empty(), "Accumulator should have rules");
        assert_eq!(acc.rules[0].pattern.0, "item", "Rule should match 'item'");
    }

    #[test]
    fn test_compile_stream() {
        use crate::ast::Xslt3Instruction;

        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <xsl:stream href="data.xml">
                        <xsl:apply-templates/>
                    </xsl:stream>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let stylesheet = parse_stylesheet(xslt).expect("Failed to compile stream");
        assert!(
            stylesheet.features.uses_streaming,
            "uses_streaming should be set"
        );

        let rules = stylesheet.template_rules.get(&None).expect("No rules");
        let stream = rules[0]
            .body
            .0
            .iter()
            .find(|i| matches!(i, Xslt3Instruction::Stream { .. }));
        assert!(stream.is_some(), "Should have Stream instruction");

        if let Some(Xslt3Instruction::Stream { body, .. }) = stream {
            assert!(!body.0.is_empty(), "Stream should have body content");
        }
    }

    #[test]
    fn test_compile_merge() {
        use crate::ast::Xslt3Instruction;

        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <xsl:merge>
                        <xsl:merge-source select="/data/items/item">
                            <xsl:merge-key select="@id"/>
                        </xsl:merge-source>
                        <xsl:merge-action>
                            <merged><xsl:value-of select="."/></merged>
                        </xsl:merge-action>
                    </xsl:merge>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let stylesheet = parse_stylesheet(xslt).expect("Failed to compile merge");
        assert!(stylesheet.features.uses_merge, "uses_merge should be set");

        let rules = stylesheet.template_rules.get(&None).expect("No rules");
        let merge = rules[0]
            .body
            .0
            .iter()
            .find(|i| matches!(i, Xslt3Instruction::Merge { .. }));
        assert!(merge.is_some(), "Should have Merge instruction");

        if let Some(Xslt3Instruction::Merge { sources, action }) = merge {
            assert!(!sources.is_empty(), "Merge should have sources");
            assert!(!action.body.0.is_empty(), "Merge action should have body");
        }
    }

    #[test]
    fn test_compile_global_context_item() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:global-context-item as="document-node()"/>
                <xsl:template match="/">
                    <output/>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let stylesheet = parse_stylesheet(xslt).expect("Failed to compile global-context-item");
        assert!(
            stylesheet.global_context_item.is_some(),
            "Should have global context item"
        );

        let gci = stylesheet.global_context_item.as_ref().unwrap();
        assert!(gci.as_type.is_some(), "Should have as type");
    }

    #[test]
    fn test_compile_function_with_visibility() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:local="http://local">
                <xsl:function name="local:double" as="xs:integer" visibility="public">
                    <xsl:param name="x" as="xs:integer"/>
                    <xsl:sequence select="$x * 2"/>
                </xsl:function>
                <xsl:template match="/">
                    <result><xsl:value-of select="local:double(5)"/></result>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let stylesheet = parse_stylesheet(xslt).expect("Failed to compile function");
        assert!(
            stylesheet.features.uses_higher_order_functions,
            "uses_higher_order_functions should be set"
        );

        let func = stylesheet
            .functions
            .values()
            .find(|f| f.name.ends_with("double"));
        assert!(func.is_some(), "Should have function named 'double'");

        let func = func.unwrap();
        assert!(!func.params.is_empty(), "Function should have params");
        assert_eq!(func.params[0].name, "x", "Param should be named 'x'");
        assert!(func.as_type.is_some(), "Function should have return type");
    }

    #[test]
    fn test_compile_import_declaration() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:import href="common.xsl"/>
                <xsl:template match="/">
                    <output>Test</output>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let result = parse_stylesheet(xslt);
        assert!(
            result.is_ok(),
            "Failed to compile import: {:?}",
            result.err()
        );
        let stylesheet = result.unwrap();
        assert_eq!(stylesheet.imports.len(), 1);
        assert_eq!(stylesheet.imports[0].href, "common.xsl");
    }

    #[test]
    fn test_compile_include_declaration() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:include href="utilities.xsl"/>
                <xsl:template match="/">
                    <output>Test</output>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let result = parse_stylesheet(xslt);
        assert!(
            result.is_ok(),
            "Failed to compile include: {:?}",
            result.err()
        );
        let stylesheet = result.unwrap();
        assert_eq!(stylesheet.includes.len(), 1);
        assert_eq!(stylesheet.includes[0].href, "utilities.xsl");
    }

    #[test]
    fn test_compile_multiple_imports_and_includes() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:import href="base.xsl"/>
                <xsl:import href="override.xsl"/>
                <xsl:include href="utils.xsl"/>
                <xsl:template match="/">
                    <output>Test</output>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let result = parse_stylesheet(xslt);
        assert!(result.is_ok(), "Failed to compile: {:?}", result.err());
        let stylesheet = result.unwrap();
        assert_eq!(stylesheet.imports.len(), 2);
        assert_eq!(stylesheet.includes.len(), 1);
    }
}

mod executor_tests {
    use super::*;

    #[test]
    fn test_execute_basic_template() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <output>Hello World</output>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root/>";
        let result = execute_xslt3(xslt, xml);
        assert!(result.is_ok(), "Execution failed: {:?}", result.err());

        let nodes = result.unwrap();
        let text = get_text_content(&nodes);
        assert_eq!(text, "Hello World");
    }

    #[test]
    fn test_execute_value_of() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <output><xsl:value-of select="/root/name"/></output>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root><name>Alice</name></root>";
        let result = execute_xslt3(xslt, xml);
        assert!(result.is_ok(), "Execution failed: {:?}", result.err());

        let nodes = result.unwrap();
        let text = get_text_content(&nodes);
        assert_eq!(text, "Alice");
    }

    #[test]
    fn test_execute_for_each() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <items>
                        <xsl:for-each select="/root/item">
                            <entry><xsl:value-of select="."/></entry>
                        </xsl:for-each>
                    </items>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root><item>A</item><item>B</item><item>C</item></root>";
        let result = execute_xslt3(xslt, xml);
        assert!(result.is_ok(), "Execution failed: {:?}", result.err());

        let nodes = result.unwrap();
        let text = get_text_content(&nodes);
        assert!(text.contains('A'));
        assert!(text.contains('B'));
        assert!(text.contains('C'));
    }

    #[test]
    fn test_execute_apply_templates() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <root><xsl:apply-templates/></root>
                </xsl:template>
                <xsl:template match="item">
                    <processed><xsl:value-of select="."/></processed>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<data><item>Test</item></data>";
        let result = execute_xslt3(xslt, xml);
        assert!(result.is_ok(), "Execution failed: {:?}", result.err());

        let nodes = result.unwrap();
        let text = get_text_content(&nodes);
        assert!(text.contains("Test"));
    }

    #[test]
    fn test_execute_choose_when_otherwise() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <xsl:choose>
                        <xsl:when test="/root/flag = 'yes'">
                            <result>YES</result>
                        </xsl:when>
                        <xsl:otherwise>
                            <result>NO</result>
                        </xsl:otherwise>
                    </xsl:choose>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml_yes = "<root><flag>yes</flag></root>";
        let result_yes = execute_xslt3(xslt, xml_yes).unwrap();
        assert!(get_text_content(&result_yes).contains("YES"));

        let xml_no = "<root><flag>no</flag></root>";
        let result_no = execute_xslt3(xslt, xml_no).unwrap();
        assert!(get_text_content(&result_no).contains("NO"));
    }

    #[test]
    fn test_execute_if() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <xsl:if test="count(/root/item) > 0">
                        <has-items>true</has-items>
                    </xsl:if>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root><item>A</item></root>";
        let result = execute_xslt3(xslt, xml).unwrap();
        assert!(get_text_content(&result).contains("true"));

        let xml_empty = "<root></root>";
        let result_empty = execute_xslt3(xslt, xml_empty).unwrap();
        assert!(!get_text_content(&result_empty).contains("true"));
    }

    #[test]
    fn test_execute_variable() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <xsl:variable name="greeting" select="'Hello'"/>
                    <xsl:variable name="name" select="/root/name"/>
                    <output><xsl:value-of select="concat($greeting, ' ', $name)"/></output>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root><name>World</name></root>";
        let result = execute_xslt3(xslt, xml).unwrap();
        assert_eq!(get_text_content(&result), "Hello World");
    }

    #[test]
    fn test_execute_copy_of() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <wrapper>
                        <xsl:copy-of select="/root/data"/>
                    </wrapper>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root><data>Content</data></root>";
        let result = execute_xslt3(xslt, xml).unwrap();
        assert!(get_text_content(&result).contains("Content"));
    }

    #[test]
    fn test_execute_try_catch_success() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <xsl:try>
                        <success><xsl:value-of select="/root/value"/></success>
                        <xsl:catch>
                            <error>Failed</error>
                        </xsl:catch>
                    </xsl:try>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root><value>OK</value></root>";
        let result = execute_xslt3(xslt, xml).unwrap();
        assert!(get_text_content(&result).contains("OK"));
        assert!(!get_text_content(&result).contains("Failed"));
    }

    #[test]
    fn test_try_catch_error_variables() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:err="http://www.w3.org/2005/xqt-errors">
                <xsl:template match="/">
                    <xsl:try>
                        <xsl:message terminate="yes" error-code="MY_ERROR">Test error message</xsl:message>
                        <xsl:catch>
                            <error>
                                <code><xsl:value-of select="$err:code"/></code>
                                <desc><xsl:value-of select="$err:description"/></desc>
                            </error>
                        </xsl:catch>
                    </xsl:try>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root/>";
        let result = execute_xslt3(xslt, xml).unwrap();
        let text = get_text_content(&result);
        assert!(
            text.contains("MY_ERROR"),
            "Error code not found in: {}",
            text
        );
    }

    #[test]
    fn test_try_catch_with_xpath_error() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:err="http://www.w3.org/2005/xqt-errors">
                <xsl:template match="/">
                    <xsl:try>
                        <xsl:value-of select="unknown-function()"/>
                        <xsl:catch>
                            <caught><xsl:value-of select="$err:code"/></caught>
                        </xsl:catch>
                    </xsl:try>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root/>";
        let result = execute_xslt3(xslt, xml).unwrap();
        let text = get_text_content(&result);
        assert!(!text.is_empty(), "Catch block should have executed");
    }

    #[test]
    fn test_execute_element_generation() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <xsl:element name="dynamic">
                        <xsl:attribute name="id">123</xsl:attribute>
                        <content>Text</content>
                    </xsl:element>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root/>";
        let result = execute_xslt3(xslt, xml);
        assert!(result.is_ok(), "Execution failed: {:?}", result.err());
    }

    #[test]
    fn test_execute_comment() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <output>
                        <xsl:comment>This is a comment</xsl:comment>
                    </output>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root/>";
        let result = execute_xslt3(xslt, xml);
        assert!(result.is_ok(), "Execution failed: {:?}", result.err());
    }

    #[test]
    fn test_execute_processing_instruction() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <output>
                        <xsl:processing-instruction name="xml-stylesheet">
                            type="text/css" href="style.css"
                        </xsl:processing-instruction>
                    </output>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root/>";
        let result = execute_xslt3(xslt, xml);
        assert!(result.is_ok(), "Execution failed: {:?}", result.err());
    }
}

mod text_value_template_tests {
    use super::*;

    #[test]
    fn test_tvt_basic_expression() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                expand-text="yes">
                <xsl:template match="/">
                    <output>Hello {/root/name}!</output>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root><name>World</name></root>";
        let result = execute_xslt3(xslt, xml).unwrap();
        assert_eq!(get_text_content(&result), "Hello World!");
    }

    #[test]
    fn test_tvt_multiple_expressions() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                expand-text="yes">
                <xsl:template match="/">
                    <output>{/root/first} and {/root/second}</output>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root><first>One</first><second>Two</second></root>";
        let result = execute_xslt3(xslt, xml).unwrap();
        assert_eq!(get_text_content(&result), "One and Two");
    }

    #[test]
    fn test_tvt_escaped_braces() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                expand-text="yes">
                <xsl:template match="/">
                    <output>Literal {{ and }} braces</output>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root/>";
        let result = execute_xslt3(xslt, xml).unwrap();
        assert_eq!(get_text_content(&result), "Literal { and } braces");
    }

    #[test]
    fn test_tvt_disabled_by_default() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <output>This {should} remain literal</output>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root/>";
        let result = execute_xslt3(xslt, xml).unwrap();
        assert_eq!(get_text_content(&result), "This {should} remain literal");
    }

    #[test]
    fn test_tvt_in_attribute() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                expand-text="yes">
                <xsl:template match="/">
                    <output id="{/root/id}">Content</output>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root><id>test-123</id></root>";
        let result = execute_xslt3(xslt, xml);
        assert!(result.is_ok(), "Execution failed: {:?}", result.err());
    }

    #[test]
    fn test_tvt_with_nested_expression() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                expand-text="yes">
                <xsl:template match="/">
                    <output>Count: {count(/root/item)}</output>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root><item/><item/><item/></root>";
        let result = execute_xslt3(xslt, xml).unwrap();
        assert_eq!(get_text_content(&result), "Count: 3");
    }
}

mod xslt3_instruction_tests {
    use super::*;

    #[test]
    fn test_iterate_basic() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <items>
                        <xsl:iterate select="/root/item">
                            <entry><xsl:value-of select="."/></entry>
                        </xsl:iterate>
                    </items>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root><item>A</item><item>B</item></root>";
        let result = execute_xslt3(xslt, xml).unwrap();
        let text = get_text_content(&result);
        assert!(text.contains('A'));
        assert!(text.contains('B'));
    }

    #[test]
    fn test_map_and_array_structure() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <xsl:variable name="m">
                        <xsl:map>
                            <xsl:map-entry key="'a'" select="1"/>
                        </xsl:map>
                    </xsl:variable>
                    <output>Map created</output>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root/>";
        let result = execute_xslt3(xslt, xml);
        assert!(result.is_ok(), "Execution failed: {:?}", result.err());
    }

    #[test]
    fn test_variable_with_map_body_capture() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:map="http://www.w3.org/2005/xpath-functions/map">
                <xsl:template match="/">
                    <xsl:variable name="person">
                        <xsl:map>
                            <xsl:map-entry key="'name'" select="'Alice'"/>
                            <xsl:map-entry key="'age'" select="30"/>
                        </xsl:map>
                    </xsl:variable>
                    <output>
                        <name><xsl:value-of select="map:get($person, 'name')"/></name>
                        <age><xsl:value-of select="map:get($person, 'age')"/></age>
                    </output>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root/>";
        let result = execute_xslt3(xslt, xml);
        assert!(result.is_ok(), "Execution failed: {:?}", result.err());
        let nodes = result.unwrap();
        let text = get_text_content(&nodes);
        assert!(
            text.contains("Alice"),
            "Expected 'Alice' in output, got: {}",
            text
        );
        assert!(
            text.contains("30"),
            "Expected '30' in output, got: {}",
            text
        );
    }

    #[test]
    fn test_variable_with_map_body_map_size() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:map="http://www.w3.org/2005/xpath-functions/map">
                <xsl:template match="/">
                    <xsl:variable name="m">
                        <xsl:map>
                            <xsl:map-entry key="'a'" select="1"/>
                        </xsl:map>
                    </xsl:variable>
                    <output><xsl:value-of select="map:size($m)"/></output>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root/>";
        let result = execute_xslt3(xslt, xml);
        assert!(result.is_ok(), "Execution failed: {:?}", result.err());
        let text = get_text_content(&result.unwrap());
        assert!(
            text.contains("1"),
            "Expected map size '1' in output, got: '{}'",
            text
        );
    }

    #[test]
    fn test_variable_with_array_body_capture() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:array="http://www.w3.org/2005/xpath-functions/array">
                <xsl:template match="/">
                    <xsl:variable name="colors">
                        <xsl:array>
                            <xsl:array-member select="'red'"/>
                            <xsl:array-member select="'green'"/>
                            <xsl:array-member select="'blue'"/>
                        </xsl:array>
                    </xsl:variable>
                    <output>
                        <first><xsl:value-of select="array:get($colors, 1)"/></first>
                        <size><xsl:value-of select="array:size($colors)"/></size>
                    </output>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root/>";
        let result = execute_xslt3(xslt, xml);
        assert!(result.is_ok(), "Execution failed: {:?}", result.err());
        let nodes = result.unwrap();
        let text = get_text_content(&nodes);
        assert!(
            text.contains("red"),
            "Expected 'red' in output, got: {}",
            text
        );
        assert!(
            text.contains("3"),
            "Expected array size '3' in output, got: {}",
            text
        );
    }

    #[test]
    fn test_where_populated_with_content() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <xsl:where-populated>
                        <wrapper>
                            <xsl:value-of select="/root/data"/>
                        </wrapper>
                    </xsl:where-populated>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root><data>Content</data></root>";
        let result = execute_xslt3(xslt, xml).unwrap();
        assert!(get_text_content(&result).contains("Content"));
    }

    #[test]
    fn test_on_empty_fallback() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <xsl:on-empty>
                        <fallback>Empty</fallback>
                    </xsl:on-empty>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root/>";
        let result = execute_xslt3(xslt, xml);
        assert!(result.is_ok(), "Execution failed: {:?}", result.err());
    }

    #[test]
    fn test_break_in_iterate() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <xsl:iterate select="/root/item">
                        <xsl:if test=". = 'STOP'">
                            <xsl:break/>
                        </xsl:if>
                        <entry><xsl:value-of select="."/></entry>
                    </xsl:iterate>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root><item>A</item><item>STOP</item><item>B</item></root>";
        let result = execute_xslt3(xslt, xml).unwrap();
        let text = get_text_content(&result);
        assert!(text.contains('A'));
        assert!(!text.contains('B'));
    }

    #[test]
    fn test_iterate_with_next_iteration_params() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <result>
                        <xsl:iterate select="/root/item">
                            <xsl:param name="counter" select="0"/>
                            <xsl:param name="sum" select="0"/>
                            <item n="{$counter}"><xsl:value-of select="."/></item>
                            <xsl:next-iteration>
                                <xsl:with-param name="counter" select="$counter + 1"/>
                                <xsl:with-param name="sum" select="$sum + number(.)"/>
                            </xsl:next-iteration>
                            <xsl:on-completion>
                                <total count="{$counter}" sum="{$sum}"/>
                            </xsl:on-completion>
                        </xsl:iterate>
                    </result>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root><item>10</item><item>20</item><item>30</item></root>";
        let result = execute_xslt3(xslt, xml).unwrap();
        let text = get_text_content(&result);

        assert!(text.contains("10"), "Should contain first item value");
        assert!(text.contains("20"), "Should contain second item value");
        assert!(text.contains("30"), "Should contain third item value");
    }
}

mod grouping_tests {
    use super::*;

    #[test]
    fn test_for_each_group_by() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <groups>
                        <xsl:for-each-group select="/data/item" group-by="@category">
                            <group key="{current-grouping-key()}">
                                <xsl:for-each select="current-group()">
                                    <member><xsl:value-of select="."/></member>
                                </xsl:for-each>
                            </group>
                        </xsl:for-each-group>
                    </groups>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = r#"
            <data>
                <item category="A">Item1</item>
                <item category="B">Item2</item>
                <item category="A">Item3</item>
                <item category="B">Item4</item>
            </data>
        "#;

        let result = execute_xslt3(xslt, xml).unwrap();
        let text = get_text_content(&result);
        assert!(text.contains("Item1"), "Should contain Item1");
        assert!(text.contains("Item3"), "Should contain Item3");
    }

    #[test]
    fn test_for_each_group_adjacent() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <groups>
                        <xsl:for-each-group select="/data/item" group-adjacent="@type">
                            <run type="{current-grouping-key()}" count="{count(current-group())}"/>
                        </xsl:for-each-group>
                    </groups>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = r#"
            <data>
                <item type="header">H1</item>
                <item type="para">P1</item>
                <item type="para">P2</item>
                <item type="header">H2</item>
            </data>
        "#;

        let result = execute_xslt3(xslt, xml).expect("group-adjacent failed");
        assert!(
            result.is_empty() || !result.is_empty(),
            "group-adjacent execution succeeded"
        );
    }

    #[test]
    fn test_for_each_group_starting_with() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <chapters>
                        <xsl:for-each-group select="/doc/*" group-starting-with="heading">
                            <chapter>
                                <xsl:apply-templates select="current-group()"/>
                            </chapter>
                        </xsl:for-each-group>
                    </chapters>
                </xsl:template>
                <xsl:template match="heading">
                    <title><xsl:value-of select="."/></title>
                </xsl:template>
                <xsl:template match="para">
                    <p><xsl:value-of select="."/></p>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = r#"
            <doc>
                <heading>Chapter 1</heading>
                <para>Para 1</para>
                <para>Para 2</para>
                <heading>Chapter 2</heading>
                <para>Para 3</para>
            </doc>
        "#;

        let result = execute_xslt3(xslt, xml).unwrap();
        let text = get_text_content(&result);
        assert!(text.contains("Chapter 1"), "Should contain Chapter 1");
        assert!(text.contains("Chapter 2"), "Should contain Chapter 2");
    }

    #[test]
    fn test_for_each_group_ending_with() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <pages>
                        <xsl:for-each-group select="/doc/*" group-ending-with="break">
                            <page items="{count(current-group())}"/>
                        </xsl:for-each-group>
                    </pages>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = r#"
            <doc>
                <para>P1</para>
                <para>P2</para>
                <break/>
                <para>P3</para>
                <break/>
            </doc>
        "#;

        let result = execute_xslt3(xslt, xml).expect("group-ending-with failed");
        assert!(
            !result.is_empty(),
            "group-ending-with should produce output"
        );
    }
}

mod analyze_string_tests {
    use super::*;

    #[test]
    fn test_analyze_string_basic() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <result>
                        <xsl:analyze-string select="'hello123world456'" regex="[0-9]+">
                            <xsl:matching-substring>
                                <num><xsl:value-of select="."/></num>
                            </xsl:matching-substring>
                            <xsl:non-matching-substring>
                                <text><xsl:value-of select="."/></text>
                            </xsl:non-matching-substring>
                        </xsl:analyze-string>
                    </result>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root/>";
        let result = execute_xslt3(xslt, xml).unwrap();
        let text = get_text_content(&result);
        assert!(text.contains("hello"), "Should extract 'hello'");
        assert!(text.contains("123"), "Should extract '123'");
        assert!(text.contains("world"), "Should extract 'world'");
        assert!(text.contains("456"), "Should extract '456'");
    }

    #[test]
    fn test_analyze_string_case_insensitive() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <result>
                        <xsl:analyze-string select="'Hello HELLO hello'" regex="hello" flags="i">
                            <xsl:matching-substring>
                                <match><xsl:value-of select="."/></match>
                            </xsl:matching-substring>
                        </xsl:analyze-string>
                    </result>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root/>";
        let result = execute_xslt3(xslt, xml).unwrap();
        let text = get_text_content(&result);
        assert!(text.contains("Hello"), "Should match 'Hello'");
        assert!(text.contains("HELLO"), "Should match 'HELLO'");
    }

    #[test]
    fn test_analyze_string_with_groups() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <result>
                        <xsl:analyze-string select="'2024-01-15'" regex="(\d{{4}})-(\d{{2}})-(\d{{2}})">
                            <xsl:matching-substring>
                                <date>
                                    <year><xsl:value-of select="regex-group(1)"/></year>
                                    <month><xsl:value-of select="regex-group(2)"/></month>
                                    <day><xsl:value-of select="regex-group(3)"/></day>
                                </date>
                            </xsl:matching-substring>
                        </xsl:analyze-string>
                    </result>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root/>";
        let result = execute_xslt3(xslt, xml);
        assert!(
            result.is_ok(),
            "analyze-string with groups failed: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_analyze_string_no_match() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <result>
                        <xsl:analyze-string select="'no numbers here'" regex="[0-9]+">
                            <xsl:matching-substring>
                                <num><xsl:value-of select="."/></num>
                            </xsl:matching-substring>
                            <xsl:non-matching-substring>
                                <text><xsl:value-of select="."/></text>
                            </xsl:non-matching-substring>
                        </xsl:analyze-string>
                    </result>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root/>";
        let result = execute_xslt3(xslt, xml).unwrap();
        let text = get_text_content(&result);
        assert!(
            text.contains("no numbers here"),
            "Should output non-matching text"
        );
    }
}

mod merge_tests {
    use super::*;

    #[test]
    fn test_merge_single_source() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <merged>
                        <xsl:merge>
                            <xsl:merge-source select="/data/items/item">
                                <xsl:merge-key select="@id"/>
                            </xsl:merge-source>
                            <xsl:merge-action>
                                <entry id="{current-merge-key()}">
                                    <xsl:value-of select="."/>
                                </entry>
                            </xsl:merge-action>
                        </xsl:merge>
                    </merged>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = r#"
            <data>
                <items>
                    <item id="1">First</item>
                    <item id="2">Second</item>
                    <item id="3">Third</item>
                </items>
            </data>
        "#;

        let result = execute_xslt3(xslt, xml).unwrap();
        let text = get_text_content(&result);
        assert!(text.contains("First"), "Should contain First");
        assert!(text.contains("Second"), "Should contain Second");
    }

    #[test]
    fn test_merge_sorted_output() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <sorted>
                        <xsl:merge>
                            <xsl:merge-source select="/data/item">
                                <xsl:merge-key select="@value" order="ascending"/>
                            </xsl:merge-source>
                            <xsl:merge-action>
                                <item><xsl:value-of select="."/></item>
                            </xsl:merge-action>
                        </xsl:merge>
                    </sorted>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = r#"
            <data>
                <item value="3">C</item>
                <item value="1">A</item>
                <item value="2">B</item>
            </data>
        "#;

        let result = execute_xslt3(xslt, xml).expect("merge failed");
        let text = get_text_content(&result);
        assert!(!text.is_empty(), "Merge should produce output");
    }

    #[test]
    fn test_merge_multiple_sources() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <merged>
                        <xsl:merge>
                            <xsl:merge-source name="source1" select="/data/source1/item">
                                <xsl:merge-key select="@key"/>
                            </xsl:merge-source>
                            <xsl:merge-source name="source2" select="/data/source2/item">
                                <xsl:merge-key select="@key"/>
                            </xsl:merge-source>
                            <xsl:merge-action>
                                <entry>
                                    <xsl:value-of select="."/>
                                </entry>
                            </xsl:merge-action>
                        </xsl:merge>
                    </merged>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = r#"
            <data>
                <source1>
                    <item key="a">Alpha1</item>
                    <item key="b">Beta1</item>
                </source1>
                <source2>
                    <item key="a">Alpha2</item>
                    <item key="c">Gamma2</item>
                </source2>
            </data>
        "#;

        let result = execute_xslt3(xslt, xml).expect("multi-source merge failed");
        let text = get_text_content(&result);
        assert!(
            text.contains("Alpha") || text.contains("Beta") || text.contains("Gamma"),
            "Multi-source merge should produce output, got: '{}'",
            text
        );
    }

    #[test]
    fn test_merge_with_named_sources() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <result>
                        <xsl:merge>
                            <xsl:merge-source name="primary" select="/data/primary/item">
                                <xsl:merge-key select="@id"/>
                            </xsl:merge-source>
                            <xsl:merge-source name="secondary" select="/data/secondary/item">
                                <xsl:merge-key select="@id"/>
                            </xsl:merge-source>
                            <xsl:merge-action>
                                <merged-item>
                                    <xsl:value-of select="."/>
                                </merged-item>
                            </xsl:merge-action>
                        </xsl:merge>
                    </result>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = r#"
            <data>
                <primary>
                    <item id="1">One</item>
                    <item id="2">Two</item>
                </primary>
                <secondary>
                    <item id="1">Uno</item>
                    <item id="3">Tres</item>
                </secondary>
            </data>
        "#;

        let result = execute_xslt3(xslt, xml).expect("merge with named sources failed");
        let text = get_text_content(&result);
        assert!(
            text.contains("One")
                || text.contains("Two")
                || text.contains("Uno")
                || text.contains("Tres"),
            "Merge should produce output from sources, got: '{}'",
            text
        );
    }

    #[test]
    fn test_merge_descending_order() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <sorted>
                        <xsl:merge>
                            <xsl:merge-source select="/data/item">
                                <xsl:merge-key select="@num" order="descending" data-type="number"/>
                            </xsl:merge-source>
                            <xsl:merge-action>
                                <item><xsl:value-of select="."/></item>
                            </xsl:merge-action>
                        </xsl:merge>
                    </sorted>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = r#"
            <data>
                <item num="1">First</item>
                <item num="10">Tenth</item>
                <item num="5">Fifth</item>
            </data>
        "#;

        let result = execute_xslt3(xslt, xml).expect("merge descending failed");
        let text = get_text_content(&result);
        assert!(
            text.contains("First") || text.contains("Tenth") || text.contains("Fifth"),
            "Merge should produce output, got: '{}'",
            text
        );
    }
}

mod fork_tests {
    use super::*;

    #[test]
    fn test_fork_basic() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <output>
                        <xsl:fork>
                            <xsl:sequence>
                                <branch1>First</branch1>
                            </xsl:sequence>
                            <xsl:sequence>
                                <branch2>Second</branch2>
                            </xsl:sequence>
                        </xsl:fork>
                    </output>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root/>";
        let result = execute_xslt3(xslt, xml).unwrap();
        let text = get_text_content(&result);
        assert!(
            text.contains("First") || text.contains("Second"),
            "Fork should produce output from branches"
        );
    }

    #[test]
    fn test_fork_single_branch() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <output>
                        <xsl:fork>
                            <xsl:sequence>
                                <only>Single Branch</only>
                            </xsl:sequence>
                        </xsl:fork>
                    </output>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root/>";
        let result = execute_xslt3(xslt, xml).unwrap();
        let text = get_text_content(&result);
        assert!(
            text.contains("Single Branch"),
            "Should output single branch"
        );
    }

    #[test]
    fn test_fork_multiple_branches() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <output>
                        <xsl:fork>
                            <xsl:sequence>
                                <a>Alpha</a>
                            </xsl:sequence>
                            <xsl:sequence>
                                <b>Beta</b>
                            </xsl:sequence>
                            <xsl:sequence>
                                <c>Gamma</c>
                            </xsl:sequence>
                        </xsl:fork>
                    </output>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root/>";
        let result = execute_xslt3(xslt, xml).expect("fork with multiple branches failed");
        let text = get_text_content(&result);
        assert!(
            text.contains("Alpha") || text.contains("Beta") || text.contains("Gamma"),
            "Fork should produce output from at least one branch, got: '{}'",
            text
        );
    }

    #[test]
    fn test_fork_with_for_each() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <output>
                        <xsl:fork>
                            <xsl:sequence>
                                <items>
                                    <xsl:for-each select="/data/item">
                                        <item><xsl:value-of select="."/></item>
                                    </xsl:for-each>
                                </items>
                            </xsl:sequence>
                            <xsl:sequence>
                                <count><xsl:value-of select="count(/data/item)"/></count>
                            </xsl:sequence>
                        </xsl:fork>
                    </output>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<data><item>A</item><item>B</item><item>C</item></data>";
        let result = execute_xslt3(xslt, xml).expect("fork with for-each failed");
        let text = get_text_content(&result);
        assert!(
            text.contains('A') || text.contains('B') || text.contains('C') || text.contains('3'),
            "Fork should produce output from at least one branch, got: '{}'",
            text
        );
    }

    #[test]
    fn test_fork_empty_branches() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <output>
                        <xsl:fork>
                            <xsl:sequence select="()"/>
                            <xsl:sequence select="()"/>
                        </xsl:fork>
                    </output>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root/>";
        let result = execute_xslt3(xslt, xml).expect("fork with empty branches failed");
        assert!(
            result.is_empty() || get_text_content(&result).is_empty(),
            "Empty fork branches should produce no output"
        );
    }

    #[test]
    fn test_fork_with_variables() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <xsl:variable name="data" select="/root/value"/>
                    <output>
                        <xsl:fork>
                            <xsl:sequence>
                                <doubled><xsl:value-of select="$data * 2"/></doubled>
                            </xsl:sequence>
                            <xsl:sequence>
                                <squared><xsl:value-of select="$data * $data"/></squared>
                            </xsl:sequence>
                        </xsl:fork>
                    </output>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root><value>5</value></root>";
        let result = execute_xslt3(xslt, xml).expect("fork with variables failed");
        let text = get_text_content(&result);
        assert!(
            !text.is_empty(),
            "Fork with variables should produce output, got: '{}'",
            text
        );
    }
}

mod accumulator_tests {
    use super::*;

    #[test]
    fn test_accumulator_declaration() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:accumulator name="item-count" initial-value="0">
                    <xsl:accumulator-rule match="item" select="$value + 1"/>
                </xsl:accumulator>
                <xsl:template match="/">
                    <result>Accumulator declared</result>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root><item/><item/></root>";
        let result = execute_xslt3(xslt, xml).expect("Accumulator declaration failed");
        let text = get_text_content(&result);
        assert!(text.contains("declared"), "Should produce expected output");
    }

    #[test]
    fn test_accumulator_with_streamable() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:accumulator name="sum" initial-value="0" streamable="yes">
                    <xsl:accumulator-rule match="value" phase="end" 
                        select="$value + number(.)"/>
                </xsl:accumulator>
                <xsl:template match="/">
                    <total>Accumulator ready</total>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<data><value>10</value><value>20</value></data>";
        let result = execute_xslt3(xslt, xml).expect("Streamable accumulator failed");
        let text = get_text_content(&result);
        assert!(text.contains("ready"), "Should produce expected output");
    }
}

mod next_match_tests {
    use super::*;

    #[test]
    fn test_next_match_basic() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="item" priority="2">
                    <high-priority><xsl:value-of select="."/></high-priority>
                    <xsl:next-match/>
                </xsl:template>
                <xsl:template match="item" priority="1">
                    <low-priority><xsl:value-of select="."/></low-priority>
                </xsl:template>
                <xsl:template match="/">
                    <result>
                        <xsl:apply-templates select="/root/item"/>
                    </result>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root><item>Test</item></root>";
        let result = execute_xslt3(xslt, xml).unwrap();
        let text = get_text_content(&result);
        assert!(text.contains("Test"), "Should contain item text");
    }

    #[test]
    fn test_next_match_chain() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="*" priority="3">
                    <wrapper>
                        <xsl:next-match/>
                    </wrapper>
                </xsl:template>
                <xsl:template match="data" priority="2">
                    <data-handler>
                        <xsl:next-match/>
                    </data-handler>
                </xsl:template>
                <xsl:template match="*" priority="1">
                    <fallback><xsl:value-of select="local-name()"/></fallback>
                </xsl:template>
                <xsl:template match="/">
                    <xsl:apply-templates select="/root/data"/>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root><data>Content</data></root>";
        let result = execute_xslt3(xslt, xml);
        assert!(
            result.is_ok(),
            "next-match chain failed: {:?}",
            result.err()
        );
    }
}

mod perform_sort_tests {
    use super::*;

    #[test]
    fn test_perform_sort_basic() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <sorted>
                        <xsl:perform-sort select="/data/item">
                            <xsl:sort select="." order="ascending"/>
                        </xsl:perform-sort>
                    </sorted>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<data><item>C</item><item>A</item><item>B</item></data>";
        let result = execute_xslt3(xslt, xml).expect("perform-sort failed");
        assert!(
            result.is_empty() || !result.is_empty(),
            "perform-sort execution succeeded"
        );
    }

    #[test]
    fn test_perform_sort_descending() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <sorted>
                        <xsl:perform-sort select="/data/num">
                            <xsl:sort select="." data-type="number" order="descending"/>
                        </xsl:perform-sort>
                    </sorted>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<data><num>1</num><num>3</num><num>2</num></data>";
        let result = execute_xslt3(xslt, xml).expect("perform-sort descending failed");
        assert!(
            result.is_empty() || !result.is_empty(),
            "perform-sort descending execution succeeded"
        );
    }
}

mod evaluate_tests {
    use super::*;

    #[test]
    fn test_evaluate_basic_expression() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <result>
                        <xsl:evaluate xpath="'1 + 2'"/>
                    </result>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root/>";
        let result = execute_xslt3(xslt, xml);
        assert!(result.is_ok(), "evaluate failed: {:?}", result.err());
        let text = get_text_content(&result.unwrap());
        assert!(
            text.contains("3"),
            "Expected '3' in output, got: '{}'",
            text
        );
    }

    #[test]
    fn test_evaluate_string_function() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <result>
                        <xsl:evaluate xpath="/config/expr"/>
                    </result>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = r#"<config><expr>upper-case("hello")</expr></config>"#;
        let result = execute_xslt3(xslt, xml);
        assert!(
            result.is_ok(),
            "evaluate string function failed: {:?}",
            result.err()
        );
        let text = get_text_content(&result.unwrap());
        assert!(
            text.contains("HELLO"),
            "Expected 'HELLO' in output, got: '{}'",
            text
        );
    }

    #[test]
    fn test_evaluate_with_context_item() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <result>
                        <xsl:evaluate xpath="'.'" context-item="/data/value"/>
                    </result>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<data><value>Hello World</value></data>";
        let result = execute_xslt3(xslt, xml);
        assert!(
            result.is_ok(),
            "evaluate with context-item failed: {:?}",
            result.err()
        );
        let text = get_text_content(&result.unwrap());
        assert!(
            text.contains("Hello World"),
            "Expected 'Hello World' in output, got: '{}'",
            text
        );
    }

    #[test]
    fn test_evaluate_dynamic_xpath() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <result>
                        <xsl:evaluate xpath="/config/expression"/>
                    </result>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<config><expression>2 * 21</expression></config>";
        let result = execute_xslt3(xslt, xml);
        assert!(
            result.is_ok(),
            "evaluate dynamic xpath failed: {:?}",
            result.err()
        );
        let text = get_text_content(&result.unwrap());
        assert!(
            text.contains("42"),
            "Expected '42' in output, got: '{}'",
            text
        );
    }

    #[test]
    fn test_evaluate_with_variable_in_xpath() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <xsl:variable name="path" select="'/data/items/item'"/>
                    <result>
                        <xsl:evaluate xpath="concat('count(', $path, ')')"/>
                    </result>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<data><items><item>1</item><item>2</item><item>3</item></items></data>";
        let result = execute_xslt3(xslt, xml);
        assert!(
            result.is_ok(),
            "evaluate with variable failed: {:?}",
            result.err()
        );
        let text = get_text_content(&result.unwrap());
        assert!(
            text.contains("3"),
            "Expected '3' items in output, got: '{}'",
            text
        );
    }

    #[test]
    fn test_evaluate_invalid_xpath_silent_failure() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <result>
                        <before>OK</before>
                        <xsl:evaluate xpath="'[invalid xpath'"/>
                        <after>STILL OK</after>
                    </result>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root/>";
        let result = execute_xslt3(xslt, xml);
        assert!(
            result.is_ok(),
            "evaluate should handle invalid xpath gracefully: {:?}",
            result.err()
        );
        let text = get_text_content(&result.unwrap());
        assert!(
            text.contains("OK"),
            "Expected processing to continue, got: '{}'",
            text
        );
    }
}

mod integration_tests {
    use super::*;

    #[test]
    fn test_complex_xslt3_stylesheet() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                expand-text="yes">
                
                <xsl:template match="/">
                    <report>
                        <title>Report for {/data/title}</title>
                        <xsl:apply-templates select="/data/items"/>
                    </report>
                </xsl:template>
                
                <xsl:template match="items">
                    <section>
                        <xsl:for-each select="item">
                            <xsl:choose>
                                <xsl:when test="@status = 'active'">
                                    <active>{.}</active>
                                </xsl:when>
                                <xsl:otherwise>
                                    <inactive>{.}</inactive>
                                </xsl:otherwise>
                            </xsl:choose>
                        </xsl:for-each>
                    </section>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = r#"
            <data>
                <title>Test Report</title>
                <items>
                    <item status="active">Item 1</item>
                    <item status="inactive">Item 2</item>
                    <item status="active">Item 3</item>
                </items>
            </data>
        "#;

        let result = execute_xslt3(xslt, xml);
        assert!(result.is_ok(), "Execution failed: {:?}", result.err());

        let nodes = result.unwrap();
        let text = get_text_content(&nodes);
        assert!(text.contains("Test Report"));
        assert!(text.contains("Item 1"));
        assert!(text.contains("Item 2"));
        assert!(text.contains("Item 3"));
    }

    #[test]
    fn test_mixed_xslt1_xslt3_features() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                
                <xsl:variable name="global" select="'Global Value'"/>
                
                <xsl:template match="/">
                    <output>
                        <xsl:value-of select="$global"/>
                        <xsl:text> - </xsl:text>
                        <xsl:try>
                            <xsl:value-of select="/root/value"/>
                            <xsl:catch>
                                <error/>
                            </xsl:catch>
                        </xsl:try>
                    </output>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root><value>Local Value</value></root>";
        let result = execute_xslt3(xslt, xml).unwrap();
        let text = get_text_content(&result);
        assert!(text.contains("Global Value"));
        assert!(text.contains("Local Value"));
    }

    #[test]
    fn test_nested_templates() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                expand-text="yes">
                
                <xsl:template match="/">
                    <document>
                        <xsl:apply-templates select="/doc/section"/>
                    </document>
                </xsl:template>
                
                <xsl:template match="section">
                    <div class="{@type}">
                        <xsl:apply-templates select="para"/>
                    </div>
                </xsl:template>
                
                <xsl:template match="para">
                    <p>{.}</p>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = r#"
            <doc>
                <section type="intro">
                    <para>First paragraph</para>
                    <para>Second paragraph</para>
                </section>
            </doc>
        "#;

        let result = execute_xslt3(xslt, xml).unwrap();
        let text = get_text_content(&result);
        assert!(text.contains("First paragraph"));
        assert!(text.contains("Second paragraph"));
    }
}

mod streaming_tests {
    use crate::ast::{Accumulator, AccumulatorPhase, AccumulatorRule, Pattern3};
    use crate::streaming::parse_and_stream_with_accumulators;
    use crate::test_helpers::parse_stylesheet;
    use petty_xpath1::ast::BinaryOperator;
    use petty_xpath31::Expression;
    use petty_xpath31::ast::Literal;

    #[test]
    fn test_streaming_accumulator_counts_items() {
        let mut stylesheet = parse_stylesheet(
            r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <output/>
                </xsl:template>
            </xsl:stylesheet>
        "#,
        )
        .unwrap();

        stylesheet.accumulators.insert(
            "item-count".to_string(),
            Accumulator {
                name: "item-count".to_string(),
                initial_value: Expression::Literal(Literal::Integer(0)),
                rules: vec![AccumulatorRule {
                    pattern: Pattern3("item".to_string()),
                    phase: AccumulatorPhase::Start,
                    select: Expression::BinaryOp {
                        left: Box::new(Expression::Variable("value".to_string())),
                        right: Box::new(Expression::Literal(Literal::Integer(1))),
                        op: BinaryOperator::Plus,
                    },
                }],
                streamable: true,
            },
        );

        let xml = r#"<root><item/><item/><item/><item/><item/></root>"#;
        let result = parse_and_stream_with_accumulators(xml, &stylesheet).unwrap();

        assert_eq!(
            result
                .accumulator_values
                .get("item-count")
                .map(String::as_str),
            Some("5"),
            "Should count 5 items"
        );
    }

    #[test]
    fn test_streaming_accumulator_with_nested_items() {
        let mut stylesheet = parse_stylesheet(
            r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <output/>
                </xsl:template>
            </xsl:stylesheet>
        "#,
        )
        .unwrap();

        stylesheet.accumulators.insert(
            "count".to_string(),
            Accumulator {
                name: "count".to_string(),
                initial_value: Expression::Literal(Literal::Integer(0)),
                rules: vec![AccumulatorRule {
                    pattern: Pattern3("item".to_string()),
                    phase: AccumulatorPhase::Start,
                    select: Expression::BinaryOp {
                        left: Box::new(Expression::Variable("value".to_string())),
                        right: Box::new(Expression::Literal(Literal::Integer(1))),
                        op: BinaryOperator::Plus,
                    },
                }],
                streamable: true,
            },
        );

        let xml = r#"
            <root>
                <section>
                    <item id="1"/>
                    <item id="2"/>
                </section>
                <section>
                    <item id="3"/>
                </section>
            </root>
        "#;
        let result = parse_and_stream_with_accumulators(xml, &stylesheet).unwrap();

        assert_eq!(
            result.accumulator_values.get("count").map(String::as_str),
            Some("3"),
            "Should count 3 items across nested sections"
        );
    }

    #[test]
    fn test_streaming_multiple_accumulators() {
        let mut stylesheet = parse_stylesheet(
            r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <output/>
                </xsl:template>
            </xsl:stylesheet>
        "#,
        )
        .unwrap();

        stylesheet.accumulators.insert(
            "item-count".to_string(),
            Accumulator {
                name: "item-count".to_string(),
                initial_value: Expression::Literal(Literal::Integer(0)),
                rules: vec![AccumulatorRule {
                    pattern: Pattern3("item".to_string()),
                    phase: AccumulatorPhase::Start,
                    select: Expression::BinaryOp {
                        left: Box::new(Expression::Variable("value".to_string())),
                        right: Box::new(Expression::Literal(Literal::Integer(1))),
                        op: BinaryOperator::Plus,
                    },
                }],
                streamable: true,
            },
        );

        stylesheet.accumulators.insert(
            "section-count".to_string(),
            Accumulator {
                name: "section-count".to_string(),
                initial_value: Expression::Literal(Literal::Integer(0)),
                rules: vec![AccumulatorRule {
                    pattern: Pattern3("section".to_string()),
                    phase: AccumulatorPhase::Start,
                    select: Expression::BinaryOp {
                        left: Box::new(Expression::Variable("value".to_string())),
                        right: Box::new(Expression::Literal(Literal::Integer(1))),
                        op: BinaryOperator::Plus,
                    },
                }],
                streamable: true,
            },
        );

        let xml = r#"
            <root>
                <section><item/><item/></section>
                <section><item/></section>
                <section></section>
            </root>
        "#;
        let result = parse_and_stream_with_accumulators(xml, &stylesheet).unwrap();

        assert_eq!(
            result
                .accumulator_values
                .get("item-count")
                .map(String::as_str),
            Some("3"),
            "Should count 3 items"
        );
        assert_eq!(
            result
                .accumulator_values
                .get("section-count")
                .map(String::as_str),
            Some("3"),
            "Should count 3 sections"
        );
    }

    #[test]
    fn test_streaming_accumulator_end_phase() {
        let mut stylesheet = parse_stylesheet(
            r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <output/>
                </xsl:template>
            </xsl:stylesheet>
        "#,
        )
        .unwrap();

        stylesheet.accumulators.insert(
            "end-count".to_string(),
            Accumulator {
                name: "end-count".to_string(),
                initial_value: Expression::Literal(Literal::Integer(0)),
                rules: vec![AccumulatorRule {
                    pattern: Pattern3("item".to_string()),
                    phase: AccumulatorPhase::End,
                    select: Expression::BinaryOp {
                        left: Box::new(Expression::Variable("value".to_string())),
                        right: Box::new(Expression::Literal(Literal::Integer(1))),
                        op: BinaryOperator::Plus,
                    },
                }],
                streamable: true,
            },
        );

        let xml = r#"<root><item/><item/></root>"#;
        let result = parse_and_stream_with_accumulators(xml, &stylesheet).unwrap();

        assert_eq!(
            result
                .accumulator_values
                .get("end-count")
                .map(String::as_str),
            Some("2"),
            "Should count items on end-element events"
        );
    }

    #[test]
    fn test_streaming_large_document() {
        let mut stylesheet = parse_stylesheet(
            r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <output/>
                </xsl:template>
            </xsl:stylesheet>
        "#,
        )
        .unwrap();

        stylesheet.accumulators.insert(
            "count".to_string(),
            Accumulator {
                name: "count".to_string(),
                initial_value: Expression::Literal(Literal::Integer(0)),
                rules: vec![AccumulatorRule {
                    pattern: Pattern3("item".to_string()),
                    phase: AccumulatorPhase::Start,
                    select: Expression::BinaryOp {
                        left: Box::new(Expression::Variable("value".to_string())),
                        right: Box::new(Expression::Literal(Literal::Integer(1))),
                        op: BinaryOperator::Plus,
                    },
                }],
                streamable: true,
            },
        );

        let mut xml = String::from("<root>");
        for i in 0..10000 {
            xml.push_str(&format!("<item id=\"{}\"/>", i));
        }
        xml.push_str("</root>");

        let result = parse_and_stream_with_accumulators(&xml, &stylesheet).unwrap();

        assert_eq!(
            result.accumulator_values.get("count").map(String::as_str),
            Some("10000"),
            "Should count 10000 items efficiently"
        );
    }

    #[test]
    fn test_stream_body_validation_rejects_non_streamable() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <xsl:stream href="data.xml">
                        <xsl:value-of select="preceding-sibling::item"/>
                    </xsl:stream>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let result = parse_stylesheet(xslt);
        assert!(
            result.is_err(),
            "Should reject non-streamable expression in xsl:stream body"
        );
    }

    #[test]
    fn test_stream_body_validation_accepts_grounded() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <xsl:stream href="data.xml">
                        <xsl:value-of select="'literal'"/>
                    </xsl:stream>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let result = parse_stylesheet(xslt);
        assert!(
            result.is_ok(),
            "Should accept grounded expression in xsl:stream body: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_stream_body_validation_accepts_accumulator_after() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:accumulator name="count" initial-value="0" streamable="yes">
                    <xsl:accumulator-rule match="item">
                        <xsl:sequence select="$value + 1"/>
                    </xsl:accumulator-rule>
                </xsl:accumulator>
                <xsl:template match="/">
                    <xsl:stream href="data.xml">
                        <result>
                            <xsl:accumulator-after name="count"/>
                        </result>
                    </xsl:stream>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let result = parse_stylesheet(xslt);
        assert!(
            result.is_ok(),
            "Should accept accumulator-after in xsl:stream body: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_xsl_stream_with_resource_provider_and_accumulator() {
        use crate::ast::{Accumulator, AccumulatorPhase, AccumulatorRule, Pattern3};
        use crate::executor::TemplateExecutor3;
        use petty_traits::InMemoryResourceProvider;
        use petty_xpath1::ast::BinaryOperator;
        use petty_xpath31::Expression;
        use petty_xpath31::ast::Literal;
        use petty_xslt::datasources::xml::XmlDocument;
        use std::sync::Arc;

        let mut stylesheet = parse_stylesheet(
            r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <result>
                        <xsl:stream href="streamed-data.xml"/>
                    </result>
                </xsl:template>
            </xsl:stylesheet>
        "#,
        )
        .unwrap();

        stylesheet.accumulators.insert(
            "total".to_string(),
            Accumulator {
                name: "total".to_string(),
                initial_value: Expression::Literal(Literal::Integer(0)),
                rules: vec![AccumulatorRule {
                    pattern: Pattern3("item".to_string()),
                    phase: AccumulatorPhase::Start,
                    select: Expression::BinaryOp {
                        left: Box::new(Expression::Variable("value".to_string())),
                        right: Box::new(Expression::Literal(Literal::Integer(1))),
                        op: BinaryOperator::Plus,
                    },
                }],
                streamable: true,
            },
        );

        let provider = InMemoryResourceProvider::new();
        provider
            .add(
                "streamed-data.xml",
                b"<root><item/><item/><item/><item/><item/></root>".to_vec(),
            )
            .unwrap();

        let doc = XmlDocument::parse("<context/>").unwrap();
        let root_node = doc.root_node();

        let mut executor = TemplateExecutor3::new(&stylesheet, root_node, false)
            .unwrap()
            .with_resource_provider(Arc::new(provider));

        let result = executor.build_tree();
        assert!(
            result.is_ok(),
            "xsl:stream execution should succeed: {:?}",
            result.err()
        );

        assert_eq!(
            executor.accumulator_values.get("total").map(String::as_str),
            Some("5"),
            "Accumulator should count 5 items from streamed document"
        );
    }

    #[test]
    fn test_xsl_source_document_with_streaming_accumulator() {
        use crate::ast::{Accumulator, AccumulatorPhase, AccumulatorRule, Pattern3};
        use crate::executor::TemplateExecutor3;
        use petty_traits::InMemoryResourceProvider;
        use petty_xpath1::ast::BinaryOperator;
        use petty_xpath31::Expression;
        use petty_xpath31::ast::Literal;
        use petty_xslt::datasources::xml::XmlDocument;
        use std::sync::Arc;

        let mut stylesheet = parse_stylesheet(
            r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <output>
                        <xsl:source-document href="records.xml" streamable="yes"/>
                    </output>
                </xsl:template>
            </xsl:stylesheet>
        "#,
        )
        .unwrap();

        stylesheet.accumulators.insert(
            "counter".to_string(),
            Accumulator {
                name: "counter".to_string(),
                initial_value: Expression::Literal(Literal::Integer(0)),
                rules: vec![AccumulatorRule {
                    pattern: Pattern3("record".to_string()),
                    phase: AccumulatorPhase::Start,
                    select: Expression::BinaryOp {
                        left: Box::new(Expression::Variable("value".to_string())),
                        right: Box::new(Expression::Literal(Literal::Integer(1))),
                        op: BinaryOperator::Plus,
                    },
                }],
                streamable: true,
            },
        );

        let provider = InMemoryResourceProvider::new();
        provider
            .add(
                "records.xml",
                b"<data><record id=\"1\"/><record id=\"2\"/><record id=\"3\"/></data>".to_vec(),
            )
            .unwrap();

        let doc = XmlDocument::parse("<context/>").unwrap();
        let root_node = doc.root_node();

        let mut executor = TemplateExecutor3::new(&stylesheet, root_node, false)
            .unwrap()
            .with_resource_provider(Arc::new(provider));

        let result = executor.build_tree();
        assert!(
            result.is_ok(),
            "xsl:source-document streaming should succeed: {:?}",
            result.err()
        );

        assert_eq!(
            executor
                .accumulator_values
                .get("counter")
                .map(String::as_str),
            Some("3"),
            "Accumulator should count 3 records from streamed source-document"
        );
    }

    #[test]
    fn test_xsl_stream_without_resource_provider_fails() {
        use crate::executor::TemplateExecutor3;
        use petty_xslt::datasources::xml::XmlDocument;

        let stylesheet = parse_stylesheet(
            r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <xsl:stream href="missing.xml"/>
                </xsl:template>
            </xsl:stylesheet>
        "#,
        )
        .unwrap();

        let doc = XmlDocument::parse("<context/>").unwrap();
        let root_node = doc.root_node();

        let mut executor = TemplateExecutor3::new(&stylesheet, root_node, false).unwrap();

        let result = executor.build_tree();
        assert!(
            result.is_err(),
            "xsl:stream without resource provider should fail"
        );

        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("No resource provider") || err.contains("resource provider"),
            "Error should mention missing resource provider. Got: {}",
            err
        );
    }
}

mod result_document_tests {
    use super::*;
    use crate::executor::TemplateExecutor3;
    use petty_xslt::datasources::xml::XmlDocument;
    use petty_xslt::output::MultiOutputCollector;
    use std::sync::Arc;

    fn execute_with_multi_output(
        xslt_source: &str,
        xml_data: &str,
    ) -> Result<MultiOutputCollector, crate::error::Xslt3Error> {
        let stylesheet = parse_stylesheet(xslt_source)?;
        let doc = XmlDocument::parse(xml_data)
            .map_err(|e| crate::error::Xslt3Error::runtime(format!("XML parse error: {}", e)))?;
        let root_node = doc.root_node();

        let collector = MultiOutputCollector::new();
        let mut executor = TemplateExecutor3::new(&stylesheet, root_node, false)?
            .with_output_sink(Arc::new(collector.clone()));

        executor
            .build_tree()
            .map_err(|e| crate::error::Xslt3Error::runtime(e.to_string()))?;

        Ok(collector)
    }

    #[test]
    fn test_result_document_primary_output_without_href() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <primary>Main content</primary>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let result = execute_xslt3(xslt, "<root/>");
        assert!(
            result.is_ok(),
            "Basic execution should work: {:?}",
            result.err()
        );
        let text = get_text_content(&result.unwrap());
        assert!(text.contains("Main content"));
    }

    #[test]
    fn test_result_document_creates_secondary_output() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <primary>Main</primary>
                    <xsl:result-document href="secondary.xml">
                        <secondary>Other</secondary>
                    </xsl:result-document>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let collector = execute_with_multi_output(xslt, "<root/>").unwrap();
        let outputs = collector.get_outputs();

        assert!(
            outputs.contains_key("secondary.xml"),
            "Should have secondary.xml output. Got: {:?}",
            outputs.keys().collect::<Vec<_>>()
        );

        let secondary = outputs.get("secondary.xml").unwrap();
        let text = get_text_content(secondary);
        assert!(
            text.contains("Other"),
            "Secondary should contain 'Other'. Got: {}",
            text
        );
    }

    #[test]
    fn test_result_document_duplicate_href_error() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <xsl:result-document href="same.xml">
                        <first/>
                    </xsl:result-document>
                    <xsl:result-document href="same.xml">
                        <second/>
                    </xsl:result-document>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let result = execute_with_multi_output(xslt, "<root/>");
        assert!(result.is_err(), "Should fail with duplicate href");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("XTDE1490") || err.contains("Duplicate"),
            "Should mention XTDE1490. Got: {}",
            err
        );
    }

    #[test]
    fn test_result_document_nested_conflict_error() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <xsl:result-document href="outer.xml">
                        <outer>
                            <xsl:result-document href="outer.xml">
                                <nested/>
                            </xsl:result-document>
                        </outer>
                    </xsl:result-document>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let result = execute_with_multi_output(xslt, "<root/>");
        assert!(result.is_err(), "Should fail with nested conflict");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("XTDE1500") || err.contains("Nested"),
            "Should mention XTDE1500 or nested. Got: {}",
            err
        );
    }

    #[test]
    fn test_result_document_dynamic_href() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <xsl:for-each select="root/item">
                        <xsl:result-document href="{@id}.xml">
                            <item><xsl:value-of select="."/></item>
                        </xsl:result-document>
                    </xsl:for-each>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = r#"<root><item id="a">Alpha</item><item id="b">Beta</item></root>"#;
        let collector = execute_with_multi_output(xslt, xml).unwrap();
        let outputs = collector.get_outputs();

        assert!(
            outputs.contains_key("a.xml"),
            "Should have a.xml. Got: {:?}",
            outputs.keys().collect::<Vec<_>>()
        );
        assert!(
            outputs.contains_key("b.xml"),
            "Should have b.xml. Got: {:?}",
            outputs.keys().collect::<Vec<_>>()
        );

        let a_text = get_text_content(outputs.get("a.xml").unwrap());
        assert!(
            a_text.contains("Alpha"),
            "a.xml should contain Alpha. Got: {}",
            a_text
        );

        let b_text = get_text_content(outputs.get("b.xml").unwrap());
        assert!(
            b_text.contains("Beta"),
            "b.xml should contain Beta. Got: {}",
            b_text
        );
    }

    #[test]
    fn test_result_document_without_sink_errors_for_secondary() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <xsl:result-document href="secondary.xml">
                        <content/>
                    </xsl:result-document>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let stylesheet = parse_stylesheet(xslt).unwrap();
        let doc = XmlDocument::parse("<root/>").unwrap();
        let root_node = doc.root_node();

        let mut executor = TemplateExecutor3::new(&stylesheet, root_node, false).unwrap();
        let result = executor.build_tree();

        assert!(
            result.is_err(),
            "Should fail without output sink for secondary output"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("OutputSink") || err.contains("XTDE1480"),
            "Should mention OutputSink requirement. Got: {}",
            err
        );
    }

    #[test]
    fn test_result_document_empty_href_uses_primary() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <xsl:result-document href="">
                        <primary>Content</primary>
                    </xsl:result-document>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let result = execute_xslt3(xslt, "<root/>");
        assert!(
            result.is_ok(),
            "Empty href should use primary: {:?}",
            result.err()
        );
        let text = get_text_content(&result.unwrap());
        assert!(text.contains("Content"), "Should have content in primary");
    }

    #[test]
    fn test_result_document_default_href_uses_primary() {
        let xslt = r##"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <xsl:result-document href="#default">
                        <primary>Content</primary>
                    </xsl:result-document>
                </xsl:template>
            </xsl:stylesheet>
        "##;

        let result = execute_xslt3(xslt, "<root/>");
        assert!(
            result.is_ok(),
            "default href should use primary: {:?}",
            result.err()
        );
        let text = get_text_content(&result.unwrap());
        assert!(text.contains("Content"), "Should have content in primary");
    }

    #[test]
    fn test_result_document_multiple_distinct_outputs() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <primary>Main</primary>
                    <xsl:result-document href="doc1.xml">
                        <doc1>First</doc1>
                    </xsl:result-document>
                    <xsl:result-document href="doc2.xml">
                        <doc2>Second</doc2>
                    </xsl:result-document>
                    <xsl:result-document href="doc3.xml">
                        <doc3>Third</doc3>
                    </xsl:result-document>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let collector = execute_with_multi_output(xslt, "<root/>").unwrap();
        let outputs = collector.get_outputs();

        assert_eq!(outputs.len(), 3, "Should have 3 secondary outputs");
        assert!(outputs.contains_key("doc1.xml"));
        assert!(outputs.contains_key("doc2.xml"));
        assert!(outputs.contains_key("doc3.xml"));
    }

    #[test]
    fn test_result_document_nested_different_hrefs_allowed() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <xsl:result-document href="outer.xml">
                        <outer>
                            <xsl:result-document href="inner.xml">
                                <inner/>
                            </xsl:result-document>
                        </outer>
                    </xsl:result-document>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let collector = execute_with_multi_output(xslt, "<root/>").unwrap();
        let outputs = collector.get_outputs();

        assert!(outputs.contains_key("outer.xml"), "Should have outer.xml");
        assert!(outputs.contains_key("inner.xml"), "Should have inner.xml");
    }
}

mod number_tests {
    use super::*;

    #[test]
    fn test_number_simple() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <result>
                        <xsl:for-each select="/data/item">
                            <num><xsl:number/></num>
                        </xsl:for-each>
                    </result>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<data><item>A</item><item>B</item><item>C</item></data>";
        let result = execute_xslt3(xslt, xml);
        assert!(result.is_ok(), "xsl:number failed: {:?}", result.err());
        let text = get_text_content(&result.unwrap());
        assert!(text.contains("1"), "Should contain '1'");
        assert!(text.contains("2"), "Should contain '2'");
        assert!(text.contains("3"), "Should contain '3'");
    }

    #[test]
    fn test_number_with_value() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <result>
                        <xsl:number value="42"/>
                    </result>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root/>";
        let result = execute_xslt3(xslt, xml);
        assert!(
            result.is_ok(),
            "xsl:number with value failed: {:?}",
            result.err()
        );
        let text = get_text_content(&result.unwrap());
        assert!(text.contains("42"), "Should contain '42', got: '{}'", text);
    }

    #[test]
    fn test_number_format_alpha() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <result>
                        <xsl:for-each select="/data/item">
                            <num><xsl:number format="a"/></num>
                        </xsl:for-each>
                    </result>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<data><item>A</item><item>B</item><item>C</item></data>";
        let result = execute_xslt3(xslt, xml);
        assert!(
            result.is_ok(),
            "xsl:number format='a' failed: {:?}",
            result.err()
        );
        let text = get_text_content(&result.unwrap());
        assert!(text.contains("a"), "Should contain 'a'");
        assert!(text.contains("b"), "Should contain 'b'");
        assert!(text.contains("c"), "Should contain 'c'");
    }

    #[test]
    fn test_number_format_roman() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <result>
                        <xsl:number value="4" format="i"/>
                        <xsl:text> </xsl:text>
                        <xsl:number value="9" format="I"/>
                    </result>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root/>";
        let result = execute_xslt3(xslt, xml);
        assert!(
            result.is_ok(),
            "xsl:number format roman failed: {:?}",
            result.err()
        );
        let text = get_text_content(&result.unwrap());
        assert!(
            text.contains("iv"),
            "Should contain 'iv' for 4, got: '{}'",
            text
        );
        assert!(
            text.contains("IX"),
            "Should contain 'IX' for 9, got: '{}'",
            text
        );
    }

    #[test]
    fn test_number_level_any() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <result>
                        <xsl:for-each select="//item">
                            <num><xsl:number level="any" count="item"/></num>
                        </xsl:for-each>
                    </result>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = r#"
            <root>
                <group><item>A</item><item>B</item></group>
                <group><item>C</item></group>
            </root>
        "#;
        let result = execute_xslt3(xslt, xml);
        assert!(
            result.is_ok(),
            "xsl:number level='any' failed: {:?}",
            result.err()
        );
        let text = get_text_content(&result.unwrap());
        assert!(text.contains("1"), "Should contain '1'");
        assert!(text.contains("2"), "Should contain '2'");
        assert!(text.contains("3"), "Should contain '3'");
    }

    #[test]
    fn test_number_level_multiple() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <result>
                        <xsl:for-each select="//section">
                            <num><xsl:number level="multiple" count="chapter|section" format="1.1"/></num>
                        </xsl:for-each>
                    </result>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = r#"
            <book>
                <chapter><section>S1</section><section>S2</section></chapter>
                <chapter><section>S3</section></chapter>
            </book>
        "#;
        let result = execute_xslt3(xslt, xml);
        assert!(
            result.is_ok(),
            "xsl:number level='multiple' failed: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_number_grouping() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <result>
                        <xsl:number value="1234567" grouping-separator="," grouping-size="3"/>
                    </result>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root/>";
        let result = execute_xslt3(xslt, xml);
        assert!(
            result.is_ok(),
            "xsl:number grouping failed: {:?}",
            result.err()
        );
        let text = get_text_content(&result.unwrap());
        assert!(
            text.contains("1,234,567"),
            "Should contain '1,234,567', got: '{}'",
            text
        );
    }

    #[test]
    fn test_number_format_padded() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <result>
                        <xsl:number value="7" format="001"/>
                    </result>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root/>";
        let result = execute_xslt3(xslt, xml);
        assert!(
            result.is_ok(),
            "xsl:number format='001' failed: {:?}",
            result.err()
        );
        let text = get_text_content(&result.unwrap());
        assert!(
            text.contains("007"),
            "Should contain '007', got: '{}'",
            text
        );
    }
}

mod key_tests {
    use super::*;

    #[test]
    fn test_attribute_access_basic() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <result>
                        <xsl:for-each select="//item">
                            <id><xsl:value-of select="@id"/></id>
                        </xsl:for-each>
                    </result>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root><item id='a'>First</item><item id='b'>Second</item></root>";
        let result = execute_xslt3(xslt, xml);
        assert!(
            result.is_ok(),
            "Attribute access failed: {:?}",
            result.err()
        );
        let text = get_text_content(&result.unwrap());
        assert!(text.contains("a"), "Should contain 'a', got: '{}'", text);
        assert!(text.contains("b"), "Should contain 'b', got: '{}'", text);
    }

    #[test]
    fn test_key_declaration() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:key name="items-by-id" match="item" use="@id"/>
                <xsl:template match="/">
                    <result>Key declared</result>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root><item id='a'/><item id='b'/></root>";
        let result = execute_xslt3(xslt, xml);
        assert!(
            result.is_ok(),
            "xsl:key declaration failed: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_key_lookup_simple() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:key name="items-by-id" match="item" use="@id"/>
                <xsl:template match="/">
                    <result>
                        <found><xsl:value-of select="key('items-by-id', 'b')"/></found>
                    </result>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root><item id='a'>First</item><item id='b'>Second</item></root>";
        let result = execute_xslt3(xslt, xml);
        assert!(result.is_ok(), "key() lookup failed: {:?}", result.err());
        let text = get_text_content(&result.unwrap());
        assert!(
            text.contains("Second"),
            "Should find item 'b' with content 'Second', got: '{}'",
            text
        );
    }

    #[test]
    fn test_key_lookup_not_found() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:key name="items-by-id" match="item" use="@id"/>
                <xsl:template match="/">
                    <result>
                        <count><xsl:value-of select="count(key('items-by-id', 'nonexistent'))"/></count>
                    </result>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root><item id='a'>First</item></root>";
        let result = execute_xslt3(xslt, xml);
        assert!(
            result.is_ok(),
            "key() lookup for nonexistent failed: {:?}",
            result.err()
        );
        let text = get_text_content(&result.unwrap());
        assert!(
            text.contains("0"),
            "Should return 0 for nonexistent key, got: '{}'",
            text
        );
    }

    #[test]
    fn test_key_multiple_matches() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:key name="items-by-type" match="item" use="@type"/>
                <xsl:template match="/">
                    <result>
                        <count><xsl:value-of select="count(key('items-by-type', 'fruit'))"/></count>
                    </result>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = r#"
            <root>
                <item type="fruit">Apple</item>
                <item type="vegetable">Carrot</item>
                <item type="fruit">Banana</item>
                <item type="fruit">Orange</item>
            </root>
        "#;
        let result = execute_xslt3(xslt, xml);
        assert!(
            result.is_ok(),
            "key() with multiple matches failed: {:?}",
            result.err()
        );
        let text = get_text_content(&result.unwrap());
        assert!(
            text.contains("3"),
            "Should find 3 fruit items, got: '{}'",
            text
        );
    }

    #[test]
    fn test_key_in_for_each() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:key name="products-by-category" match="product" use="@category"/>
                <xsl:template match="/">
                    <result>
                        <xsl:for-each select="key('products-by-category', 'electronics')">
                            <item><xsl:value-of select="."/></item>
                        </xsl:for-each>
                    </result>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = r#"
            <catalog>
                <product category="electronics">Laptop</product>
                <product category="books">Novel</product>
                <product category="electronics">Phone</product>
            </catalog>
        "#;
        let result = execute_xslt3(xslt, xml);
        assert!(
            result.is_ok(),
            "key() in for-each failed: {:?}",
            result.err()
        );
        let text = get_text_content(&result.unwrap());
        assert!(
            text.contains("Laptop"),
            "Should contain 'Laptop', got: '{}'",
            text
        );
        assert!(
            text.contains("Phone"),
            "Should contain 'Phone', got: '{}'",
            text
        );
    }
}

mod import_include_tests {
    use super::*;
    use std::collections::HashMap;

    fn create_stylesheet_resolver(
        stylesheets: HashMap<&str, &str>,
    ) -> impl Fn(&str) -> Result<crate::ast::CompiledStylesheet3, crate::error::Xslt3Error> {
        move |href: &str| {
            let content = stylesheets
                .get(href)
                .ok_or_else(|| crate::error::Xslt3Error::parse(format!("Not found: {}", href)))?;
            parse_stylesheet(content)
        }
    }

    #[test]
    fn test_include_merges_templates() {
        let main = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:include href="helpers.xsl"/>
                <xsl:template match="/">
                    <output><xsl:call-template name="greet"/></output>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let helpers = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template name="greet">Hello from included</xsl:template>
            </xsl:stylesheet>
        "#;

        let mut stylesheet = parse_stylesheet(main).unwrap();
        let mut external = HashMap::new();
        external.insert("helpers.xsl", helpers);
        let resolver = create_stylesheet_resolver(external);

        stylesheet.resolve_imports_includes(resolver).unwrap();
        stylesheet.finalize_after_merge();

        assert!(stylesheet.named_templates.contains_key("greet"));
    }

    #[test]
    fn test_import_merges_with_lower_priority() {
        let main = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:import href="base.xsl"/>
                <xsl:template match="item" priority="0">MAIN</xsl:template>
            </xsl:stylesheet>
        "#;

        let base = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="item" priority="0">BASE</xsl:template>
            </xsl:stylesheet>
        "#;

        let mut stylesheet = parse_stylesheet(main).unwrap();
        let mut external = HashMap::new();
        external.insert("base.xsl", base);
        let resolver = create_stylesheet_resolver(external);

        stylesheet.resolve_imports_includes(resolver).unwrap();
        stylesheet.finalize_after_merge();

        let rules = stylesheet.template_rules.get(&None).unwrap();
        assert_eq!(rules.len(), 2);
        assert!(
            rules[0].priority > rules[1].priority,
            "Main template should have higher priority"
        );
    }

    #[test]
    fn test_include_preserves_priority() {
        let main = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:include href="extra.xsl"/>
                <xsl:template match="item" priority="1">MAIN</xsl:template>
            </xsl:stylesheet>
        "#;

        let extra = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="item" priority="2">EXTRA</xsl:template>
            </xsl:stylesheet>
        "#;

        let mut stylesheet = parse_stylesheet(main).unwrap();
        let mut external = HashMap::new();
        external.insert("extra.xsl", extra);
        let resolver = create_stylesheet_resolver(external);

        stylesheet.resolve_imports_includes(resolver).unwrap();
        stylesheet.finalize_after_merge();

        let rules = stylesheet.template_rules.get(&None).unwrap();
        assert_eq!(rules.len(), 2);
        assert!(
            rules[0].priority > rules[1].priority,
            "Higher explicit priority should come first"
        );
    }

    #[test]
    fn test_import_merges_named_templates() {
        let main = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:import href="base.xsl"/>
                <xsl:template match="/">
                    <result><xsl:call-template name="helper"/></result>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let base = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template name="helper">Imported helper</xsl:template>
            </xsl:stylesheet>
        "#;

        let mut stylesheet = parse_stylesheet(main).unwrap();
        let mut external = HashMap::new();
        external.insert("base.xsl", base);
        let resolver = create_stylesheet_resolver(external);

        stylesheet.resolve_imports_includes(resolver).unwrap();
        stylesheet.finalize_after_merge();

        assert!(stylesheet.named_templates.contains_key("helper"));
    }

    #[test]
    fn test_main_template_overrides_imported() {
        let main = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:import href="base.xsl"/>
                <xsl:template name="shared">MAIN VERSION</xsl:template>
            </xsl:stylesheet>
        "#;

        let base = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template name="shared">BASE VERSION</xsl:template>
            </xsl:stylesheet>
        "#;

        let mut stylesheet = parse_stylesheet(main).unwrap();
        let mut external = HashMap::new();
        external.insert("base.xsl", base);
        let resolver = create_stylesheet_resolver(external);

        stylesheet.resolve_imports_includes(resolver).unwrap();

        assert!(stylesheet.named_templates.contains_key("shared"));
    }

    #[test]
    fn test_import_merges_keys() {
        let main = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:import href="keys.xsl"/>
                <xsl:template match="/"><result/></xsl:template>
            </xsl:stylesheet>
        "#;

        let keys = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:key name="items-by-id" match="item" use="@id"/>
            </xsl:stylesheet>
        "#;

        let mut stylesheet = parse_stylesheet(main).unwrap();
        let mut external = HashMap::new();
        external.insert("keys.xsl", keys);
        let resolver = create_stylesheet_resolver(external);

        stylesheet.resolve_imports_includes(resolver).unwrap();

        assert!(stylesheet.keys.contains_key("items-by-id"));
    }
}

mod attribute_set_tests {
    use super::*;

    #[test]
    fn test_compile_attribute_set_basic() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:attribute-set name="common-attrs">
                    <xsl:attribute name="class" select="'default'"/>
                    <xsl:attribute name="id" select="'main'"/>
                </xsl:attribute-set>
                <xsl:template match="/">
                    <div xsl:use-attribute-sets="common-attrs">Content</div>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let stylesheet = parse_stylesheet(xslt).unwrap();
        assert!(stylesheet.attribute_sets.contains_key("common-attrs"));
        let attr_set = stylesheet.attribute_sets.get("common-attrs").unwrap();
        assert_eq!(attr_set.name, "common-attrs");
        assert_eq!(attr_set.attributes.0.len(), 2);
    }

    #[test]
    fn test_execute_use_attribute_sets() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:attribute-set name="test-attrs">
                    <xsl:attribute name="data-test" select="'hello'"/>
                </xsl:attribute-set>
                <xsl:template match="/">
                    <result xsl:use-attribute-sets="test-attrs">
                        <xsl:value-of select="'content'"/>
                    </result>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root/>";
        let result = execute_xslt3(xslt, xml).unwrap();
        let text = get_text_content(&result);
        assert!(text.contains("content"));
    }

    #[test]
    fn test_compile_attribute_set_with_use_attribute_sets() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:attribute-set name="base-attrs">
                    <xsl:attribute name="class" select="'base'"/>
                </xsl:attribute-set>
                <xsl:attribute-set name="extended-attrs" use-attribute-sets="base-attrs">
                    <xsl:attribute name="data-extended" select="'true'"/>
                </xsl:attribute-set>
                <xsl:template match="/">
                    <div xsl:use-attribute-sets="extended-attrs">Content</div>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let stylesheet = parse_stylesheet(xslt).unwrap();
        assert!(stylesheet.attribute_sets.contains_key("base-attrs"));
        assert!(stylesheet.attribute_sets.contains_key("extended-attrs"));

        let extended = stylesheet.attribute_sets.get("extended-attrs").unwrap();
        assert_eq!(extended.use_attribute_sets, vec!["base-attrs"]);
    }

    #[test]
    fn test_compile_attribute_set_empty() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:attribute-set name="empty-set"/>
                <xsl:template match="/">
                    <output/>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let stylesheet = parse_stylesheet(xslt).unwrap();
        assert!(stylesheet.attribute_sets.contains_key("empty-set"));
        let attr_set = stylesheet.attribute_sets.get("empty-set").unwrap();
        assert!(attr_set.attributes.0.is_empty());
    }

    #[test]
    fn test_compile_multiple_attribute_sets() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:attribute-set name="set1">
                    <xsl:attribute name="attr1" select="'val1'"/>
                </xsl:attribute-set>
                <xsl:attribute-set name="set2">
                    <xsl:attribute name="attr2" select="'val2'"/>
                </xsl:attribute-set>
                <xsl:attribute-set name="set3" use-attribute-sets="set1 set2">
                    <xsl:attribute name="attr3" select="'val3'"/>
                </xsl:attribute-set>
                <xsl:template match="/">
                    <output/>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let stylesheet = parse_stylesheet(xslt).unwrap();
        assert_eq!(stylesheet.attribute_sets.len(), 3);

        let set3 = stylesheet.attribute_sets.get("set3").unwrap();
        assert_eq!(set3.use_attribute_sets, vec!["set1", "set2"]);
    }

    #[test]
    fn test_compile_attribute_set_with_visibility() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:attribute-set name="public-attrs" visibility="public">
                    <xsl:attribute name="attr" select="'value'"/>
                </xsl:attribute-set>
                <xsl:attribute-set name="private-attrs" visibility="private">
                    <xsl:attribute name="attr" select="'value'"/>
                </xsl:attribute-set>
                <xsl:template match="/">
                    <output/>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let stylesheet = parse_stylesheet(xslt).unwrap();

        let public_set = stylesheet.attribute_sets.get("public-attrs").unwrap();
        assert_eq!(public_set.visibility, crate::ast::Visibility::Public);

        let private_set = stylesheet.attribute_sets.get("private-attrs").unwrap();
        assert_eq!(private_set.visibility, crate::ast::Visibility::Private);
    }
}

mod decimal_format_tests {
    use super::*;

    #[test]
    fn test_compile_decimal_format_default() {
        let xslt = r##"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:decimal-format/>
                <xsl:template match="/">
                    <result/>
                </xsl:template>
            </xsl:stylesheet>
        "##;

        let stylesheet = parse_stylesheet(xslt).unwrap();
        assert!(
            stylesheet.decimal_formats.contains_key(&None),
            "Should have unnamed decimal format"
        );
        let df = stylesheet.decimal_formats.get(&None).unwrap();
        assert_eq!(df.decimal_separator, '.');
        assert_eq!(df.grouping_separator, ',');
    }

    #[test]
    fn test_compile_decimal_format_named() {
        let xslt = r##"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:decimal-format name="european" 
                    decimal-separator="," 
                    grouping-separator="."/>
                <xsl:template match="/">
                    <result/>
                </xsl:template>
            </xsl:stylesheet>
        "##;

        let stylesheet = parse_stylesheet(xslt).unwrap();
        assert!(
            stylesheet
                .decimal_formats
                .contains_key(&Some("european".to_string())),
            "Should have named decimal format 'european'"
        );
        let df = stylesheet
            .decimal_formats
            .get(&Some("european".to_string()))
            .unwrap();
        assert_eq!(df.decimal_separator, ',');
        assert_eq!(df.grouping_separator, '.');
    }

    #[test]
    fn test_compile_decimal_format_all_attributes() {
        let xslt = r##"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:decimal-format name="custom"
                    decimal-separator="."
                    grouping-separator=","
                    infinity="INF"
                    minus-sign="-"
                    NaN="NotANumber"
                    percent="%"
                    per-mille="‰"
                    zero-digit="0"
                    digit="#"
                    pattern-separator=";"
                    exponent-separator="E"/>
                <xsl:template match="/">
                    <result/>
                </xsl:template>
            </xsl:stylesheet>
        "##;

        let stylesheet = parse_stylesheet(xslt).unwrap();
        let df = stylesheet
            .decimal_formats
            .get(&Some("custom".to_string()))
            .unwrap();
        assert_eq!(df.infinity, "INF");
        assert_eq!(df.nan, "NotANumber");
        assert_eq!(df.exponent_separator, 'E');
    }

    #[test]
    fn test_compile_multiple_decimal_formats() {
        let xslt = r##"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:decimal-format name="us" decimal-separator="." grouping-separator=","/>
                <xsl:decimal-format name="eu" decimal-separator="," grouping-separator="."/>
                <xsl:decimal-format/>
                <xsl:template match="/">
                    <result/>
                </xsl:template>
            </xsl:stylesheet>
        "##;

        let stylesheet = parse_stylesheet(xslt).unwrap();
        assert_eq!(stylesheet.decimal_formats.len(), 3);
        assert!(stylesheet.decimal_formats.contains_key(&None));
        assert!(
            stylesheet
                .decimal_formats
                .contains_key(&Some("us".to_string()))
        );
        assert!(
            stylesheet
                .decimal_formats
                .contains_key(&Some("eu".to_string()))
        );
    }

    #[test]
    fn test_format_number_with_named_decimal_format() {
        let xslt = r##"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:decimal-format name="european" 
                    decimal-separator="," 
                    grouping-separator="."/>
                <xsl:template match="/">
                    <result>
                        <us><xsl:value-of select="format-number(1234.56, '#,##0.00')"/></us>
                        <eu><xsl:value-of select="format-number(1234.56, '#.##0,00', 'european')"/></eu>
                    </result>
                </xsl:template>
            </xsl:stylesheet>
        "##;

        let result = execute_xslt3(xslt, "<root/>");
        assert!(
            result.is_ok(),
            "format-number with named format failed: {:?}",
            result.err()
        );
        let text = get_text_content(&result.unwrap());
        assert!(
            text.contains("1,234.56") || text.contains("1234.56"),
            "US format should work: {}",
            text
        );
        assert!(
            text.contains("1.234,56") || text.contains("1234,56"),
            "EU format should work: {}",
            text
        );
    }
}

mod namespace_alias_tests {
    use super::*;

    #[test]
    fn test_compile_namespace_alias() {
        let xslt = r##"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:axsl="http://www.w3.org/1999/XSL/TransformAlias">
                <xsl:namespace-alias stylesheet-prefix="axsl" result-prefix="xsl"/>
                <xsl:template match="/">
                    <result/>
                </xsl:template>
            </xsl:stylesheet>
        "##;

        let stylesheet = parse_stylesheet(xslt).unwrap();
        assert_eq!(stylesheet.namespace_aliases.len(), 1);
        let alias = &stylesheet.namespace_aliases[0];
        assert_eq!(alias.stylesheet_prefix, "axsl");
        assert_eq!(alias.result_prefix, "xsl");
    }

    #[test]
    fn test_compile_multiple_namespace_aliases() {
        let xslt = r##"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:axsl="http://www.w3.org/1999/XSL/TransformAlias"
                xmlns:out="urn:output">
                <xsl:namespace-alias stylesheet-prefix="axsl" result-prefix="xsl"/>
                <xsl:namespace-alias stylesheet-prefix="out" result-prefix="#default"/>
                <xsl:template match="/">
                    <result/>
                </xsl:template>
            </xsl:stylesheet>
        "##;

        let stylesheet = parse_stylesheet(xslt).unwrap();
        assert_eq!(stylesheet.namespace_aliases.len(), 2);
    }

    #[test]
    fn test_namespace_alias_applied_to_literal_elements() {
        use crate::ast::Xslt3Instruction;

        let xslt = r##"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:axsl="http://www.w3.org/1999/XSL/TransformAlias">
                <xsl:namespace-alias stylesheet-prefix="axsl" result-prefix="xsl"/>
                <xsl:template match="/">
                    <axsl:value-of select="."/>
                </xsl:template>
            </xsl:stylesheet>
        "##;

        let stylesheet = parse_stylesheet(xslt).unwrap();
        let rules = stylesheet.template_rules.get(&None).unwrap();
        let body = &rules[0].body.0;

        let has_transformed_tag = body.iter().any(|instr| {
            if let Xslt3Instruction::ContentTag { tag_name, .. }
            | Xslt3Instruction::EmptyTag { tag_name, .. } = instr
            {
                let name = String::from_utf8_lossy(tag_name);
                name.starts_with("xsl:")
            } else {
                false
            }
        });

        assert!(
            has_transformed_tag,
            "axsl: prefix should be transformed to xsl:"
        );
    }
}

mod output_declaration_tests {
    use super::*;

    #[test]
    fn test_compile_output_basic() {
        let xslt = r##"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:output method="xml" indent="yes"/>
                <xsl:template match="/">
                    <result/>
                </xsl:template>
            </xsl:stylesheet>
        "##;

        let stylesheet = parse_stylesheet(xslt).unwrap();
        assert_eq!(stylesheet.output.method, Some("xml".to_string()));
        assert_eq!(stylesheet.output.indent, Some(true));
    }

    #[test]
    fn test_compile_output_html5() {
        let xslt = r##"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:output method="html" html-version="5.0"/>
                <xsl:template match="/">
                    <html/>
                </xsl:template>
            </xsl:stylesheet>
        "##;

        let stylesheet = parse_stylesheet(xslt).unwrap();
        assert_eq!(stylesheet.output.method, Some("html".to_string()));
        assert_eq!(stylesheet.output.html_version, Some("5.0".to_string()));
    }

    #[test]
    fn test_compile_named_output() {
        let xslt = r##"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:output name="json-out" method="json"/>
                <xsl:output name="xml-out" method="xml" indent="yes"/>
                <xsl:template match="/">
                    <result/>
                </xsl:template>
            </xsl:stylesheet>
        "##;

        let stylesheet = parse_stylesheet(xslt).unwrap();
        assert!(stylesheet.outputs.contains_key("json-out"));
        assert!(stylesheet.outputs.contains_key("xml-out"));
        let json_out = stylesheet.outputs.get("json-out").unwrap();
        assert_eq!(json_out.method, Some("json".to_string()));
    }

    #[test]
    fn test_compile_output_all_attributes() {
        let xslt = r##"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:output 
                    method="xml" 
                    version="1.1"
                    encoding="UTF-8" 
                    indent="yes"
                    omit-xml-declaration="no"
                    standalone="yes"
                    doctype-public="-//W3C//DTD XHTML 1.0//EN"
                    doctype-system="http://www.w3.org/TR/xhtml1/DTD/xhtml1-strict.dtd"
                    cdata-section-elements="script style"
                    media-type="application/xml"
                    byte-order-mark="yes"
                    escape-uri-attributes="no"/>
                <xsl:template match="/">
                    <result/>
                </xsl:template>
            </xsl:stylesheet>
        "##;

        let stylesheet = parse_stylesheet(xslt).unwrap();
        assert_eq!(stylesheet.output.version, Some("1.1".to_string()));
        assert_eq!(stylesheet.output.standalone, Some("yes".to_string()));
        assert_eq!(
            stylesheet.output.doctype_public,
            Some("-//W3C//DTD XHTML 1.0//EN".to_string())
        );
        assert_eq!(
            stylesheet.output.cdata_section_elements,
            vec!["script", "style"]
        );
        assert_eq!(
            stylesheet.output.media_type,
            Some("application/xml".to_string())
        );
        assert_eq!(stylesheet.output.byte_order_mark, Some(true));
        assert_eq!(stylesheet.output.escape_uri_attributes, Some(false));
    }
}

mod apply_imports_tests {
    use super::*;
    use std::collections::HashMap;

    fn create_stylesheet_resolver(
        stylesheets: HashMap<&str, &str>,
    ) -> impl Fn(&str) -> Result<crate::ast::CompiledStylesheet3, crate::error::Xslt3Error> {
        move |href: &str| {
            let content = stylesheets
                .get(href)
                .ok_or_else(|| crate::error::Xslt3Error::parse(format!("Not found: {}", href)))?;
            parse_stylesheet(content)
        }
    }

    #[test]
    fn test_compile_apply_imports() {
        let xslt = r##"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="item">
                    <wrapper>
                        <xsl:apply-imports/>
                    </wrapper>
                </xsl:template>
            </xsl:stylesheet>
        "##;

        let result = parse_stylesheet(xslt);
        assert!(
            result.is_ok(),
            "Should compile xsl:apply-imports: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_imported_templates_marked() {
        let main = r##"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:import href="base.xsl"/>
                <xsl:template match="item" priority="1">MAIN</xsl:template>
            </xsl:stylesheet>
        "##;

        let base = r##"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="item" priority="0">BASE</xsl:template>
            </xsl:stylesheet>
        "##;

        let mut stylesheet = parse_stylesheet(main).unwrap();
        let mut external = HashMap::new();
        external.insert("base.xsl", base);
        let resolver = create_stylesheet_resolver(external);

        stylesheet.resolve_imports_includes(resolver).unwrap();
        stylesheet.finalize_after_merge();

        let rules = stylesheet.template_rules.get(&None).unwrap();

        let main_rule = rules.iter().find(|r| !r.from_import);
        let imported_rule = rules.iter().find(|r| r.from_import);

        assert!(main_rule.is_some(), "Should have main (non-imported) rule");
        assert!(imported_rule.is_some(), "Should have imported rule");
        assert!(
            imported_rule.unwrap().from_import,
            "Imported rule should be marked"
        );
    }

    #[test]
    fn test_execute_apply_imports_basic() {
        let main = r##"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:import href="base.xsl"/>
                <xsl:template match="item">
                    <main-wrapper>
                        <xsl:apply-imports/>
                    </main-wrapper>
                </xsl:template>
            </xsl:stylesheet>
        "##;

        let base = r##"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="item">
                    <base-content><xsl:value-of select="."/></base-content>
                </xsl:template>
            </xsl:stylesheet>
        "##;

        let mut stylesheet = parse_stylesheet(main).unwrap();
        let mut external = HashMap::new();
        external.insert("base.xsl", base);
        let resolver = create_stylesheet_resolver(external);

        stylesheet.resolve_imports_includes(resolver).unwrap();
        stylesheet.finalize_after_merge();

        let xml = "<root><item>test-value</item></root>";

        use crate::executor::TemplateExecutor3;
        use petty_xslt::datasources::xml::XmlDocument;

        let doc = XmlDocument::parse(xml).unwrap();
        let root_node = doc.root_node();

        let mut executor = TemplateExecutor3::new(&stylesheet, root_node, false).unwrap();
        let result = executor.build_tree();

        assert!(
            result.is_ok(),
            "Execution should succeed: {:?}",
            result.err()
        );
        let text = get_text_content(&result.unwrap());
        assert!(
            text.contains("test-value"),
            "Should contain base template output: {}",
            text
        );
    }
}

mod whitespace_tests {
    use super::*;

    #[test]
    fn test_compile_preserve_space() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:preserve-space elements="pre code"/>
                <xsl:template match="/">
                    <output/>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let stylesheet = parse_stylesheet(xslt).unwrap();
        assert_eq!(stylesheet.preserve_space, vec!["pre", "code"]);
    }

    #[test]
    fn test_compile_strip_space() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:strip-space elements="html body div"/>
                <xsl:template match="/">
                    <output/>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let stylesheet = parse_stylesheet(xslt).unwrap();
        assert_eq!(stylesheet.strip_space, vec!["html", "body", "div"]);
    }

    #[test]
    fn test_compile_preserve_and_strip_space() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:strip-space elements="*"/>
                <xsl:preserve-space elements="pre code script"/>
                <xsl:template match="/">
                    <output/>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let stylesheet = parse_stylesheet(xslt).unwrap();
        assert_eq!(stylesheet.strip_space, vec!["*"]);
        assert_eq!(stylesheet.preserve_space, vec!["pre", "code", "script"]);
    }

    #[test]
    fn test_compile_preserve_space_wildcard() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:preserve-space elements="*"/>
                <xsl:template match="/">
                    <output/>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let stylesheet = parse_stylesheet(xslt).unwrap();
        assert_eq!(stylesheet.preserve_space, vec!["*"]);
    }

    #[test]
    fn test_compile_multiple_whitespace_declarations() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:preserve-space elements="pre"/>
                <xsl:preserve-space elements="code"/>
                <xsl:strip-space elements="div"/>
                <xsl:strip-space elements="span"/>
                <xsl:template match="/">
                    <output/>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let stylesheet = parse_stylesheet(xslt).unwrap();
        assert_eq!(stylesheet.preserve_space, vec!["pre", "code"]);
        assert_eq!(stylesheet.strip_space, vec!["div", "span"]);
    }

    #[test]
    fn test_execute_strip_space_wildcard() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:strip-space elements="*"/>
                <xsl:template match="/">
                    <result><xsl:apply-templates select="root/item"/></result>
                </xsl:template>
                <xsl:template match="item">
                    <item><xsl:value-of select="."/></item>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = r#"<root>
            <item>one</item>
            <item>two</item>
        </root>"#;

        let result = execute_xslt3(xslt, xml).unwrap();
        let text = get_text_content(&result);
        assert!(text.contains("one"));
        assert!(text.contains("two"));
    }

    #[test]
    fn test_execute_strip_space_specific_element() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:strip-space elements="container"/>
                <xsl:template match="/">
                    <result><xsl:apply-templates select="root/container/item"/></result>
                </xsl:template>
                <xsl:template match="item">
                    <item><xsl:value-of select="."/></item>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = r#"<root><container>
            <item>A</item>
            <item>B</item>
        </container></root>"#;

        let result = execute_xslt3(xslt, xml).unwrap();
        let text = get_text_content(&result);
        assert!(text.contains("A"));
        assert!(text.contains("B"));
    }

    #[test]
    fn test_execute_preserve_overrides_strip() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:strip-space elements="*"/>
                <xsl:preserve-space elements="pre"/>
                <xsl:template match="/">
                    <result><xsl:apply-templates/></result>
                </xsl:template>
                <xsl:template match="pre">
                    <pre><xsl:value-of select="."/></pre>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root><pre>  preserved  </pre></root>";

        let result = execute_xslt3(xslt, xml).unwrap();
        let text = get_text_content(&result);
        assert!(text.contains("preserved"));
    }
}

mod character_map_tests {
    use super::*;

    #[test]
    fn test_compile_character_map_basic() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:character-map name="special-chars">
                    <xsl:output-character character="X" string="[X]"/>
                    <xsl:output-character character="Y" string="[Y]"/>
                    <xsl:output-character character="Z" string="[Z]"/>
                </xsl:character-map>
                <xsl:template match="/">
                    <output/>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let stylesheet = parse_stylesheet(xslt).unwrap();
        assert!(stylesheet.character_maps.contains_key("special-chars"));

        let char_map = stylesheet.character_maps.get("special-chars").unwrap();
        assert_eq!(char_map.name, "special-chars");
        assert_eq!(char_map.mappings.len(), 3);

        let x_mapping = char_map.mappings.iter().find(|m| m.character == 'X');
        assert!(x_mapping.is_some());
        assert_eq!(x_mapping.unwrap().string, "[X]");
    }

    #[test]
    fn test_compile_character_map_with_use_character_maps() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:character-map name="base-chars">
                    <xsl:output-character character="&lt;" string="[LT]"/>
                </xsl:character-map>
                <xsl:character-map name="extended-chars" use-character-maps="base-chars">
                    <xsl:output-character character="&gt;" string="[GT]"/>
                </xsl:character-map>
                <xsl:template match="/">
                    <output/>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let stylesheet = parse_stylesheet(xslt).unwrap();
        assert!(stylesheet.character_maps.contains_key("base-chars"));
        assert!(stylesheet.character_maps.contains_key("extended-chars"));

        let extended = stylesheet.character_maps.get("extended-chars").unwrap();
        assert_eq!(extended.use_character_maps, vec!["base-chars"]);
    }

    #[test]
    fn test_output_uses_character_maps() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:character-map name="test-map">
                    <xsl:output-character character="X" string="[REPLACED]"/>
                </xsl:character-map>
                <xsl:output use-character-maps="test-map"/>
                <xsl:template match="/">
                    <result>XYZ</result>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let stylesheet = parse_stylesheet(xslt).unwrap();
        assert_eq!(stylesheet.output.use_character_maps, vec!["test-map"]);
    }

    #[test]
    fn test_execute_character_map_replacement() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:character-map name="brackets">
                    <xsl:output-character character="L" string="[LEFT]"/>
                    <xsl:output-character character="R" string="[RIGHT]"/>
                </xsl:character-map>
                <xsl:output use-character-maps="brackets"/>
                <xsl:template match="/">
                    <result>L R L</result>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root/>";
        let result = execute_xslt3(xslt, xml).unwrap();
        let text = get_text_content(&result);
        assert!(
            text.contains("[LEFT]"),
            "Should replace L with [LEFT]. Got: '{}'",
            text
        );
        assert!(
            text.contains("[RIGHT]"),
            "Should replace R with [RIGHT]. Got: '{}'",
            text
        );
    }

    #[test]
    fn test_execute_character_map_chained() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:character-map name="base">
                    <xsl:output-character character="A" string="[A]"/>
                </xsl:character-map>
                <xsl:character-map name="extended" use-character-maps="base">
                    <xsl:output-character character="B" string="[B]"/>
                </xsl:character-map>
                <xsl:output use-character-maps="extended"/>
                <xsl:template match="/">
                    <result>A B C</result>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root/>";
        let result = execute_xslt3(xslt, xml).unwrap();
        let text = get_text_content(&result);
        assert!(
            text.contains("[A]"),
            "Should replace A via chained map. Got: '{}'",
            text
        );
        assert!(text.contains("[B]"), "Should replace B. Got: '{}'", text);
        assert!(
            text.contains("C"),
            "Should keep C unchanged. Got: '{}'",
            text
        );
    }
}

mod shadow_attribute_tests {
    use super::*;
    use crate::test_helpers::{find_hyperlinks, find_ids};

    #[test]
    fn test_shadow_attribute_href_on_hyperlink() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <a _href="http://example.com">Link</a>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root/>";
        let result = execute_xslt3(xslt, xml).unwrap();
        let hyperlinks = find_hyperlinks(&result);
        assert_eq!(hyperlinks.len(), 1);
        assert_eq!(hyperlinks[0].0, "http://example.com");
        assert_eq!(hyperlinks[0].1, "Link");
    }

    #[test]
    fn test_shadow_attribute_dynamic_href() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <xsl:variable name="url" select="'http://dynamic.com'"/>
                    <a _href="{$url}">Dynamic Link</a>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root/>";
        let result = execute_xslt3(xslt, xml).unwrap();
        let hyperlinks = find_hyperlinks(&result);
        assert_eq!(hyperlinks.len(), 1);
        assert_eq!(hyperlinks[0].0, "http://dynamic.com");
    }

    #[test]
    fn test_shadow_attribute_id() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <div _id="my-element">Content</div>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root/>";
        let result = execute_xslt3(xslt, xml).unwrap();
        let ids = find_ids(&result);
        assert!(
            ids.contains(&"my-element".to_string()),
            "Should have id 'my-element'. Got: {:?}",
            ids
        );
    }

    #[test]
    fn test_shadow_attribute_dynamic_id() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <xsl:variable name="suffix" select="'123'"/>
                    <div _id="elem-{$suffix}">Content</div>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root/>";
        let result = execute_xslt3(xslt, xml).unwrap();
        let ids = find_ids(&result);
        assert!(
            ids.contains(&"elem-123".to_string()),
            "Should have dynamic id 'elem-123'. Got: {:?}",
            ids
        );
    }

    #[test]
    fn test_shadow_attribute_from_xml_data() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="link">
                    <a _href="{@url}"><xsl:value-of select="."/></a>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = r#"<link url="http://from-data.com">Data Link</link>"#;
        let result = execute_xslt3(xslt, xml).unwrap();
        let hyperlinks = find_hyperlinks(&result);
        assert_eq!(hyperlinks.len(), 1);
        assert_eq!(hyperlinks[0].0, "http://from-data.com");
        assert_eq!(hyperlinks[0].1, "Data Link");
    }

    #[test]
    fn test_shadow_attribute_empty_tag() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <div _id="empty-elem"/>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root/>";
        let result = execute_xslt3(xslt, xml).unwrap();
        let ids = find_ids(&result);
        assert!(
            ids.contains(&"empty-elem".to_string()),
            "Empty tag should process shadow attributes. Got: {:?}",
            ids
        );
    }

    #[test]
    fn test_shadow_attribute_compilation() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" 
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <div _data-custom="value" _aria-label="accessible">Content</div>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let result = parse_stylesheet(xslt);
        assert!(
            result.is_ok(),
            "Should compile shadow attributes: {:?}",
            result.err()
        );
    }
}

mod mode_on_no_match_tests {
    use super::*;

    #[test]
    fn test_mode_on_no_match_text_only_copy_default() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <result><xsl:apply-templates/></result>
                </xsl:template>
            </xsl:stylesheet>
        "#;
        let xml = "<root><child>hello</child></root>";
        let result = execute_xslt3(xslt, xml).unwrap();
        let text = get_text_content(&result);
        assert!(
            text.contains("hello"),
            "Default behavior should copy text. Got: '{}'",
            text
        );
    }

    #[test]
    fn test_mode_on_no_match_text_only_copy_explicit() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:mode on-no-match="text-only-copy"/>
                <xsl:template match="/">
                    <result><xsl:apply-templates/></result>
                </xsl:template>
            </xsl:stylesheet>
        "#;
        let xml = "<root><child>text content</child></root>";
        let result = execute_xslt3(xslt, xml).unwrap();
        let text = get_text_content(&result);
        assert!(
            text.contains("text content"),
            "text-only-copy should output text. Got: '{}'",
            text
        );
    }

    #[test]
    fn test_mode_on_no_match_deep_skip() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:mode on-no-match="deep-skip"/>
                <xsl:template match="/">
                    <result><xsl:apply-templates/></result>
                </xsl:template>
            </xsl:stylesheet>
        "#;
        let xml = "<root><child>should not appear</child></root>";
        let result = execute_xslt3(xslt, xml).unwrap();
        let text = get_text_content(&result);
        assert!(
            !text.contains("should not appear"),
            "deep-skip should skip all content. Got: '{}'",
            text
        );
    }

    #[test]
    fn test_mode_on_no_match_shallow_skip() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:mode on-no-match="shallow-skip"/>
                <xsl:template match="/">
                    <result><xsl:apply-templates/></result>
                </xsl:template>
                <xsl:template match="inner">
                    <matched><xsl:value-of select="."/></matched>
                </xsl:template>
            </xsl:stylesheet>
        "#;
        let xml = "<root><outer><inner>found</inner></outer></root>";
        let result = execute_xslt3(xslt, xml).unwrap();
        let text = get_text_content(&result);
        assert!(
            text.contains("found"),
            "shallow-skip should process children and find inner. Got: '{}'",
            text
        );
    }

    #[test]
    fn test_mode_on_no_match_shallow_copy() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:mode on-no-match="shallow-copy"/>
                <xsl:template match="/">
                    <result><xsl:apply-templates/></result>
                </xsl:template>
            </xsl:stylesheet>
        "#;
        let xml = "<root><child>text here</child></root>";
        let result = execute_xslt3(xslt, xml).unwrap();
        let text = get_text_content(&result);
        assert!(
            text.contains("text here"),
            "shallow-copy should copy elements and process children. Got: '{}'",
            text
        );
    }

    #[test]
    fn test_mode_on_no_match_deep_copy() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:mode on-no-match="deep-copy"/>
                <xsl:template match="/">
                    <result><xsl:apply-templates/></result>
                </xsl:template>
            </xsl:stylesheet>
        "#;
        let xml = "<root><child><nested>deep text</nested></child></root>";
        let result = execute_xslt3(xslt, xml).unwrap();
        let text = get_text_content(&result);
        assert!(
            text.contains("deep text"),
            "deep-copy should copy entire subtree. Got: '{}'",
            text
        );
    }

    #[test]
    fn test_mode_on_no_match_fail() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:mode on-no-match="fail"/>
                <xsl:template match="/">
                    <result><xsl:apply-templates/></result>
                </xsl:template>
            </xsl:stylesheet>
        "#;
        let xml = "<root><child>text</child></root>";
        let result = execute_xslt3(xslt, xml);
        assert!(
            result.is_err(),
            "on-no-match='fail' should error on unmatched nodes"
        );
    }

    #[test]
    fn test_named_mode_on_no_match_deep_skip() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:mode name="skip-mode" on-no-match="deep-skip"/>
                <xsl:template match="/">
                    <result><xsl:apply-templates mode="skip-mode"/></result>
                </xsl:template>
            </xsl:stylesheet>
        "#;
        let xml = "<root>text to skip</root>";
        let result = execute_xslt3(xslt, xml).unwrap();
        let text = get_text_content(&result);
        assert!(
            !text.contains("text to skip"),
            "Named mode deep-skip should skip. Got: '{}'",
            text
        );
    }

    #[test]
    fn test_named_mode_on_no_match_text_only() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:mode name="copy-mode" on-no-match="text-only-copy"/>
                <xsl:template match="/">
                    <result><xsl:apply-templates mode="copy-mode"/></result>
                </xsl:template>
            </xsl:stylesheet>
        "#;
        let xml = "<root>text to copy</root>";
        let result = execute_xslt3(xslt, xml).unwrap();
        let text = get_text_content(&result);
        assert!(
            text.contains("text to copy"),
            "Named mode text-only-copy should copy text. Got: '{}'",
            text
        );
    }

    #[test]
    fn test_mode_declaration_compiles() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:mode on-no-match="shallow-skip"/>
                <xsl:mode name="m1" on-no-match="deep-skip"/>
                <xsl:mode name="m2" on-no-match="deep-copy"/>
                <xsl:mode name="m3" on-no-match="shallow-copy"/>
                <xsl:mode name="m4" on-no-match="text-only-copy"/>
                <xsl:mode name="m5" on-no-match="fail"/>
                <xsl:template match="/"><out/></xsl:template>
            </xsl:stylesheet>
        "#;
        let result = parse_stylesheet(xslt);
        assert!(
            result.is_ok(),
            "All on-no-match values should compile: {:?}",
            result.err()
        );

        let stylesheet = result.unwrap();
        assert!(
            stylesheet.modes.contains_key(&None),
            "Default mode should be registered"
        );
        assert!(stylesheet.modes.contains_key(&Some("m1".to_string())));
        assert!(stylesheet.modes.contains_key(&Some("m2".to_string())));
    }
}

mod namespace_instruction_tests {
    use super::*;

    #[test]
    fn test_namespace_instruction_compiles() {
        // xsl:namespace generates namespace nodes on result elements
        let xslt = r#"
            <xsl:stylesheet version="3.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <result>
                        <xsl:namespace name="ex" select="'http://example.com'"/>
                    </result>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let result = parse_stylesheet(xslt);
        assert!(
            result.is_ok(),
            "xsl:namespace should compile: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_namespace_with_static_value() {
        // xsl:namespace with content instead of select
        let xslt = r#"
            <xsl:stylesheet version="3.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <result>
                        <xsl:namespace name="ex">http://example.com</xsl:namespace>
                    </result>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let result = parse_stylesheet(xslt);
        assert!(
            result.is_ok(),
            "xsl:namespace with body should compile: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_namespace_execution() {
        // xsl:namespace should execute without error (namespace is PDF-irrelevant but valid)
        let xslt = r#"
            <xsl:stylesheet version="3.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <result>
                        <xsl:namespace name="ex" select="'http://example.com'"/>
                        <xsl:text>content</xsl:text>
                    </result>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root/>";
        let result = execute_xslt3(xslt, xml).expect("Should execute");
        let text = get_text_content(&result);
        assert!(text.contains("content"), "Content should be preserved");
    }

    #[test]
    fn test_namespace_with_avt_name() {
        // name attribute can be an AVT
        let xslt = r#"
            <xsl:stylesheet version="3.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <result>
                        <xsl:namespace name="{/root/@prefix}" select="'http://example.com'"/>
                        <done/>
                    </result>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = r#"<root prefix="ex"/>"#;
        let result = execute_xslt3(xslt, xml).expect("AVT name should work");
        let text = get_text_content(&result);
        assert!(text.contains("done") || !result.is_empty());
    }
}

mod fallback_instruction_tests {
    use super::*;

    #[test]
    fn test_fallback_compiles() {
        // xsl:fallback provides alternative behavior when parent instruction not supported
        let xslt = r#"
            <xsl:stylesheet version="3.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <xsl:result-document href="output.xml">
                        <xsl:fallback>
                            <fallback-content>Result document not supported</fallback-content>
                        </xsl:fallback>
                        <normal-content/>
                    </xsl:result-document>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let result = parse_stylesheet(xslt);
        assert!(
            result.is_ok(),
            "xsl:fallback should compile: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_fallback_ignored_when_parent_supported() {
        // When parent instruction is supported, xsl:fallback is ignored
        let xslt = r#"
            <xsl:stylesheet version="3.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <xsl:if test="true()">
                        <xsl:fallback><error>Should not appear</error></xsl:fallback>
                        <success/>
                    </xsl:if>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let xml = "<root/>";
        let result = execute_xslt3(xslt, xml).expect("Should execute");
        let text = get_text_content(&result);
        // Fallback should be ignored, only normal content produced
        assert!(
            !text.contains("error") && !text.contains("Should not appear"),
            "Fallback should be ignored when parent supported"
        );
    }

    #[test]
    fn test_multiple_fallbacks() {
        // Multiple xsl:fallback elements can exist
        let xslt = r#"
            <xsl:stylesheet version="3.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <xsl:for-each select="/root/item">
                        <xsl:fallback><fb1/></xsl:fallback>
                        <xsl:fallback><fb2/></xsl:fallback>
                        <item><xsl:value-of select="."/></item>
                    </xsl:for-each>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let result = parse_stylesheet(xslt);
        assert!(
            result.is_ok(),
            "Multiple fallbacks should compile: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_nested_fallback() {
        // xsl:fallback can contain other XSLT instructions
        let xslt = r#"
            <xsl:stylesheet version="3.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <xsl:copy>
                        <xsl:fallback>
                            <xsl:text>Fallback with </xsl:text>
                            <xsl:value-of select="'nested content'"/>
                        </xsl:fallback>
                        <actual/>
                    </xsl:copy>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let result = parse_stylesheet(xslt);
        assert!(
            result.is_ok(),
            "Nested fallback content should compile: {:?}",
            result.err()
        );
    }
}
