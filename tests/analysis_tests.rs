use std::time::{Duration, SystemTime};

use rust_vault_map::analysis::{analyze_graph, STALE_DAYS};
use rust_vault_map::graph::{build_graph, GraphNote};
use rust_vault_map::parser::WikiLink;

#[test]
fn analysis_finds_orphans_and_stale_notes_deterministically() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(200 * 24 * 60 * 60);
    let old = now - Duration::from_secs((STALE_DAYS + 1) * 24 * 60 * 60);
    let recent = now - Duration::from_secs(2 * 24 * 60 * 60);
    let graph = build_graph(vec![
        note_with_time("Index.md", "", vec![link("Rust")], recent),
        note_with_time("Rust.md", "", vec![], recent),
        note_with_time("Zeta Orphan.md", "", vec![], old),
        note_with_time("Alpha Orphan.md", "", vec![], old),
    ]);

    let analysis = analyze_graph(&graph, now);

    assert_eq!(
        analysis.orphan_notes,
        vec!["Alpha Orphan.md", "Zeta Orphan.md"]
    );
    assert_eq!(
        analysis
            .stale_notes
            .iter()
            .map(|note| note.path.as_str())
            .collect::<Vec<_>>(),
        vec!["Alpha Orphan.md", "Zeta Orphan.md"]
    );
}

#[test]
fn analysis_ranks_hubs_by_degree_then_path() {
    let now = SystemTime::UNIX_EPOCH;
    let graph = build_graph(vec![
        note(
            "Index.md",
            "",
            vec![link("Rust"), link("Projects/Vault Map")],
        ),
        note("Rust.md", "", vec![link("Index")]),
        note("Projects/Vault Map.md", "", vec![link("Index")]),
    ]);

    let analysis = analyze_graph(&graph, now);

    assert_eq!(analysis.hub_notes[0].path, "Index.md");
    assert_eq!(analysis.hub_notes[0].inbound, 2);
    assert_eq!(analysis.hub_notes[0].outbound, 2);
}

#[test]
fn analysis_reports_connected_components_as_clusters() {
    let now = SystemTime::UNIX_EPOCH;
    let graph = build_graph(vec![
        note("Index.md", "", vec![link("Rust")]),
        note("Rust.md", "", vec![]),
        note("Clusters/Alpha.md", "", vec![link("Clusters/Beta")]),
        note("Clusters/Beta.md", "", vec![]),
        note("Solo.md", "", vec![]),
    ]);

    let analysis = analyze_graph(&graph, now);

    assert_eq!(
        analysis
            .clusters
            .iter()
            .map(|cluster| cluster.notes.clone())
            .collect::<Vec<_>>(),
        vec![
            vec!["Clusters/Alpha.md", "Clusters/Beta.md"],
            vec!["Index.md", "Rust.md"],
            vec!["Solo.md"],
        ]
    );
}

#[test]
fn analysis_suggests_unlinked_title_mentions_without_self_links() {
    let now = SystemTime::UNIX_EPOCH;
    let graph = build_graph(vec![
        note(
            "Index.md",
            "This mentions Rust and Vault Map.",
            vec![link("Rust")],
        ),
        note("Rust.md", "Rust mentions itself.", vec![]),
        note("Projects/Vault Map.md", "", vec![]),
    ]);

    let analysis = analyze_graph(&graph, now);

    assert_eq!(analysis.suggested_links.len(), 1);
    assert_eq!(analysis.suggested_links[0].source_path, "Index.md");
    assert_eq!(
        analysis.suggested_links[0].target_path,
        "Projects/Vault Map.md"
    );
}

#[test]
fn analysis_filters_generic_suggestion_titles_but_keeps_specific_mentions() {
    let now = SystemTime::UNIX_EPOCH;
    let graph = build_graph(vec![
        note(
            "Index.md",
            "This mentions spec, context, Work, and Vault Map.",
            vec![],
        ),
        note("codex/spec.md", "", vec![]),
        note("codex/context.md", "", vec![]),
        note("Career/Work.md", "", vec![]),
        note("Projects/Vault Map.md", "", vec![]),
    ]);

    let analysis = analyze_graph(&graph, now);
    let targets: Vec<_> = analysis
        .suggested_links
        .iter()
        .map(|suggestion| suggestion.target_path.as_str())
        .collect();

    assert_eq!(targets, vec!["Projects/Vault Map.md"]);
}

fn note(path: &str, body: &str, links: Vec<WikiLink>) -> GraphNote {
    note_with_time(path, body, links, SystemTime::UNIX_EPOCH)
}

fn note_with_time(path: &str, body: &str, links: Vec<WikiLink>, modified: SystemTime) -> GraphNote {
    GraphNote {
        path: path.to_string(),
        title: path
            .trim_end_matches(".md")
            .rsplit('/')
            .next()
            .unwrap()
            .to_string(),
        body: body.to_string(),
        links,
        modified: Some(modified),
    }
}

fn link(target: &str) -> WikiLink {
    WikiLink {
        target: target.to_string(),
        alias: None,
        raw: format!("[[{target}]]"),
    }
}
