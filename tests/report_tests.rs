use std::time::{Duration, SystemTime};

use rust_vault_map::analysis::analyze_graph;
use rust_vault_map::graph::{build_graph, GraphNote};
use rust_vault_map::parser::WikiLink;
use rust_vault_map::report::{render_report, render_report_colored, ReportInput};

#[test]
fn report_renders_required_sections_and_counts() {
    let graph = build_graph(vec![
        note("Index.md", "This mentions Rust.", vec![link("Missing")]),
        note("Rust.md", "", vec![]),
    ]);
    let analysis = analyze_graph(&graph, SystemTime::UNIX_EPOCH);

    let output = render_report(&ReportInput {
        vault_path: "C:/vault".to_string(),
        note_count: 2,
        folder_count: 0,
        wiki_link_count: 1,
        skipped_file_count: 0,
        stale_threshold_days: 90,
        graph: &graph,
        analysis: &analysis,
    });

    assert!(output.contains("rust-vault-map scan report"));
    assert!(output.contains("Scanned path: C:/vault"));
    assert!(output.contains("Markdown notes: 2"));
    assert!(output.contains("Folders: 0"));
    assert!(output.contains("Obsidian links: 1"));
    assert!(output.contains("Broken links"));
    assert!(output.contains("Index.md -> Missing"));
    assert!(output.contains("Ambiguous links"));
    assert!(output.contains("Orphan notes"));
    assert!(output.contains("Stale notes"));
    assert!(output.contains("Hub notes"));
    assert!(output.contains("Clusters"));
    assert!(output.contains("Suggested links"));
    assert!(output.contains("Next actions"));
}

#[test]
fn report_renders_ambiguous_links_with_candidates() {
    let graph = build_graph(vec![
        note("Index.md", "", vec![link("Topic")]),
        note("Areas/Topic.md", "", vec![]),
        note("Projects/Topic.md", "", vec![]),
    ]);
    let analysis = analyze_graph(&graph, SystemTime::UNIX_EPOCH);

    let output = render_report(&ReportInput {
        vault_path: "C:/vault".to_string(),
        note_count: 3,
        folder_count: 2,
        wiki_link_count: 1,
        skipped_file_count: 0,
        stale_threshold_days: 90,
        graph: &graph,
        analysis: &analysis,
    });

    assert!(output.contains("Ambiguous links: 1"));
    assert!(output.contains("Index.md -> Topic"));
    assert!(output.contains("candidates: Areas/Topic.md, Projects/Topic.md"));
}

#[test]
fn report_explains_empty_stale_notes_with_threshold_and_oldest_age() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(30 * 24 * 60 * 60);
    let graph = build_graph(vec![note_with_time(
        "Recent.md",
        "",
        vec![],
        now - Duration::from_secs(12 * 24 * 60 * 60),
    )]);
    let analysis = analyze_graph(&graph, now);

    let output = render_report(&ReportInput {
        vault_path: "C:/vault".to_string(),
        note_count: 1,
        folder_count: 0,
        wiki_link_count: 0,
        skipped_file_count: 0,
        stale_threshold_days: 90,
        graph: &graph,
        analysis: &analysis,
    });

    assert!(output.contains("Stale threshold: 90 days"));
    assert!(output.contains("Oldest scanned note age: 12 days"));
    assert!(output.contains("No stale notes found because no scanned note is older than 90 days."));
}

#[test]
fn report_summarizes_clusters_without_dumping_every_single_note() {
    let graph = build_graph(vec![
        note("Alpha.md", "", vec![link("Beta")]),
        note("Beta.md", "", vec![link("Gamma")]),
        note("Gamma.md", "", vec![]),
        note("Solo One.md", "", vec![]),
        note("Solo Two.md", "", vec![]),
    ]);
    let analysis = analyze_graph(&graph, SystemTime::UNIX_EPOCH);

    let output = render_report(&ReportInput {
        vault_path: "C:/vault".to_string(),
        note_count: 5,
        folder_count: 0,
        wiki_link_count: 2,
        skipped_file_count: 0,
        stale_threshold_days: 90,
        graph: &graph,
        analysis: &analysis,
    });

    assert!(output.contains("Clusters: 3 total"));
    assert!(output.contains("1 multi-note clusters"));
    assert!(output.contains("2 one-note clusters"));
    assert!(output.contains("3 notes; samples: Alpha.md, Beta.md, Gamma.md"));
    assert!(!output.contains("1 notes: Solo One.md"));
    assert!(!output.contains("1 notes: Solo Two.md"));
}

#[test]
fn report_groups_suggested_links_by_source_note() {
    let graph = build_graph(vec![
        note("Index.md", "This mentions Alpha and Beta.", vec![]),
        note("Alpha.md", "", vec![]),
        note("Beta.md", "", vec![]),
    ]);
    let analysis = analyze_graph(&graph, SystemTime::UNIX_EPOCH);

    let output = render_report(&ReportInput {
        vault_path: "C:/vault".to_string(),
        note_count: 3,
        folder_count: 0,
        wiki_link_count: 0,
        skipped_file_count: 0,
        stale_threshold_days: 90,
        graph: &graph,
        analysis: &analysis,
    });

    assert!(output.contains("Index.md"));
    assert!(output.contains("  - Alpha -> Alpha.md"));
    assert!(output.contains("  - Beta -> Beta.md"));
}

#[test]
fn colored_report_uses_ansi_styling_for_terminal_headings_and_findings() {
    let graph = build_graph(vec![
        note("Index.md", "This mentions Rust.", vec![link("Missing")]),
        note("Rust.md", "", vec![]),
    ]);
    let analysis = analyze_graph(&graph, SystemTime::UNIX_EPOCH);

    let output = render_report_colored(&ReportInput {
        vault_path: "C:/vault".to_string(),
        note_count: 2,
        folder_count: 0,
        wiki_link_count: 1,
        skipped_file_count: 0,
        stale_threshold_days: 90,
        graph: &graph,
        analysis: &analysis,
    });

    assert!(output.contains("\u{1b}["));
    assert!(output.contains("rust-vault-map scan report"));
    assert!(output.contains("Broken links"));
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
