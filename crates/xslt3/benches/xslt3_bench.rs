use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use petty_xslt3::test_helpers::{execute_xslt3, parse_stylesheet};

fn simple_template() -> &'static str {
    r#"<xsl:stylesheet version="3.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
        <xsl:template match="/">
            <result>
                <xsl:for-each select="/items/item">
                    <row><xsl:value-of select="."/></row>
                </xsl:for-each>
            </result>
        </xsl:template>
    </xsl:stylesheet>"#
}

fn grouping_template() -> &'static str {
    r#"<xsl:stylesheet version="3.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
        <xsl:template match="/">
            <groups>
                <xsl:for-each-group select="/items/item" group-by="@category">
                    <group key="{current-grouping-key()}" count="{count(current-group())}">
                        <xsl:for-each select="current-group()">
                            <item><xsl:value-of select="."/></item>
                        </xsl:for-each>
                    </group>
                </xsl:for-each-group>
            </groups>
        </xsl:template>
    </xsl:stylesheet>"#
}

fn analyze_string_template() -> &'static str {
    r#"<xsl:stylesheet version="3.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
        <xsl:template match="/">
            <result>
                <xsl:analyze-string select="/data/text" regex="[0-9]+">
                    <xsl:matching-substring>
                        <num><xsl:value-of select="."/></num>
                    </xsl:matching-substring>
                    <xsl:non-matching-substring>
                        <text><xsl:value-of select="."/></text>
                    </xsl:non-matching-substring>
                </xsl:analyze-string>
            </result>
        </xsl:template>
    </xsl:stylesheet>"#
}

fn generate_items_xml(count: usize) -> String {
    let mut xml = String::from("<items>");
    for i in 0..count {
        let category = match i % 3 {
            0 => "A",
            1 => "B",
            _ => "C",
        };
        xml.push_str(&format!(
            r#"<item category="{}">Item {}</item>"#,
            category, i
        ));
    }
    xml.push_str("</items>");
    xml
}

fn benchmark_compilation(c: &mut Criterion) {
    let mut group = c.benchmark_group("xslt3_compilation");

    group.bench_function("simple_template", |b| {
        b.iter(|| parse_stylesheet(simple_template()).unwrap())
    });

    group.bench_function("grouping_template", |b| {
        b.iter(|| parse_stylesheet(grouping_template()).unwrap())
    });

    group.bench_function("analyze_string_template", |b| {
        b.iter(|| parse_stylesheet(analyze_string_template()).unwrap())
    });

    group.finish();
}

fn benchmark_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("xslt3_execution");

    for item_count in [10, 100, 1000] {
        let xml = generate_items_xml(item_count);

        group.bench_with_input(
            BenchmarkId::new("simple_foreach", item_count),
            &xml,
            |b, xml| b.iter(|| execute_xslt3(simple_template(), xml).unwrap()),
        );

        group.bench_with_input(BenchmarkId::new("grouping", item_count), &xml, |b, xml| {
            b.iter(|| execute_xslt3(grouping_template(), xml).unwrap())
        });
    }

    group.finish();
}

fn benchmark_analyze_string(c: &mut Criterion) {
    let mut group = c.benchmark_group("xslt3_analyze_string");

    for text_len in [100, 1000, 10000] {
        let text: String = (0..text_len)
            .map(|i| if i % 5 == 0 { '0' } else { 'a' })
            .collect();
        let xml = format!("<data><text>{}</text></data>", text);

        group.bench_with_input(
            BenchmarkId::new("analyze_string", text_len),
            &xml,
            |b, xml| b.iter(|| execute_xslt3(analyze_string_template(), xml).unwrap()),
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    benchmark_compilation,
    benchmark_execution,
    benchmark_analyze_string
);
criterion_main!(benches);
