//! Builds the graph JSON the vendored viewer script expects.
//!
//! The shape is Google's: `{ nodes, edges, bodies, types, palette }`, where a
//! node is `{ data: { id, label, … } }` in Cytoscape's element format.

use okft_core::bundle::{Bundle, Entry};
use okft_core::date::Date;
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};

const DEFAULT_NODE_COLOR: &str = "#94a3b8";

/// Colours for the types Google's reference viewer knows about.
const TYPE_PALETTE: [(&str, &str); 3] = [
    ("BigQuery Dataset", "#8b5cf6"),
    ("BigQuery Table", "#3b82f6"),
    ("Reference", "#10b981"),
];

#[derive(Debug, Serialize)]
pub struct Graph {
    pub nodes: Vec<Value>,
    pub edges: Vec<Value>,
    /// Concept bodies, rendered as markdown in the detail pane.
    pub bodies: BTreeMap<String, String>,
    pub types: Vec<String>,
    pub palette: BTreeMap<String, String>,
}

/// Builds the graph for a bundle.
///
/// Reserved files are excluded: §3.1 says `index.md` and `log.md` are not
/// concept documents, so they are not nodes. Google's generator drops only
/// `index.md`, which makes `log.md` appear as an untyped concept.
pub fn build(bundle: &Bundle, today: Date) -> Graph {
    let concepts: Vec<&Entry> = bundle.concepts().collect();

    let nodes = concepts.iter().map(|entry| node(entry, today)).collect();
    let bodies = concepts
        .iter()
        .map(|entry| (entry.id.to_string(), entry.document.body.text.clone()))
        .collect();
    let types: BTreeSet<String> = concepts
        .iter()
        .map(|entry| concept_type(entry).to_owned())
        .collect();

    let mut edges = Vec::new();
    for entry in &concepts {
        // `links_from` already deduplicates targets, and each concept is
        // visited once, so no separate edge-deduplication pass is needed.
        for target in bundle.links_from(&entry.id) {
            // Self-links and links to reserved files are not graph edges.
            if target == entry.id || bundle.get(&target).is_none_or(|e| !e.is_concept()) {
                continue;
            }
            edges.push(json!({
                "data": {
                    "id": format!("{}__{target}", entry.id),
                    "source": entry.id.to_string(),
                    "target": target.to_string(),
                }
            }));
        }
    }

    Graph {
        nodes,
        edges,
        bodies,
        types: types.into_iter().collect(),
        palette: TYPE_PALETTE
            .iter()
            .map(|(name, color)| ((*name).to_owned(), (*color).to_owned()))
            .collect(),
    }
}

fn concept_type(entry: &Entry) -> &str {
    entry
        .document
        .frontmatter
        .parsed()
        .and_then(okft_core::document::Frontmatter::concept_type)
        .unwrap_or("Unknown")
}

fn node(entry: &Entry, today: Date) -> Value {
    let frontmatter = entry.document.frontmatter.parsed();
    let concept_type = concept_type(entry);
    let body_length = entry.document.body.text.len();

    let (generated, verified, sources, status, stale_after, trust_tier, stale) = frontmatter
        .map_or_else(
            || {
                (
                    Value::Object(Map::new()),
                    json!([]),
                    json!([]),
                    "stable".to_owned(),
                    String::new(),
                    "unverified".to_owned(),
                    false,
                )
            },
            |f| {
                let generated = f.generated().map_or_else(
                    || Value::Object(Map::new()),
                    |g| json!({ "by": g.by.unwrap_or_default(), "at": g.at.unwrap_or_default() }),
                );
                let verified = Value::Array(
                    f.verified()
                        .iter()
                        .map(|v| {
                            json!({
                                "by": v.by.unwrap_or_default(),
                                "at": v.at.unwrap_or_default(),
                            })
                        })
                        .collect(),
                );
                let sources = Value::Array(
                    f.sources()
                        .iter()
                        .map(|s| {
                            json!({
                                "id": s.id.unwrap_or_default(),
                                "resource": s.resource.unwrap_or_default(),
                                "title": s.title.unwrap_or_default(),
                                "author": s.author.unwrap_or_default(),
                                "last_modified": s.last_modified.unwrap_or_default(),
                                "usage_count": s.usage_count,
                            })
                        })
                        .collect(),
                );
                (
                    generated,
                    verified,
                    sources,
                    f.status().as_str().to_owned(),
                    f.stale_after().map(|d| d.to_string()).unwrap_or_default(),
                    f.trust_tier().as_str().to_owned(),
                    f.is_stale_on(today),
                )
            },
        );

    json!({
        "data": {
            "id": entry.id.to_string(),
            "label": frontmatter
                .and_then(okft_core::document::Frontmatter::title)
                .unwrap_or_else(|| entry.id.as_str()),
            "type": concept_type,
            "description": frontmatter
                .and_then(okft_core::document::Frontmatter::description)
                .unwrap_or_default(),
            "resource": frontmatter
                .and_then(okft_core::document::Frontmatter::resource)
                .unwrap_or_default(),
            "tags": frontmatter.map(okft_core::document::Frontmatter::tags).unwrap_or_default(),
            "status": status,
            "generated": generated,
            "verified": verified,
            "stale_after": stale_after,
            "sources": sources,
            "trust_tier": trust_tier,
            "stale": stale,
            "color": TYPE_PALETTE
                .iter()
                .find(|(name, _)| *name == concept_type)
                .map_or(DEFAULT_NODE_COLOR, |(_, color)| *color),
            // Bigger documents get bigger nodes, capped so one long concept
            // does not dominate the layout.
            "size": 30 + (body_length / 200).min(60),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn today() -> Date {
        Date::parse("2026-06-01").expect("valid")
    }

    fn bundle() -> Bundle {
        Bundle::from_sources([
            ("index.md", "# Bundle\n\n* [orders](tables/orders.md)\n"),
            ("log.md", "# Log\n\n## 2026-01-01\n"),
            (
                "tables/orders.md",
                "---\ntype: BigQuery Table\ntitle: Orders\ndescription: One row per order.\n\
                 resource: https://example.com/orders\ntags: [sales]\nstatus: stable\n\
                 generated: { by: agent/v1, at: 2026-01-01T00:00:00Z }\n\
                 verified: { by: human:gal, at: 2026-01-02T00:00:00Z }\nstale_after: 2026-12-31\n\
                 sources:\n  - id: s1\n    resource: https://example.com\n    title: Schema\n    \
                 author: team:data\n    usage_count: 42\n    last_modified: 2026-01-01\n---\n\n\
                 Joins [customers](/tables/customers.md) and [itself](/tables/orders.md), \
                 plus the [log](/log.md).\n",
            ),
            (
                "tables/customers.md",
                "---\ntype: BigQuery Table\ntitle: Customers\n---\n",
            ),
            ("tables/untyped.md", "---\ntitle: No type\n---\nshort\n"),
        ])
    }

    fn node_data(graph: &Graph, id: &str) -> Value {
        graph
            .nodes
            .iter()
            .find(|n| n["data"]["id"] == id)
            .expect("the node exists")["data"]
            .clone()
    }

    #[test]
    fn reserved_files_are_not_nodes() {
        let graph = build(&bundle(), today());
        let ids: Vec<&str> = graph
            .nodes
            .iter()
            .filter_map(|n| n["data"]["id"].as_str())
            .collect();
        assert_eq!(ids.len(), 3);
        assert!(!ids.contains(&"index"));
        assert!(!ids.contains(&"log"));
    }

    #[test]
    fn nodes_carry_the_full_frontmatter_contract() {
        let data = node_data(&build(&bundle(), today()), "tables/orders");
        assert_eq!(data["label"], "Orders");
        assert_eq!(data["type"], "BigQuery Table");
        assert_eq!(data["description"], "One row per order.");
        assert_eq!(data["resource"], "https://example.com/orders");
        assert_eq!(data["tags"], json!(["sales"]));
        assert_eq!(data["status"], "stable");
        assert_eq!(data["generated"]["by"], "agent/v1");
        assert_eq!(data["verified"][0]["by"], "human:gal");
        assert_eq!(data["stale_after"], "2026-12-31");
        assert_eq!(data["trust_tier"], "human-reviewed");
        assert_eq!(data["stale"], false);
        assert_eq!(data["color"], "#3b82f6");
        assert_eq!(data["sources"][0]["usage_count"], 42);
        assert_eq!(data["sources"][0]["author"], "team:data");
    }

    #[test]
    fn staleness_follows_the_supplied_date() {
        let graph = build(&bundle(), Date::parse("2027-01-01").expect("valid"));
        assert_eq!(node_data(&graph, "tables/orders")["stale"], true);
    }

    #[test]
    fn untyped_concepts_fall_back_to_defaults() {
        let data = node_data(&build(&bundle(), today()), "tables/untyped");
        assert_eq!(data["type"], "Unknown");
        assert_eq!(data["label"], "No type");
        assert_eq!(data["color"], DEFAULT_NODE_COLOR);
        assert_eq!(data["trust_tier"], "unverified");
        assert_eq!(data["generated"], json!({}));
        assert_eq!(data["verified"], json!([]));
        assert_eq!(data["sources"], json!([]));
        assert_eq!(data["stale_after"], "");
    }

    #[test]
    fn a_concept_without_a_title_is_labelled_by_id() {
        let bundle = Bundle::from_sources([("a/b.md", "---\ntype: X\n---\n")]);
        assert_eq!(node_data(&build(&bundle, today()), "a/b")["label"], "a/b");
    }

    #[test]
    fn node_size_grows_with_the_body_and_is_capped() {
        let small = Bundle::from_sources([("a.md", "---\ntype: X\n---\ntiny\n")]);
        assert_eq!(node_data(&build(&small, today()), "a")["size"], 30);

        let body = "x".repeat(200 * 100);
        let source = format!("---\ntype: X\n---\n{body}");
        let large = Bundle::from_sources([("a.md", source.as_str())]);
        assert_eq!(node_data(&build(&large, today()), "a")["size"], 90);
    }

    #[test]
    fn edges_skip_self_links_and_reserved_targets() {
        let graph = build(&bundle(), today());
        let pairs: Vec<(String, String)> = graph
            .edges
            .iter()
            .map(|e| {
                (
                    e["data"]["source"].as_str().unwrap_or_default().to_owned(),
                    e["data"]["target"].as_str().unwrap_or_default().to_owned(),
                )
            })
            .collect();
        assert_eq!(
            pairs,
            [("tables/orders".to_owned(), "tables/customers".to_owned())]
        );
        assert_eq!(
            graph.edges[0]["data"]["id"],
            "tables/orders__tables/customers"
        );
    }

    #[test]
    fn edges_to_missing_concepts_are_dropped() {
        let bundle = Bundle::from_sources([("a.md", "---\ntype: X\n---\n[gone](/nowhere.md)\n")]);
        assert!(build(&bundle, today()).edges.is_empty());
    }

    #[test]
    fn repeated_links_produce_one_edge() {
        let bundle = Bundle::from_sources([
            (
                "a.md",
                "---\ntype: X\n---\n[b](/b.md) and [b again](/b.md)\n",
            ),
            ("b.md", "---\ntype: X\n---\n"),
        ]);
        assert_eq!(build(&bundle, today()).edges.len(), 1);
    }

    #[test]
    fn bodies_types_and_palette_are_populated() {
        let graph = build(&bundle(), today());
        assert!(graph.bodies["tables/orders"].contains("Joins [customers]"));
        assert_eq!(graph.types, ["BigQuery Table", "Unknown"]);
        assert_eq!(graph.palette["BigQuery Table"], "#3b82f6");
        assert_eq!(graph.palette.len(), 3);
    }

    #[test]
    fn an_empty_bundle_produces_an_empty_graph() {
        let graph = build(&Bundle::default(), today());
        assert!(graph.nodes.is_empty());
        assert!(graph.edges.is_empty());
        assert!(graph.types.is_empty());
        assert!(graph.bodies.is_empty());
    }

    #[test]
    fn concepts_with_unparseable_frontmatter_still_appear() {
        let bundle = Bundle::from_sources([("a.md", "---\n[unclosed\n---\nbody text\n")]);
        let data = node_data(&build(&bundle, today()), "a");
        assert_eq!(data["type"], "Unknown");
        assert_eq!(data["label"], "a");
        assert_eq!(data["status"], "stable");
    }

    #[test]
    fn the_graph_serializes_to_the_expected_top_level_shape() {
        let value = serde_json::to_value(build(&bundle(), today())).expect("serializes");
        for key in ["nodes", "edges", "bodies", "types", "palette"] {
            assert!(value.get(key).is_some(), "missing {key}");
        }
    }
}
