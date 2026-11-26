//! First-party plugin for content features lsdocs adds of its own: things
//! neither docutils nor Sphinx has, which lsdocs' themes support out of
//! the box.
//!
//! It is a transformer that declares its directives as syntax extensions
//! (see "Syntax extensions" in `spec/wire.md`), so they work in every
//! parser that supports them: in reStructuredText (`.. tab-set::`) and in
//! Markdown in MyST's syntax (```` ```{tab-set} ````). The parsers turn
//! them into `custom` nodes; this plugin turns those into the custom nodes
//! renderers know (see "Custom nodes" in the HTML markup reference).
//!
//! Directives:
//!
//! | Directive | Result |
//! |---|---|
//! | `tab-set` | A `custom:tab-set` node whose children are `custom:tab-item` nodes. The names and options are sphinx-design's; `sync-group` and `class` are accepted and ignored. |
//! | `tab-item` | A `custom:tab-item` node; its argument is its label (the `label` attribute). `selected`, `sync`, `name`, `class-label` and `class-content` are accepted and ignored. |
//! | `button-link` | A `custom:button` node holding a `link` to its argument (a URL, or a path to a page, which the host resolves), with its content as the link's text (the URL if none). |
//! | `button-ref` | The same, to a page or a label: its argument is a document name (with or without its extension, relative to the page or, starting with `/`, to the source directory) or a label, as `ref-type` says (`any`, the default, tries a document first; `doc`; `ref`; `myst` is `any`). With no content, the link's text is the target's title. |
//!
//! Both take sphinx-design's options: `color`, `outline`, `expand` and
//! `align` become the `custom:button` node's attributes (`color` and
//! `align` as text, the others `true`); `tooltip` is the link's title;
//! `click-parent`, `shadow` and `class` are accepted and ignored.
//!
//! It has no settings. Its cache key covers the site's file list, which
//! `button-ref` reads.

use std::collections::BTreeMap;

use lsdocs_plugin_sdk::doctree::{Custom, Link, LinkTarget, Node, text};
use lsdocs_plugin_sdk::links::{UrlTarget, resolve_url};
use lsdocs_plugin_sdk::protocol::{
    ArgumentSyntax, ContentSyntax, DirectiveSyntax, OptionSyntax, RenderTransformEachParams,
    RenderTransformEachResult, Syntax,
};
use lsdocs_plugin_sdk::{Context, Plugin, PluginResult, Transformer};
use serde_json::{Value, json};

#[cfg(test)]
mod tests;

/// The plugin, ready to serve or to run in-process.
pub fn plugin() -> Plugin {
    Plugin::new("extras", env!("CARGO_PKG_VERSION"))
        .transformer(Extras)
        .syntax(|_| syntax())
        // `button-ref` looks document names up in the site's files, which
        // the host's cache keys leave out.
        .cache_key(|setup| {
            Some(setup.params.site.files.join(
                "
",
            ))
        })
}

/// The transformer.
#[derive(Debug, Clone, Copy, Default)]
pub struct Extras;

/// The directives the plugin handles.
const DIRECTIVES: &[&str] = &["tab-set", "tab-item", "button-link", "button-ref"];

/// The page being expanded and the site's files, for `button-ref`'s
/// document names.
struct Page<'a> {
    source: &'a str,
    files: &'a [String],
}

fn options<const N: usize>(list: [(&str, OptionSyntax); N]) -> BTreeMap<String, OptionSyntax> {
    list.into_iter().map(|(n, k)| (n.to_owned(), k)).collect()
}

/// The directives the plugin declares.
pub(crate) fn syntax() -> Syntax {
    use OptionSyntax::{Flag, Required, Value};
    let directives = vec![
        DirectiveSyntax {
            name: "tab-set".into(),
            options: options([("sync-group", Required), ("class", Value)]),
            content: ContentSyntax::Blocks,
            ..DirectiveSyntax::default()
        },
        DirectiveSyntax {
            name: "tab-item".into(),
            required_arguments: 1,
            final_argument_whitespace: true,
            argument: ArgumentSyntax::Text,
            options: options([
                ("selected", Flag),
                ("sync", Required),
                ("name", Required),
                ("class-label", Value),
                ("class-content", Value),
            ]),
            content: ContentSyntax::Blocks,
            ..DirectiveSyntax::default()
        },
        button("button-link", &[]),
        button("button-ref", &[("ref-type", OptionSyntax::Required)]),
    ];
    Syntax {
        directives,
        roles: Vec::new(),
    }
}

/// A button directive: one argument (its target), sphinx-design's
/// options, and its text as content.
fn button(name: &str, extra: &[(&str, OptionSyntax)]) -> DirectiveSyntax {
    use OptionSyntax::{Flag, Required, Value};
    let mut options = options([
        ("color", Required),
        ("outline", Flag),
        ("align", Required),
        ("expand", Flag),
        ("click-parent", Flag),
        ("tooltip", Required),
        ("shadow", Flag),
        ("class", Value),
    ]);
    options.extend(extra.iter().map(|(n, k)| ((*n).to_owned(), *k)));
    DirectiveSyntax {
        name: name.into(),
        required_arguments: 1,
        argument: ArgumentSyntax::Text,
        options,
        content: ContentSyntax::Blocks,
        ..DirectiveSyntax::default()
    }
}

/// Expands the plugin's directives in `nodes` and below them.
fn expand(page: &Page<'_>, nodes: Vec<Node>) -> Vec<Node> {
    nodes
        .into_iter()
        .map(|node| match node {
            Node::Custom(c) if c.kind == "button-link" || c.kind == "button-ref" => {
                button_node(page, c)
            }
            Node::Custom(c) if DIRECTIVES.contains(&c.kind.as_str()) => directive(page, c),
            mut node => {
                if let Some(children) = node.children_mut() {
                    let expanded = expand(page, std::mem::take(children));
                    *children = expanded;
                }
                node
            }
        })
        .collect()
}

/// A `button-link` or `button-ref` as a `custom:button` node holding a
/// link to its target.
fn button_node(page: &Page<'_>, c: Custom) -> Node {
    let argument = c
        .attrs
        .get("argument")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_owned();
    let options = c.attrs.get("options").and_then(Value::as_object);
    let option = |name: &str| options.and_then(|o| o.get(name));
    let text_option = |name: &str| {
        option(name)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|v| !v.is_empty())
    };
    // The content's paragraphs, as the link's text.
    let mut children: Vec<Node> = c
        .children
        .into_iter()
        .filter(|n| !matches!(n, Node::Custom(a) if a.kind == "directive_argument"))
        .flat_map(|n| match n {
            Node::Paragraph(p) => p.children,
            other => vec![other],
        })
        .collect();
    let (url, target) = if c.kind == "button-ref" {
        reference(page, &argument, text_option("ref-type").unwrap_or("any"))
    } else {
        if children.is_empty() {
            children.push(text(&argument));
        }
        (argument, None)
    };
    let mut attrs = BTreeMap::new();
    for name in ["color", "align"] {
        if let Some(v) = text_option(name) {
            attrs.insert(name.to_owned(), json!(v));
        }
    }
    for name in ["outline", "expand"] {
        if option(name).is_some() {
            attrs.insert(name.to_owned(), json!(true));
        }
    }
    Node::Custom(Custom {
        span: c.span,
        kind: "button".into(),
        attrs,
        children: vec![Node::Link(Link {
            span: c.span,
            url,
            title: text_option("tooltip").map(str::to_owned),
            target,
            children: expand(page, children),
        })],
    })
}

/// A `button-ref` target as a link's URL and target: a document (by its
/// name, with or without its extension), or a label.
fn reference(page: &Page<'_>, name: &str, ref_type: &str) -> (String, Option<LinkTarget>) {
    if ref_type != "ref"
        && let Some(url) = document_url(page, name)
    {
        return (url, None);
    }
    let label = LinkTarget {
        label: Some(name.to_owned()),
        ..LinkTarget::default()
    };
    (format!("#{name}"), Some(label))
}

/// `name` as a URL to a file of the site, adding the extension if it has
/// none (the first file with that stem, in sorted order). The host gives
/// the link its target.
fn document_url(page: &Page<'_>, name: &str) -> Option<String> {
    let UrlTarget::Internal {
        path: Some(path),
        fragment: None,
    } = resolve_url(name, page.source)
    else {
        return None;
    };
    if page.files.contains(&path) {
        return Some(name.to_owned());
    }
    let prefix = format!("{path}.");
    let mut matches: Vec<&String> = page
        .files
        .iter()
        .filter(|f| {
            f.strip_prefix(&prefix)
                .is_some_and(|ext| !ext.contains(['/', '.']))
        })
        .collect();
    matches.sort();
    let ext = matches.first()?.strip_prefix(&prefix)?;
    Some(format!("{name}.{ext}"))
}

/// The node a directive's `custom` node stands for: the same kind, with
/// only its content as children, and a tab item's label.
fn directive(page: &Page<'_>, c: Custom) -> Node {
    let label = c
        .attrs
        .get("argument")
        .and_then(Value::as_str)
        .map(|a| a.trim().to_owned());
    // The parsers put a parsed argument first, as a `directive_argument`.
    let children = c
        .children
        .into_iter()
        .filter(|n| !matches!(n, Node::Custom(a) if a.kind == "directive_argument"))
        .collect();
    let mut attrs = BTreeMap::new();
    if c.kind == "tab-item" {
        attrs.insert("label".into(), json!(label.unwrap_or_default()));
    }
    Node::Custom(Custom {
        span: c.span,
        kind: c.kind,
        attrs,
        children: expand(page, children),
    })
}

impl Transformer for Extras {
    /// The directives' nodes, as the host selects them (`custom:<kind>`).
    fn nodes(&self) -> Vec<String> {
        DIRECTIVES.iter().map(|d| format!("custom:{d}")).collect()
    }

    async fn apply(
        &self,
        params: RenderTransformEachParams,
        ctx: &Context,
    ) -> PluginResult<RenderTransformEachResult> {
        let mut document = params.document;
        let source = document.source.clone();
        let page = Page {
            source: &source,
            files: &ctx.site().files,
        };
        document.children = expand(&page, std::mem::take(&mut document.children));
        Ok(RenderTransformEachResult {
            document,
            ..RenderTransformEachResult::default()
        })
    }
}
