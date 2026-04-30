use std::fs;
use std::path::Path;
use std::time::SystemTime;

use anyhow::Result;

use crate::analysis::{analyze_graph, VaultAnalysis, STALE_DAYS};
use crate::graph::{build_graph, GraphNote, VaultGraph};
use crate::parser::extract_wiki_links;
use crate::report::ReportInput;
use crate::scanner::scan_vault;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ScanPhase {
    DiscoverNotes,
    ParseLinks,
    BuildGraph,
    AnalyzeHealth,
    PrepareReport,
}

impl ScanPhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::DiscoverNotes => "Discover Markdown notes",
            Self::ParseLinks => "Parse Obsidian links",
            Self::BuildGraph => "Build link graph",
            Self::AnalyzeHealth => "Analyze vault health",
            Self::PrepareReport => "Prepare report",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PhaseState {
    Complete,
    Attention,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ScanProgress {
    pub phase: ScanPhase,
    pub state: PhaseState,
    pub detail: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ScanOutcome {
    pub vault_path: String,
    pub note_count: usize,
    pub folder_count: usize,
    pub wiki_link_count: usize,
    pub skipped_file_count: usize,
    pub stale_threshold_days: u64,
    pub graph: VaultGraph,
    pub analysis: VaultAnalysis,
    pub phases: Vec<ScanProgress>,
}

impl ScanOutcome {
    pub fn report_input(&self) -> ReportInput<'_> {
        ReportInput {
            vault_path: self.vault_path.clone(),
            note_count: self.note_count,
            folder_count: self.folder_count,
            wiki_link_count: self.wiki_link_count,
            skipped_file_count: self.skipped_file_count,
            stale_threshold_days: self.stale_threshold_days,
            graph: &self.graph,
            analysis: &self.analysis,
        }
    }
}

pub fn scan_vault_with_progress<F>(vault_path: &Path, mut on_progress: F) -> Result<ScanOutcome>
where
    F: FnMut(ScanProgress),
{
    let mut phases = Vec::new();

    let scan = scan_vault(vault_path)?;
    push_progress(
        &mut phases,
        &mut on_progress,
        ScanProgress {
            phase: ScanPhase::DiscoverNotes,
            state: PhaseState::Complete,
            detail: format!("{} notes · {} folders", scan.notes.len(), scan.folder_count),
        },
    );

    let mut graph_notes = Vec::new();
    let mut skipped_file_count = scan.skipped_files.len();
    let mut wiki_link_count = 0;

    for note in &scan.notes {
        match fs::read_to_string(&note.path) {
            Ok(markdown) => {
                let links = extract_wiki_links(&markdown);
                wiki_link_count += links.len();
                graph_notes.push(GraphNote {
                    path: note.relative_path.clone(),
                    title: title_for_note(&note.relative_path, &markdown),
                    body: markdown,
                    links,
                    modified: note.modified,
                });
            }
            Err(_) => skipped_file_count += 1,
        }
    }

    push_progress(
        &mut phases,
        &mut on_progress,
        ScanProgress {
            phase: ScanPhase::ParseLinks,
            state: attention_if(skipped_file_count > 0),
            detail: format!("{wiki_link_count} wiki links · {skipped_file_count} skipped files"),
        },
    );

    let graph = build_graph(graph_notes);
    push_progress(
        &mut phases,
        &mut on_progress,
        ScanProgress {
            phase: ScanPhase::BuildGraph,
            state: attention_if(
                !graph.broken_links.is_empty() || !graph.ambiguous_links.is_empty(),
            ),
            detail: format!(
                "{} resolved · {} broken · {} ambiguous",
                graph.edges.len(),
                graph.broken_links.len(),
                graph.ambiguous_links.len()
            ),
        },
    );

    let analysis = analyze_graph(&graph, SystemTime::now());
    push_progress(
        &mut phases,
        &mut on_progress,
        ScanProgress {
            phase: ScanPhase::AnalyzeHealth,
            state: attention_if(
                !analysis.orphan_notes.is_empty()
                    || !analysis.stale_notes.is_empty()
                    || !analysis.suggested_links.is_empty(),
            ),
            detail: format!(
                "{} orphan · {} stale · {} suggestions",
                analysis.orphan_notes.len(),
                analysis.stale_notes.len(),
                analysis.suggested_links.len()
            ),
        },
    );

    push_progress(
        &mut phases,
        &mut on_progress,
        ScanProgress {
            phase: ScanPhase::PrepareReport,
            state: PhaseState::Complete,
            detail: "ready".to_string(),
        },
    );

    Ok(ScanOutcome {
        vault_path: vault_path.display().to_string(),
        note_count: graph.notes.len(),
        folder_count: scan.folder_count,
        wiki_link_count,
        skipped_file_count,
        stale_threshold_days: STALE_DAYS,
        graph,
        analysis,
        phases,
    })
}

pub fn scan_vault_quiet(vault_path: &Path) -> Result<ScanOutcome> {
    scan_vault_with_progress(vault_path, |_| {})
}

fn push_progress<F>(phases: &mut Vec<ScanProgress>, on_progress: &mut F, event: ScanProgress)
where
    F: FnMut(ScanProgress),
{
    on_progress(event.clone());
    phases.push(event);
}

fn attention_if(condition: bool) -> PhaseState {
    if condition {
        PhaseState::Attention
    } else {
        PhaseState::Complete
    }
}

fn title_for_note(relative_path: &str, markdown: &str) -> String {
    markdown
        .lines()
        .find_map(|line| line.strip_prefix("# ").map(str::trim))
        .filter(|title| !title.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            relative_path
                .trim_end_matches(".md")
                .rsplit('/')
                .next()
                .unwrap_or(relative_path)
                .to_string()
        })
}
