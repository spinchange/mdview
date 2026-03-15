use comrak::nodes::{AstNode, NodeCode, NodeHeading, NodeValue};
use comrak::{format_html, parse_document, Arena, ComrakOptions};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeadingSpan {
    pub level: u8,
    pub text: String,
    pub line_start: usize,
    pub line_end: usize,
    pub column_start: usize,
    pub column_end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderedDocument {
    pub html: String,
    pub headings: Vec<HeadingSpan>,
    pub is_blank: bool,
}

pub struct MarkdownEngine {
    options: ComrakOptions,
}

impl Default for MarkdownEngine {
    fn default() -> Self {
        let mut options = ComrakOptions::default();
        options.extension.table = true;
        options.extension.tasklist = true;
        options.extension.strikethrough = true;
        options.extension.autolink = true;
        options.extension.tagfilter = true;
        options.render.unsafe_ = false;
        options.render.sourcepos = true;
        Self { options }
    }
}

impl MarkdownEngine {
    pub fn new(options: ComrakOptions) -> Self {
        Self { options }
    }

    pub fn render(&self, source: &str) -> RenderedDocument {
        if source.trim().is_empty() {
            return RenderedDocument {
                html: String::new(),
                headings: Vec::new(),
                is_blank: true,
            };
        }

        let arena = Arena::new();
        let root = parse_document(&arena, source, &self.options);

        let mut html_bytes = Vec::new();
        format_html(root, &self.options, &mut html_bytes).expect("failed to render markdown");

        let headings = collect_headings(root);
        let html = String::from_utf8(html_bytes).expect("comrak produced non-utf8 html");

        RenderedDocument {
            html,
            headings,
            is_blank: false,
        }
    }
}

fn collect_headings<'a>(root: &'a AstNode<'a>) -> Vec<HeadingSpan> {
    let mut out = Vec::new();
    walk(root, &mut |node| {
        if let NodeValue::Heading(NodeHeading { level, .. }) = &node.data.borrow().value {
            let sourcepos = node.data.borrow().sourcepos;
            out.push(HeadingSpan {
                level: *level,
                text: normalize_whitespace(&heading_text(node)),
                line_start: sourcepos.start.line,
                line_end: sourcepos.end.line,
                column_start: sourcepos.start.column,
                column_end: sourcepos.end.column,
            });
        }
    });
    out
}

fn heading_text<'a>(node: &'a AstNode<'a>) -> String {
    let mut out = String::new();
    walk(node, &mut |child| match &child.data.borrow().value {
        NodeValue::Text(text) => out.push_str(text),
        NodeValue::Code(NodeCode { literal, .. }) => out.push_str(literal),
        NodeValue::LineBreak | NodeValue::SoftBreak => out.push(' '),
        _ => {}
    });
    out
}

fn normalize_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn walk<'a>(node: &'a AstNode<'a>, f: &mut impl FnMut(&'a AstNode<'a>)) {
    f(node);
    for child in node.children() {
        walk(child, f);
    }
}

#[cfg(test)]
mod tests {
    use super::MarkdownEngine;

    #[test]
    fn extracts_heading_source_positions() {
        let doc = "# Title\n\n## Section Two\nBody text\n";
        let rendered = MarkdownEngine::default().render(doc);

        assert_eq!(rendered.headings.len(), 2);
        assert_eq!(rendered.headings[0].text, "Title");
        assert_eq!(rendered.headings[0].line_start, 1);
        assert_eq!(rendered.headings[1].text, "Section Two");
        assert_eq!(rendered.headings[1].line_start, 3);
    }

    #[test]
    fn blocks_unsafe_raw_html() {
        let doc = "<script>alert('x')</script>\n\n# Safe";
        let rendered = MarkdownEngine::default().render(doc);

        assert!(!rendered.html.contains("<script>"));
        assert!(rendered.html.contains("<!-- raw HTML omitted -->"));
        assert!(rendered.html.contains("Safe"));
    }

    #[test]
    fn handles_blank_input_without_markup_noise() {
        let rendered = MarkdownEngine::default().render("  \n\t\r\n");
        assert!(rendered.is_blank);
        assert!(rendered.html.is_empty());
        assert!(rendered.headings.is_empty());
    }
}
