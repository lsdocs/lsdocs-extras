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
//!
//! It has no settings.

use std::collections::BTreeMap;

use lsdocs_plugin_sdk::doctree::{Custom, Node};
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
        .cacheable()
}

/// The transformer.
#[derive(Debug, Clone, Copy, Default)]
pub struct Extras;

/// The directives the plugin handles.
const DIRECTIVES: &[&str] = &["tab-set", "tab-item"];

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
    ];
    Syntax {
        directives,
        roles: Vec::new(),
    }
}

/// Expands the plugin's directives in `nodes` and below them.
fn expand(nodes: Vec<Node>) -> Vec<Node> {
    nodes
        .into_iter()
        .map(|node| match node {
            Node::Custom(c) if DIRECTIVES.contains(&c.kind.as_str()) => directive(c),
            mut node => {
                if let Some(children) = node.children_mut() {
                    let expanded = expand(std::mem::take(children));
                    *children = expanded;
                }
                node
            }
        })
        .collect()
}

/// The node a directive's `custom` node stands for: the same kind, with
/// only its content as children, and a tab item's label.
fn directive(c: Custom) -> Node {
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
        children: expand(children),
    })
}

impl Transformer for Extras {
    fn nodes(&self) -> Vec<String> {
        vec!["custom".into()]
    }

    async fn apply(
        &self,
        params: RenderTransformEachParams,
        _ctx: &Context,
    ) -> PluginResult<RenderTransformEachResult> {
        let mut document = params.document;
        document.children = expand(std::mem::take(&mut document.children));
        Ok(RenderTransformEachResult {
            document,
            ..RenderTransformEachResult::default()
        })
    }
}
