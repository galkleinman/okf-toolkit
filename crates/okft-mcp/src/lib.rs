//! An MCP server exposing an OKF bundle to AI agents.
//!
//! Concepts are published twice: as MCP resources under `okf://<concept-id>`,
//! and as tools. The duplication is deliberate. Resources are the right model
//! for read-only documents, but many clients never surface them to the model
//! without an explicit user action, so a resource-only server looks correct and
//! does nothing useful in practice.

use std::fmt::Write as _;
use std::sync::Arc;

use okft_core::bundle::{Bundle, Entry};
use okft_core::concept_id::ConceptId;
use okft_core::date::Date;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, ContentBlock, Implementation, ListResourcesResult, PaginatedRequestParams,
    ProtocolVersion, ReadResourceRequestParams, ReadResourceResponse, ReadResourceResult, Resource,
    ResourceContents, ServerCapabilities, ServerInfo,
};
use rmcp::service::RequestContext;
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, schemars, tool, tool_handler, tool_router,
};
use serde::Deserialize;

/// URI scheme used for concept resources.
pub const URI_SCHEME: &str = "okf://";

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchRequest {
    /// Case-insensitive text to look for in titles, descriptions, tags, ids, and bodies.
    pub query: String,
    /// Maximum number of concepts to return. Defaults to 10.
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ConceptRequest {
    /// A concept id, such as `architecture/okft-core`.
    pub id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListRequest {
    /// Only return concepts whose `type` matches, case-insensitively.
    #[serde(default)]
    pub concept_type: Option<String>,
}

/// A bundle served over MCP.
#[derive(Clone, Debug)]
pub struct OkfServer {
    bundle: Arc<Bundle>,
    /// The date staleness is reported against, fixed at construction so a long
    /// running server does not silently change its answers at midnight.
    today: Date,
    /// Read by the code `#[tool_handler]` generates, which the dead-code lint
    /// does not see.
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl OkfServer {
    pub fn new(bundle: Bundle, today: Date) -> Self {
        Self {
            bundle: Arc::new(bundle),
            today,
            tool_router: Self::tool_router(),
        }
    }

    pub fn bundle(&self) -> &Bundle {
        &self.bundle
    }

    #[tool(
        description = "Search the knowledge bundle for concepts matching a query. Returns \
                       matching concept ids with their titles and descriptions."
    )]
    fn search(&self, Parameters(request): Parameters<SearchRequest>) -> CallToolResult {
        // Agents ask in phrases ("binary naming"), which rarely appear
        // verbatim. Every term must match somewhere in the concept, but not
        // necessarily adjacently or in the same field.
        let terms: Vec<String> = request
            .query
            .split_whitespace()
            .map(str::to_lowercase)
            .collect();
        let limit = request.limit.unwrap_or(10).max(1) as usize;

        let matches: Vec<&Entry> = self
            .bundle
            .concepts()
            .filter(|entry| !terms.is_empty() && terms.iter().all(|t| Self::matches(entry, t)))
            .take(limit)
            .collect();

        if matches.is_empty() {
            return CallToolResult::success(vec![ContentBlock::text(format!(
                "No concepts match {:?}.",
                request.query
            ))]);
        }

        let mut out = format!(
            "{} concept(s) matching {:?}:\n\n",
            matches.len(),
            request.query
        );
        for entry in matches {
            out.push_str(&Self::summarize(entry));
        }
        CallToolResult::success(vec![ContentBlock::text(out)])
    }

    #[tool(
        description = "Read one concept in full, including its frontmatter and markdown body. \
                       Takes a concept id as returned by search or list."
    )]
    fn read(&self, Parameters(request): Parameters<ConceptRequest>) -> CallToolResult {
        match self.lookup(&request.id) {
            Some(entry) => CallToolResult::success(vec![ContentBlock::text(render(entry))]),
            None => self.not_found(&request.id),
        }
    }

    #[tool(
        description = "List the concepts in the bundle, optionally filtered by their OKF `type`."
    )]
    fn list(&self, Parameters(request): Parameters<ListRequest>) -> CallToolResult {
        let wanted = request.concept_type.as_ref().map(|t| t.to_lowercase());
        let mut out = String::new();
        let mut count = 0;

        for entry in self.bundle.concepts() {
            let concept_type = concept_type(entry);
            if let Some(wanted) = &wanted
                && !concept_type.to_lowercase().eq(wanted)
            {
                continue;
            }
            count += 1;
            out.push_str(&Self::summarize(entry));
        }

        if count == 0 {
            return CallToolResult::success(vec![ContentBlock::text(
                "No concepts match that filter.".to_owned(),
            )]);
        }
        CallToolResult::success(vec![ContentBlock::text(format!(
            "{count} concept(s):\n\n{out}"
        ))])
    }

    #[tool(
        description = "Show which concepts a given concept links to, and which link back to it. \
                       Use this to explore how a topic connects to the rest of the bundle."
    )]
    fn related(&self, Parameters(request): Parameters<ConceptRequest>) -> CallToolResult {
        let Some(entry) = self.lookup(&request.id) else {
            return self.not_found(&request.id);
        };

        let links = self.bundle.links_from(&entry.id);
        let backlinks = self.bundle.backlinks(&entry.id);

        let mut out = format!("{}\n\n", entry.id);
        out.push_str("Links to:\n");
        out.push_str(&render_ids(&links));
        out.push_str("\nLinked from:\n");
        out.push_str(&render_ids(&backlinks));
        CallToolResult::success(vec![ContentBlock::text(out)])
    }

    #[tool(
        description = "Report the trust tier, lifecycle status, and staleness of a concept, \
                       derived from its OKF provenance frontmatter. Use this before relying on \
                       a concept's content."
    )]
    fn trust(&self, Parameters(request): Parameters<ConceptRequest>) -> CallToolResult {
        let Some(entry) = self.lookup(&request.id) else {
            return self.not_found(&request.id);
        };

        let Some(frontmatter) = entry.document.frontmatter.parsed() else {
            return CallToolResult::success(vec![ContentBlock::text(format!(
                "{}: frontmatter could not be parsed, so no trust signals are available.",
                entry.id
            ))]);
        };

        let mut out = format!(
            "{}\n  trust tier: {}\n  status: {}\n",
            entry.id,
            frontmatter.trust_tier(),
            frontmatter.status()
        );
        if let Some(generated) = frontmatter.generated() {
            let _ = writeln!(
                out,
                "  generated: {} at {}",
                generated.by.unwrap_or("(unknown)"),
                generated.at.unwrap_or("(unknown)")
            );
        }
        for verification in frontmatter.verified() {
            let _ = writeln!(
                out,
                "  verified: {} at {}",
                verification.by.unwrap_or("(unknown)"),
                verification.at.unwrap_or("(unknown)")
            );
        }
        if let Some(stale_after) = frontmatter.stale_after() {
            let state = if frontmatter.is_stale_on(self.today) {
                "STALE"
            } else {
                "fresh"
            };
            let _ = writeln!(
                out,
                "  stale_after: {stale_after} ({state} as of {})",
                self.today
            );
        }
        for source in frontmatter.sources() {
            let _ = writeln!(
                out,
                "  source: {} ({})",
                source.title.unwrap_or("(untitled)"),
                source.resource.unwrap_or("(no resource)")
            );
        }
        CallToolResult::success(vec![ContentBlock::text(out)])
    }

    fn lookup(&self, id: &str) -> Option<&Entry> {
        let trimmed = id.strip_prefix(URI_SCHEME).unwrap_or(id);
        let concept_id = ConceptId::from_relative_path(std::path::Path::new(trimmed))?;
        self.bundle
            .get(&concept_id)
            .filter(|entry| entry.is_concept())
    }

    fn not_found(&self, id: &str) -> CallToolResult {
        let available: Vec<ConceptId> = self
            .bundle
            .concepts()
            .take(10)
            .map(|entry| entry.id.clone())
            .collect();
        CallToolResult::error(vec![ContentBlock::text(format!(
            "No concept `{id}` in this bundle. Known concepts include:\n{}",
            render_ids(&available)
        ))])
    }

    fn matches(entry: &Entry, needle: &str) -> bool {
        if entry.id.as_str().to_lowercase().contains(needle) {
            return true;
        }
        if entry.document.body.text.to_lowercase().contains(needle) {
            return true;
        }
        let Some(frontmatter) = entry.document.frontmatter.parsed() else {
            return false;
        };
        [
            frontmatter.title(),
            frontmatter.description(),
            frontmatter.concept_type(),
        ]
        .into_iter()
        .flatten()
        .any(|field| field.to_lowercase().contains(needle))
            || frontmatter
                .tags()
                .iter()
                .any(|tag| tag.to_lowercase().contains(needle))
    }

    fn summarize(entry: &Entry) -> String {
        let frontmatter = entry.document.frontmatter.parsed();
        let title = frontmatter
            .and_then(|f| f.title())
            .unwrap_or_else(|| entry.id.name());
        let description = frontmatter.and_then(|f| f.description()).unwrap_or("");
        format!(
            "- {} [{}] {title}\n  {description}\n",
            entry.id,
            concept_type(entry)
        )
    }
}

#[tool_handler]
impl ServerHandler for OkfServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_resources()
                .enable_tools()
                .build(),
        )
        // Named for the binary that serves it, so a client's server list is
        // readable; `from_build_env()` would report the rmcp crate instead.
        .with_server_info(
            Implementation::new("okf", env!("CARGO_PKG_VERSION"))
                .with_title("OKF knowledge bundle"),
        )
        .with_protocol_version(ProtocolVersion::V_2024_11_05)
        .with_instructions(
            "This server exposes an Open Knowledge Format (OKF) knowledge bundle. Use `search` \
             to find concepts by text, `list` to enumerate them (optionally by type), `read` to \
             fetch one in full, `related` to follow links between them, and `trust` to check how \
             much a concept should be relied on before quoting it."
                .to_owned(),
        )
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        let resources = self
            .bundle
            .concepts()
            .map(|entry| {
                let frontmatter = entry.document.frontmatter.parsed();
                let title = frontmatter
                    .and_then(|f| f.title())
                    .unwrap_or_else(|| entry.id.name())
                    .to_owned();
                Resource::new(format!("{URI_SCHEME}{}", entry.id), title)
            })
            .collect();
        Ok(ListResourcesResult {
            resources,
            ..Default::default()
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, McpError> {
        match self.lookup(&request.uri) {
            Some(entry) => Ok(ReadResourceResult::new(vec![ResourceContents::text(
                render(entry),
                request.uri.clone(),
            )])
            .into()),
            None => Err(McpError::resource_not_found(
                "no such concept",
                Some(serde_json::json!({ "uri": request.uri })),
            )),
        }
    }
}

fn concept_type(entry: &Entry) -> &str {
    entry
        .document
        .frontmatter
        .parsed()
        .and_then(|frontmatter| frontmatter.concept_type())
        .unwrap_or("untyped")
}

/// Renders a concept the way an agent should see it: the original document.
fn render(entry: &Entry) -> String {
    let mut out = format!("# {}\n\n", entry.id);
    if let Some(frontmatter) = entry.document.frontmatter.parsed() {
        out.push_str("type: ");
        out.push_str(frontmatter.concept_type().unwrap_or("untyped"));
        out.push('\n');
        if let Some(title) = frontmatter.title() {
            let _ = writeln!(out, "title: {title}");
        }
        if let Some(description) = frontmatter.description() {
            let _ = writeln!(out, "description: {description}");
        }
        if let Some(resource) = frontmatter.resource() {
            let _ = writeln!(out, "resource: {resource}");
        }
        let tags = frontmatter.tags();
        if !tags.is_empty() {
            let _ = writeln!(out, "tags: {}", tags.join(", "));
        }
        let _ = writeln!(out, "trust: {}", frontmatter.trust_tier());
    }
    out.push('\n');
    out.push_str(&entry.document.body.text);
    out
}

fn render_ids(ids: &[ConceptId]) -> String {
    if ids.is_empty() {
        return "  (none)\n".to_owned();
    }
    ids.iter().fold(String::new(), |mut out, id| {
        let _ = writeln!(out, "  - {id}");
        out
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundle() -> Bundle {
        Bundle::from_sources([
            ("index.md", "# Bundle\n\n* [orders](tables/orders.md)\n"),
            (
                "tables/orders.md",
                "---\ntype: BigQuery Table\ntitle: Customer Orders\n\
                 description: One row per completed order.\ntags: [sales, revenue]\n\
                 resource: https://console.cloud.google.com/bigquery?t=orders\n\
                 generated: { by: human:gal, at: 2026-01-01T00:00:00Z }\n\
                 verified: { by: human:gal, at: 2026-01-02T00:00:00Z }\n\
                 status: stable\nstale_after: 2026-12-31\n\
                 sources:\n  - id: s1\n    title: Schema doc\n    resource: https://example.com\n\
                 ---\n\nJoins [customers](/tables/customers.md) on `customer_id`.\n",
            ),
            (
                "tables/customers.md",
                "---\ntype: BigQuery Table\ntitle: Customers\ndescription: One row per customer.\n---\n\
                 \nNothing special.\n",
            ),
            ("tables/broken.md", "---\n[unclosed\n---\nbody\n"),
        ])
    }

    fn server() -> OkfServer {
        OkfServer::new(bundle(), Date::parse("2026-06-01").expect("valid"))
    }

    fn text_of(result: &CallToolResult) -> String {
        result
            .content
            .iter()
            .filter_map(|block| block.as_text().map(|text| text.text.clone()))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn call<T>(tool: impl Fn(Parameters<T>) -> CallToolResult, request: T) -> String {
        text_of(&tool(Parameters(request)))
    }

    #[test]
    fn exposes_the_bundle_it_was_built_with() {
        assert_eq!(server().bundle().concepts().count(), 3);
    }

    #[test]
    fn search_matches_titles_descriptions_tags_ids_and_bodies() {
        let server = server();
        for query in [
            "customer orders",
            "completed order",
            "revenue",
            "tables/orders",
            "join",
        ] {
            let found = call(
                |p| server.search(p),
                SearchRequest {
                    query: query.to_owned(),
                    limit: None,
                },
            );
            assert!(
                found.contains("tables/orders"),
                "query {query:?} found: {found}"
            );
        }
    }

    /// Agents search in phrases, which rarely appear verbatim, so every term
    /// must match somewhere in the concept rather than as one adjacent string.
    #[test]
    fn search_requires_every_term_but_not_adjacency() {
        let server = server();
        let found = call(
            |p| server.search(p),
            SearchRequest {
                query: "orders customer".to_owned(),
                limit: None,
            },
        );
        assert!(found.contains("tables/orders"), "got {found}");

        let missing = call(
            |p| server.search(p),
            SearchRequest {
                query: "orders kangaroo".to_owned(),
                limit: None,
            },
        );
        assert!(missing.contains("No concepts match"), "got {missing}");
    }

    #[test]
    fn an_empty_query_matches_nothing() {
        let server = server();
        let found = call(
            |p| server.search(p),
            SearchRequest {
                query: "   ".to_owned(),
                limit: None,
            },
        );
        assert!(found.contains("No concepts match"));
    }

    #[test]
    fn search_reports_when_nothing_matches() {
        let server = server();
        let found = call(
            |p| server.search(p),
            SearchRequest {
                query: "kangaroo".to_owned(),
                limit: None,
            },
        );
        assert!(found.contains("No concepts match"));
    }

    #[test]
    fn search_respects_the_limit() {
        let server = server();
        let found = call(
            |p| server.search(p),
            SearchRequest {
                query: "table".to_owned(),
                limit: Some(1),
            },
        );
        assert_eq!(found.matches("- tables/").count(), 1);

        // A zero limit would return nothing useful, so it is clamped to one.
        let clamped = call(
            |p| server.search(p),
            SearchRequest {
                query: "table".to_owned(),
                limit: Some(0),
            },
        );
        assert_eq!(clamped.matches("- tables/").count(), 1);
    }

    #[test]
    fn search_skips_concepts_whose_frontmatter_failed_to_parse() {
        let server = server();
        let found = call(
            |p| server.search(p),
            SearchRequest {
                query: "unclosed".to_owned(),
                limit: None,
            },
        );
        assert!(found.contains("No concepts match"), "got {found}");
    }

    #[test]
    fn read_returns_frontmatter_and_body() {
        let server = server();
        let document = call(
            |p| server.read(p),
            ConceptRequest {
                id: "tables/orders".to_owned(),
            },
        );
        assert!(document.contains("type: BigQuery Table"));
        assert!(document.contains("title: Customer Orders"));
        assert!(document.contains("tags: sales, revenue"));
        assert!(document.contains("trust: human-reviewed"));
        assert!(document.contains("Joins [customers]"));
    }

    #[test]
    fn read_includes_the_resource_uri_when_present() {
        let server = server();
        let document = call(
            |p| server.read(p),
            ConceptRequest {
                id: "tables/orders".to_owned(),
            },
        );
        assert!(document.contains("resource: https://console.cloud.google.com/bigquery?t=orders"));
    }

    /// A concept whose frontmatter is unparseable still has a readable body,
    /// and §11 does not let the server refuse to serve it.
    #[test]
    fn read_still_returns_the_body_when_frontmatter_is_unparseable() {
        let server = server();
        let document = call(
            |p| server.read(p),
            ConceptRequest {
                id: "tables/broken".to_owned(),
            },
        );
        assert!(document.contains("tables/broken"));
        assert!(document.contains("body"));
        assert!(!document.contains("type:"));
    }

    #[test]
    fn read_accepts_a_resource_uri_as_well_as_a_bare_id() {
        let server = server();
        let bare = call(
            |p| server.read(p),
            ConceptRequest {
                id: "tables/orders".to_owned(),
            },
        );
        let uri = call(
            |p| server.read(p),
            ConceptRequest {
                id: "okf://tables/orders".to_owned(),
            },
        );
        assert_eq!(bare, uri);
    }

    #[test]
    fn read_reports_unknown_concepts_with_suggestions() {
        let server = server();
        let result = server.read(Parameters(ConceptRequest {
            id: "nope".to_owned(),
        }));
        assert_eq!(result.is_error, Some(true));
        let text = text_of(&result);
        assert!(text.contains("No concept `nope`"));
        assert!(text.contains("tables/orders"));
    }

    #[test]
    fn reserved_files_are_not_readable_as_concepts() {
        let server = server();
        let result = server.read(Parameters(ConceptRequest {
            id: "index".to_owned(),
        }));
        assert_eq!(result.is_error, Some(true));
    }

    #[test]
    fn read_rejects_ids_that_escape_the_bundle() {
        let server = server();
        let result = server.read(Parameters(ConceptRequest {
            id: "../escape".to_owned(),
        }));
        assert_eq!(result.is_error, Some(true));
    }

    #[test]
    fn list_enumerates_and_filters_by_type() {
        let server = server();
        let all = call(|p| server.list(p), ListRequest { concept_type: None });
        assert!(all.contains("3 concept(s)"));
        assert!(
            all.contains("[untyped]"),
            "the unparseable concept should still be listed"
        );

        let filtered = call(
            |p| server.list(p),
            ListRequest {
                concept_type: Some("bigquery table".to_owned()),
            },
        );
        assert!(filtered.contains("2 concept(s)"));
        assert!(!filtered.contains("broken"));

        let none = call(
            |p| server.list(p),
            ListRequest {
                concept_type: Some("Nonexistent".to_owned()),
            },
        );
        assert!(none.contains("No concepts match"));
    }

    #[test]
    fn related_reports_links_in_both_directions() {
        let server = server();
        let orders = call(
            |p| server.related(p),
            ConceptRequest {
                id: "tables/orders".to_owned(),
            },
        );
        assert!(orders.contains("- tables/customers"));

        let customers = call(
            |p| server.related(p),
            ConceptRequest {
                id: "tables/customers".to_owned(),
            },
        );
        assert!(customers.contains("- tables/orders"));
    }

    #[test]
    fn related_shows_none_for_an_unconnected_concept() {
        let server = server();
        let broken = call(
            |p| server.related(p),
            ConceptRequest {
                id: "tables/broken".to_owned(),
            },
        );
        assert_eq!(broken.matches("(none)").count(), 2);
    }

    #[test]
    fn related_reports_unknown_concepts() {
        let server = server();
        let result = server.related(Parameters(ConceptRequest {
            id: "nope".to_owned(),
        }));
        assert_eq!(result.is_error, Some(true));
    }

    #[test]
    fn trust_reports_the_provenance_families() {
        let server = server();
        let report = call(
            |p| server.trust(p),
            ConceptRequest {
                id: "tables/orders".to_owned(),
            },
        );
        assert!(report.contains("trust tier: human-reviewed"));
        assert!(report.contains("status: stable"));
        assert!(report.contains("generated: human:gal at 2026-01-01T00:00:00Z"));
        assert!(report.contains("verified: human:gal at 2026-01-02T00:00:00Z"));
        assert!(report.contains("stale_after: 2026-12-31 (fresh as of 2026-06-01)"));
        assert!(report.contains("source: Schema doc (https://example.com)"));
    }

    #[test]
    fn trust_reports_staleness_against_the_fixed_date() {
        let server = OkfServer::new(bundle(), Date::parse("2027-01-01").expect("valid"));
        let report = call(
            |p| server.trust(p),
            ConceptRequest {
                id: "tables/orders".to_owned(),
            },
        );
        assert!(report.contains("(STALE as of 2027-01-01)"));
    }

    #[test]
    fn trust_on_a_concept_without_provenance_is_still_useful() {
        let server = server();
        let report = call(
            |p| server.trust(p),
            ConceptRequest {
                id: "tables/customers".to_owned(),
            },
        );
        assert!(report.contains("trust tier: unverified"));
        assert!(report.contains("status: stable"));
    }

    #[test]
    fn trust_explains_when_frontmatter_could_not_be_parsed() {
        let server = server();
        let report = call(
            |p| server.trust(p),
            ConceptRequest {
                id: "tables/broken".to_owned(),
            },
        );
        assert!(report.contains("could not be parsed"));
    }

    #[test]
    fn trust_reports_unknown_concepts() {
        let server = server();
        let result = server.trust(Parameters(ConceptRequest {
            id: "nope".to_owned(),
        }));
        assert_eq!(result.is_error, Some(true));
    }

    #[test]
    fn server_info_advertises_tools_and_resources() {
        let info = server().get_info();
        assert!(info.capabilities.tools.is_some());
        assert!(info.capabilities.resources.is_some());
        let instructions = info.instructions.expect("instructions");
        for tool in ["search", "list", "read", "related", "trust"] {
            assert!(
                instructions.contains(tool),
                "instructions should mention {tool}"
            );
        }
    }

    #[test]
    fn concept_type_falls_back_to_untyped() {
        let bundle = bundle();
        let broken = bundle
            .get(&ConceptId::from_relative_path(std::path::Path::new("tables/broken")).unwrap())
            .expect("entry");
        assert_eq!(concept_type(broken), "untyped");
    }

    #[test]
    fn rendering_ids_handles_the_empty_case() {
        assert_eq!(render_ids(&[]), "  (none)\n");
    }
}
