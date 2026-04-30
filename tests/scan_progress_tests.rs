use std::fs;

use rust_vault_map::scan::{scan_vault_with_progress, PhaseState, ScanPhase};
use tempfile::TempDir;

#[test]
fn scan_vault_with_progress_records_ordered_phase_statuses() {
    let fixture = fixture_vault();
    let mut events = Vec::new();

    let outcome = scan_vault_with_progress(fixture.path(), |event| events.push(event))
        .expect("scan with progress");

    let phases = events.iter().map(|event| event.phase).collect::<Vec<_>>();

    assert_eq!(
        phases,
        vec![
            ScanPhase::DiscoverNotes,
            ScanPhase::ParseLinks,
            ScanPhase::BuildGraph,
            ScanPhase::AnalyzeHealth,
            ScanPhase::PrepareReport,
        ]
    );
    assert!(events
        .iter()
        .all(|event| event.state == PhaseState::Complete));
    assert_eq!(outcome.note_count, 2);
    assert_eq!(outcome.folder_count, 0);
    assert_eq!(outcome.wiki_link_count, 1);
    assert_eq!(outcome.graph.broken_links.len(), 0);
}

#[test]
fn scan_vault_with_progress_marks_attention_phases_when_findings_exist() {
    let fixture = fixture_with_broken_link();
    let mut events = Vec::new();

    let outcome = scan_vault_with_progress(fixture.path(), |event| events.push(event))
        .expect("scan with progress");

    let graph_event = events
        .iter()
        .find(|event| event.phase == ScanPhase::BuildGraph)
        .expect("graph event");
    let analysis_event = events
        .iter()
        .find(|event| event.phase == ScanPhase::AnalyzeHealth)
        .expect("analysis event");

    assert_eq!(graph_event.state, PhaseState::Attention);
    assert_eq!(analysis_event.state, PhaseState::Attention);
    assert!(graph_event.detail.contains("1 broken"));
    assert!(analysis_event.detail.contains("1 orphan"));
    assert_eq!(outcome.graph.broken_links.len(), 1);
    assert_eq!(outcome.analysis.orphan_notes.len(), 1);
}

fn fixture_vault() -> TempDir {
    let temp = TempDir::new().expect("fixture tempdir");
    fs::write(temp.path().join("Index.md"), "# Index\n[[Rust]]\n").expect("write index");
    fs::write(temp.path().join("Rust.md"), "# Rust\n").expect("write rust");
    temp
}

fn fixture_with_broken_link() -> TempDir {
    let temp = TempDir::new().expect("fixture tempdir");
    fs::write(
        temp.path().join("Index.md"),
        "# Index\n[[Rust]] [[Missing]]\n",
    )
    .expect("write index");
    fs::write(temp.path().join("Rust.md"), "# Rust\n").expect("write rust");
    fs::write(temp.path().join("Orphan.md"), "# Orphan\n").expect("write orphan");
    temp
}
