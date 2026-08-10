//! Drives the server through a real MCP client over an in-memory transport.
//!
//! Using `tokio::io::duplex` rather than a spawned subprocess keeps every
//! handler and error branch reachable from tests, and makes the suite fast
//! enough to run on every commit.

use okft_core::bundle::Bundle;
use okft_core::date::Date;
use okft_mcp::OkfServer;
use rmcp::ServiceExt;
use rmcp::model::{CallToolRequestParams, ReadResourceRequestParams};
use rmcp::service::RunningService;

fn bundle() -> Bundle {
    Bundle::from_sources([
        ("index.md", "# Bundle\n\n* [orders](tables/orders.md)\n"),
        (
            "tables/orders.md",
            "---\ntype: BigQuery Table\ntitle: Customer Orders\n\
             description: One row per completed order.\ntags: [sales]\n\
             verified: { by: human:gal, at: 2026-01-02T00:00:00Z }\n---\n\n\
             Joins [customers](/tables/customers.md).\n",
        ),
        (
            "tables/customers.md",
            "---\ntype: BigQuery Table\ntitle: Customers\ndescription: One row per customer.\n---\n",
        ),
        // No `title`, so resource naming must fall back to the concept id.
        ("tables/untitled.md", "---\ntype: BigQuery Table\n---\n"),
    ])
}

/// Connects a client to a server over an in-memory duplex pair.
async fn connect() -> RunningService<rmcp::RoleClient, ()> {
    let (client_io, server_io) = tokio::io::duplex(16 * 1024);
    let server = OkfServer::new(bundle(), Date::parse("2026-06-01").expect("valid"));

    tokio::spawn(async move {
        if let Ok(running) = server.serve(server_io).await {
            let _ = running.waiting().await;
        }
    });

    ().serve(client_io).await.expect("client connects")
}

fn text_of(result: &rmcp::model::CallToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|block| block.as_text().map(|text| text.text.clone()))
        .collect::<Vec<_>>()
        .join("\n")
}

async fn call(
    client: &RunningService<rmcp::RoleClient, ()>,
    name: &'static str,
    arguments: serde_json::Value,
) -> String {
    let mut params = CallToolRequestParams::new(name);
    if let Some(object) = arguments.as_object() {
        params = params.with_arguments(object.clone());
    }
    let result = client
        .call_tool(params)
        .await
        .unwrap_or_else(|error| panic!("calling {name}: {error}"));
    text_of(&result)
}

#[tokio::test]
async fn the_server_advertises_every_tool() {
    let client = connect().await;

    let tools = client.list_tools(None).await.expect("lists tools");
    let mut names: Vec<&str> = tools.tools.iter().map(|tool| tool.name.as_ref()).collect();
    names.sort_unstable();
    assert_eq!(names, ["list", "read", "related", "search", "trust"]);

    // Every tool needs a description, or a model cannot tell them apart.
    for tool in &tools.tools {
        assert!(
            tool.description.as_ref().is_some_and(|d| !d.is_empty()),
            "{} has no description",
            tool.name
        );
    }

    client.cancel().await.expect("shuts down");
}

#[tokio::test]
async fn concepts_are_listed_and_readable_as_resources() {
    let client = connect().await;

    let resources = client.list_resources(None).await.expect("lists resources");
    let mut uris: Vec<&str> = resources.resources.iter().map(|r| r.uri.as_ref()).collect();
    uris.sort_unstable();
    assert_eq!(
        uris,
        [
            "okf://tables/customers",
            "okf://tables/orders",
            "okf://tables/untitled"
        ]
    );

    // §4.1 lets consumers derive a name from the filename when `title` is absent.
    let untitled = resources
        .resources
        .iter()
        .find(|r| r.uri == "okf://tables/untitled")
        .expect("untitled resource");
    assert_eq!(untitled.name, "untitled");

    let contents = client
        .read_resource(ReadResourceRequestParams::new("okf://tables/orders"))
        .await
        .expect("reads resource");
    let text = match &contents.contents[0] {
        rmcp::model::ResourceContents::TextResourceContents { text, .. } => text.clone(),
        other => panic!("expected text contents, got {other:?}"),
    };
    assert!(text.contains("Customer Orders"));
    assert!(text.contains("Joins [customers]"));

    client.cancel().await.expect("shuts down");
}

#[tokio::test]
async fn reading_an_unknown_resource_is_an_error() {
    let client = connect().await;
    let result = client
        .read_resource(ReadResourceRequestParams::new("okf://tables/nope"))
        .await;
    assert!(result.is_err(), "expected an error for an unknown resource");
    client.cancel().await.expect("shuts down");
}

#[tokio::test]
async fn the_tools_answer_questions_about_the_bundle() {
    let client = connect().await;

    let found = call(
        &client,
        "search",
        serde_json::json!({ "query": "completed order" }),
    )
    .await;
    assert!(found.contains("tables/orders"), "{found}");

    let listed = call(&client, "list", serde_json::json!({})).await;
    assert!(listed.contains("3 concept(s)"), "{listed}");

    let document = call(
        &client,
        "read",
        serde_json::json!({ "id": "tables/orders" }),
    )
    .await;
    assert!(document.contains("title: Customer Orders"), "{document}");

    let related = call(
        &client,
        "related",
        serde_json::json!({ "id": "tables/orders" }),
    )
    .await;
    assert!(related.contains("tables/customers"), "{related}");

    let trust = call(
        &client,
        "trust",
        serde_json::json!({ "id": "tables/orders" }),
    )
    .await;
    assert!(trust.contains("human-reviewed"), "{trust}");

    client.cancel().await.expect("shuts down");
}

#[tokio::test]
async fn filtering_by_type_reaches_the_server() {
    let client = connect().await;
    let listed = call(
        &client,
        "list",
        serde_json::json!({ "concept_type": "BigQuery Table" }),
    )
    .await;
    assert!(listed.contains("3 concept(s)"), "{listed}");
    client.cancel().await.expect("shuts down");
}

#[tokio::test]
async fn server_instructions_reach_the_client() {
    let client = connect().await;
    let info = client.peer_info().expect("server info");
    let instructions = info.instructions.as_ref().expect("instructions");
    assert!(instructions.contains("Open Knowledge Format"));
    client.cancel().await.expect("shuts down");
}
