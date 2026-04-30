use std::time::SystemTime;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use rust_vault_map::analysis::{analyze_graph, Cluster, HubNote, StaleNote, SuggestedLink};
use rust_vault_map::graph::{build_graph, AmbiguousLink, BrokenLink, GraphNote};
use rust_vault_map::parser::WikiLink;
use rust_vault_map::scan::{PhaseState, ScanOutcome, ScanPhase, ScanProgress};
use rust_vault_map::tui::{
    resolve_editor_command, DashboardAction, DashboardApp, DashboardFocus, DashboardView, EditorOs,
};

#[test]
fn dashboard_starts_on_overview_and_navigates_sections() {
    let mut app = DashboardApp::new(outcome());

    assert_eq!(app.active_view(), DashboardView::Overview);
    assert_eq!(app.focus(), DashboardFocus::Sidebar);

    app.handle_key(KeyCode::Down, KeyModifiers::NONE);
    assert_eq!(app.active_view(), DashboardView::BrokenLinks);

    app.handle_key(KeyCode::Up, KeyModifiers::NONE);
    assert_eq!(app.active_view(), DashboardView::Overview);

    app.set_active_view(DashboardView::Help);
    app.handle_key(KeyCode::Down, KeyModifiers::NONE);
    assert_eq!(app.active_view(), DashboardView::Overview);
}

#[test]
fn dashboard_content_focus_moves_selected_row_without_changing_section() {
    let mut app = DashboardApp::new(outcome());
    app.set_active_view(DashboardView::BrokenLinks);

    assert_eq!(
        app.handle_key(KeyCode::Right, KeyModifiers::NONE),
        DashboardAction::None
    );
    assert_eq!(app.focus(), DashboardFocus::Content);

    app.handle_key(KeyCode::Down, KeyModifiers::NONE);
    app.handle_key(KeyCode::Down, KeyModifiers::NONE);
    assert_eq!(app.active_view(), DashboardView::BrokenLinks);
    assert_eq!(app.selected_row(), 1);

    app.handle_key(KeyCode::Up, KeyModifiers::NONE);
    assert_eq!(app.selected_row(), 0);

    app.handle_key(KeyCode::Left, KeyModifiers::NONE);
    assert_eq!(app.focus(), DashboardFocus::Sidebar);
}

#[test]
fn dashboard_tab_does_not_advance_sections() {
    let mut app = DashboardApp::new(outcome());

    app.handle_key(KeyCode::Tab, KeyModifiers::NONE);

    assert_eq!(app.active_view(), DashboardView::Overview);
    assert_eq!(app.focus(), DashboardFocus::Sidebar);
}

#[test]
fn dashboard_records_export_status_and_quit_request() {
    let mut app = DashboardApp::new(outcome());

    app.set_export_status("Report written to report-home.md".to_string());
    assert_eq!(
        app.export_status(),
        Some("Report written to report-home.md")
    );

    assert!(!app.should_quit());
    app.request_quit();
    assert!(app.should_quit());
}

#[test]
fn dashboard_key_handler_maps_export_and_quit_commands() {
    let mut app = DashboardApp::new(outcome());

    assert_eq!(
        app.handle_key(KeyCode::Char('e'), KeyModifiers::NONE),
        DashboardAction::Export
    );

    assert_eq!(
        app.handle_key(KeyCode::Char('c'), KeyModifiers::CONTROL),
        DashboardAction::Quit
    );
    assert!(app.should_quit());
}

#[test]
fn dashboard_ignores_key_release_events_for_all_shortcuts() {
    let release_cases = [
        KeyEvent::new_with_kind(KeyCode::Down, KeyModifiers::NONE, KeyEventKind::Release),
        KeyEvent::new_with_kind(KeyCode::Right, KeyModifiers::NONE, KeyEventKind::Release),
        KeyEvent::new_with_kind(
            KeyCode::Char('e'),
            KeyModifiers::NONE,
            KeyEventKind::Release,
        ),
        KeyEvent::new_with_kind(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
            KeyEventKind::Release,
        ),
    ];

    for key in release_cases {
        let mut app = DashboardApp::new(outcome());

        assert_eq!(app.handle_key_event(key), DashboardAction::None);
        assert_eq!(app.active_view(), DashboardView::Overview);
        assert_eq!(app.focus(), DashboardFocus::Sidebar);
        assert!(!app.should_quit());
    }
}

#[test]
fn dashboard_handles_key_press_events_once() {
    let mut app = DashboardApp::new(outcome());

    let press = KeyEvent::new_with_kind(KeyCode::Down, KeyModifiers::NONE, KeyEventKind::Press);
    let release = KeyEvent::new_with_kind(KeyCode::Down, KeyModifiers::NONE, KeyEventKind::Release);

    assert_eq!(app.handle_key_event(press), DashboardAction::None);
    assert_eq!(app.handle_key_event(release), DashboardAction::None);
    assert_eq!(app.active_view(), DashboardView::BrokenLinks);
}

#[test]
fn dashboard_visible_window_keeps_selected_finding_visible() {
    let mut app = DashboardApp::new(long_outcome());
    app.set_active_view(DashboardView::Orphans);
    app.handle_key(KeyCode::Right, KeyModifiers::NONE);

    for _ in 0..15 {
        app.handle_key(KeyCode::Down, KeyModifiers::NONE);
    }

    assert_eq!(app.selected_row(), 15);
    assert_eq!(app.visible_item_range(5), 11..16);

    app.set_active_view(DashboardView::Clusters);
    app.handle_key(KeyCode::Right, KeyModifiers::NONE);
    for _ in 0..8 {
        app.handle_key(KeyCode::Down, KeyModifiers::NONE);
    }

    assert_eq!(app.selected_row(), 8);
    assert_eq!(app.visible_item_range(4), 5..9);
}

#[test]
fn dashboard_resolves_edit_targets_for_finding_sections() {
    let cases = [
        (DashboardView::BrokenLinks, Some("Index.md")),
        (DashboardView::AmbiguousLinks, Some("Index.md")),
        (DashboardView::Orphans, Some("Orphan.md")),
        (DashboardView::Stale, Some("Stale.md")),
        (DashboardView::Hubs, Some("Hub.md")),
        (DashboardView::Clusters, Some("Cluster/A.md")),
        (DashboardView::Suggestions, Some("Index.md")),
        (DashboardView::Overview, None),
        (DashboardView::Help, None),
    ];

    for (view, expected) in cases {
        let mut app = DashboardApp::new(edit_target_outcome());
        app.set_active_view(view);

        assert_eq!(app.selected_edit_target().as_deref(), expected);
    }
}

#[test]
fn dashboard_editor_resolution_prefers_visual_then_editor_then_os_fallback() {
    assert_eq!(
        resolve_editor_command(Some("code --wait"), Some("vim"), EditorOs::Windows).program,
        "code"
    );
    assert_eq!(
        resolve_editor_command(None, Some("vim"), EditorOs::Unix).program,
        "vim"
    );
    assert_eq!(
        resolve_editor_command(None, None, EditorOs::Windows).program,
        "notepad"
    );
    assert_eq!(
        resolve_editor_command(None, None, EditorOs::Unix).program,
        "nano"
    );
}

#[test]
fn dashboard_edit_action_only_fires_from_content_focus() {
    let mut app = DashboardApp::new(edit_target_outcome());
    app.set_active_view(DashboardView::Orphans);

    assert_eq!(
        app.handle_key(KeyCode::Enter, KeyModifiers::NONE),
        DashboardAction::None
    );
    assert_eq!(app.focus(), DashboardFocus::Content);

    assert_eq!(
        app.handle_key(KeyCode::Enter, KeyModifiers::NONE),
        DashboardAction::Edit
    );
    assert_eq!(
        app.handle_key(KeyCode::Char('o'), KeyModifiers::NONE),
        DashboardAction::Edit
    );
}

fn outcome() -> ScanOutcome {
    let graph = build_graph(vec![
        note(
            "Index.md",
            "This mentions Rust.",
            vec![link("Missing"), link("Other Missing"), link("Rust")],
        ),
        note("Rust.md", "", vec![]),
        note("Orphan.md", "", vec![]),
    ]);
    let analysis = analyze_graph(&graph, SystemTime::UNIX_EPOCH);

    ScanOutcome {
        vault_path: "D:/Notes/Home".to_string(),
        note_count: graph.notes.len(),
        folder_count: 0,
        wiki_link_count: 3,
        skipped_file_count: 0,
        stale_threshold_days: 90,
        graph,
        analysis,
        phases: vec![
            ScanProgress {
                phase: ScanPhase::DiscoverNotes,
                state: PhaseState::Complete,
                detail: "3 notes".to_string(),
            },
            ScanProgress {
                phase: ScanPhase::BuildGraph,
                state: PhaseState::Attention,
                detail: "1 broken link".to_string(),
            },
        ],
    }
}

fn long_outcome() -> ScanOutcome {
    let mut outcome = edit_target_outcome();
    outcome.analysis.orphan_notes = (0..20)
        .map(|index| format!("Orphan-{index:02}.md"))
        .collect();
    outcome.analysis.clusters = (0..12)
        .map(|index| Cluster {
            notes: vec![format!("Cluster/{index:02}.md")],
        })
        .collect();
    outcome
}

fn edit_target_outcome() -> ScanOutcome {
    let graph = build_graph(vec![
        note("Index.md", "This mentions Rust.", vec![link("Rust")]),
        note("Rust.md", "", vec![]),
        note("Orphan.md", "", vec![]),
        note("Stale.md", "", vec![]),
        note("Hub.md", "", vec![]),
        note("Cluster/A.md", "", vec![]),
        note("Suggestion.md", "", vec![]),
    ]);
    let mut analysis = analyze_graph(&graph, SystemTime::UNIX_EPOCH);
    analysis.orphan_notes = vec!["Orphan.md".to_string()];
    analysis.stale_notes = vec![StaleNote {
        path: "Stale.md".to_string(),
        modified: SystemTime::UNIX_EPOCH,
    }];
    analysis.hub_notes = vec![HubNote {
        path: "Hub.md".to_string(),
        inbound: 3,
        outbound: 2,
    }];
    analysis.clusters = vec![Cluster {
        notes: vec!["Cluster/A.md".to_string(), "Cluster/B.md".to_string()],
    }];
    analysis.suggested_links = vec![SuggestedLink {
        source_path: "Index.md".to_string(),
        target_path: "Suggestion.md".to_string(),
        mention: "Suggestion".to_string(),
    }];

    let mut graph = graph;
    graph.broken_links = vec![BrokenLink {
        source_path: "Index.md".to_string(),
        target: "Missing.md".to_string(),
    }];
    graph.ambiguous_links = vec![AmbiguousLink {
        source_path: "Index.md".to_string(),
        target: "Topic".to_string(),
        candidates: vec!["A/Topic.md".to_string(), "B/Topic.md".to_string()],
    }];

    ScanOutcome {
        vault_path: "D:/Notes/Home".to_string(),
        note_count: graph.notes.len(),
        folder_count: 1,
        wiki_link_count: 1,
        skipped_file_count: 0,
        stale_threshold_days: 90,
        graph,
        analysis,
        phases: vec![],
    }
}

fn note(path: &str, body: &str, links: Vec<WikiLink>) -> GraphNote {
    GraphNote {
        path: path.to_string(),
        title: path.trim_end_matches(".md").to_string(),
        body: body.to_string(),
        links,
        modified: Some(SystemTime::UNIX_EPOCH),
    }
}

fn link(target: &str) -> WikiLink {
    WikiLink {
        target: target.to_string(),
        alias: None,
        raw: format!("[[{target}]]"),
    }
}
