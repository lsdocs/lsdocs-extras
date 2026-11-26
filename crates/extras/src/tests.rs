#![allow(clippy::unwrap_used)]

use lsdocs_plugin_sdk::doctree::Document;
use lsdocs_plugin_sdk::protocol::{InitializeParams, SiteInfo};
use lsdocs_plugin_sdk::testing::run_transform;
use serde_json::Value;

use super::{plugin, syntax};

/// Parses `files` (by extension, with the plugin's syntax), runs the plugin
/// over them and returns their trees without spans.
async fn build(files: &[(&str, &str)]) -> Vec<Value> {
    let syntax = syntax();
    let mut documents: Vec<Document> = Vec::new();
    for (source, text) in files {
        let (doc, diagnostics) = if source.ends_with(".rst") {
            lsdocs_parser_rst::parse(source, text, None, &syntax)
        } else {
            lsdocs_parser_markdown::myst::parse_with_syntax(source, text, &syntax)
        };
        assert!(diagnostics.is_empty(), "{source}: {diagnostics:?}");
        documents.push(doc);
    }
    let setup = lsdocs_plugin_sdk::Setup::with_settings(
        InitializeParams {
            site: SiteInfo {
                files: files
                    .iter()
                    .map(|(source, _)| (*source).to_owned())
                    .collect(),
                ..SiteInfo::default()
            },
            ..InitializeParams::default()
        },
        Value::Null,
    );
    run_transform(&plugin(), &setup, documents)
        .await
        .unwrap()
        .into_iter()
        .map(|(doc, diagnostics)| {
            assert!(diagnostics.is_empty(), "{diagnostics:?}");
            without_spans(serde_json::to_value(&doc.children).unwrap())
        })
        .collect()
}

fn without_spans(mut value: Value) -> Value {
    match &mut value {
        Value::Object(map) => {
            map.remove("span");
            for v in map.values_mut() {
                *v = without_spans(v.take());
            }
        }
        Value::Array(items) => {
            for v in items {
                *v = without_spans(v.take());
            }
        }
        _ => {}
    }
    value
}

#[tokio::test]
async fn tab_sets_and_items() {
    let out = build(&[
        (
            "a.rst",
            ".. tab-set::\n\n   .. tab-item:: Rust\n      :selected:\n\n      First *tab*.\n\n   .. tab-item:: Python 3\n\n      Second tab.\n",
        ),
        (
            "a.md",
            "````{tab-set}\n```{tab-item} Rust\n:selected:\nFirst *tab*.\n```\n\n```{tab-item} Python 3\nSecond tab.\n```\n````\n",
        ),
    ])
    .await;
    assert_eq!(out[0], out[1], "reStructuredText and Markdown agree");
    let t = &out[0];
    assert_eq!(t[0]["type"], "custom");
    assert_eq!(t[0]["kind"], "tab-set");
    assert!(t[0].get("attrs").is_none(), "a tab set has no attributes");
    let items = t[0]["children"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["kind"], "tab-item");
    assert_eq!(items[0]["attrs"], serde_json::json!({"label": "Rust"}));
    assert_eq!(items[0]["children"][0]["children"][1]["type"], "emphasis");
    assert_eq!(items[1]["attrs"]["label"], "Python 3");
    assert_eq!(
        items[1]["children"][0]["children"][0]["value"],
        "Second tab."
    );
}

#[tokio::test]
async fn tab_sets_nest_in_other_blocks() {
    let out = build(&[(
        "a.md",
        "> ````{tab-set}\n> ```{tab-item} One\n> Text.\n> ```\n> ````\n",
    )])
    .await;
    let set = &out[0][0]["children"][0];
    assert_eq!(set["kind"], "tab-set");
    assert_eq!(set["children"][0]["attrs"]["label"], "One");
}

/// `button-link`: a `custom:button` around a link to the URL, with the
/// options as attributes and the tooltip as the link's title; the same
/// in both parsers.
#[tokio::test]
async fn button_links() {
    let out = build(&[
        (
            "a.rst",
            ".. button-link:: https://example.com\n   :color: primary\n   :outline:\n   :align: center\n   :tooltip: Go there\n   :shadow:\n\n   Get *started*\n",
        ),
        (
            "a.md",
            "```{button-link} https://example.com\n:color: primary\n:outline:\n:align: center\n:tooltip: Go there\n:shadow:\n\nGet *started*\n```\n",
        ),
    ])
    .await;
    assert_eq!(out[0], out[1], "reStructuredText and Markdown agree");
    let button = &out[0][0];
    assert_eq!(button["kind"], "button");
    assert_eq!(
        button["attrs"],
        serde_json::json!({"align": "center", "color": "primary", "outline": true})
    );
    let link = &button["children"][0];
    assert_eq!(link["type"], "link");
    assert_eq!(link["url"], "https://example.com");
    assert_eq!(link["title"], "Go there");
    assert_eq!(link["children"][0]["value"], "Get ");
    assert_eq!(link["children"][1]["type"], "emphasis");
}

/// Without content, a `button-link`'s text is its URL.
#[tokio::test]
async fn button_links_without_text_show_the_url() {
    let out = build(&[("a.md", "```{button-link} guide.md\n:expand:\n```\n")]).await;
    let button = &out[0][0];
    assert_eq!(button["attrs"], serde_json::json!({"expand": true}));
    assert_eq!(button["children"][0]["url"], "guide.md");
    assert_eq!(button["children"][0]["children"][0]["value"], "guide.md");
}

/// `button-ref`: a document name gets its extension (the host then gives
/// the link its target); anything else, or `ref-type: ref`, is a label.
/// Without content the link has no text, for the links transformer to
/// fill in.
#[tokio::test]
async fn button_refs_to_documents_and_labels() {
    let out = build(&[
        (
            "guide/a.md",
            "```{button-ref} b\n```\n\n```{button-ref} /index\nHome\n```\n\n\
             ```{button-ref} intro\n```\n\n```{button-ref} b\n:ref-type: ref\n```\n",
        ),
        ("guide/b.rst", "B\n=\n"),
        ("index.md", "# Home\n"),
    ])
    .await;
    let links: Vec<&Value> = out[0]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| &b["children"][0])
        .collect();
    assert_eq!(links[0]["url"], "b.rst");
    assert!(links[0].get("target").is_none(), "{}", links[0]);
    assert_eq!(links[0]["children"], serde_json::json!([]));
    assert_eq!(links[1]["url"], "/index.md");
    assert_eq!(links[1]["children"][0]["value"], "Home");
    for (link, label) in [(links[2], "intro"), (links[3], "b")] {
        assert_eq!(link["url"], format!("#{label}"));
        assert_eq!(link["target"], serde_json::json!({"label": label}));
    }
}

/// The host only sends a transformer documents holding a node kind it
/// selects (`Node::kind`, `custom:<kind>` for custom nodes): every
/// directive's parsed node must be one.
#[test]
fn the_plugin_selects_its_directives_nodes() {
    use lsdocs_plugin_sdk::Transformer;
    let selected = super::Extras.nodes();
    let syntax = syntax();
    for (source, text) in [
        (
            "a.md",
            "```{button-link} x
```

````{tab-set}
```{tab-item} A
A
```
````

```{button-ref} x
```
",
        ),
        (
            "a.rst",
            ".. button-link:: x

.. tab-set::

   .. tab-item:: A

      A

.. button-ref:: x
",
        ),
    ] {
        let (doc, _) = if source.ends_with(".rst") {
            lsdocs_parser_rst::parse(source, text, None, &syntax)
        } else {
            lsdocs_parser_markdown::myst::parse_with_syntax(source, text, &syntax)
        };
        let kinds: Vec<String> = doc
            .descendants()
            .filter(|n| matches!(n, lsdocs_plugin_sdk::doctree::Node::Custom(_)))
            .map(|n| n.kind().into_owned())
            .collect();
        assert_eq!(kinds.len(), 4, "{source}: {kinds:?}");
        for kind in kinds {
            assert!(
                selected.contains(&kind),
                "{source}: {kind} not in {selected:?}"
            );
        }
    }
}
