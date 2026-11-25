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
            site: SiteInfo::default(),
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
