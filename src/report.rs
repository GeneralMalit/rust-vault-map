use std::collections::BTreeMap;

use crate::analysis::{SuggestedLink, VaultAnalysis};
use crate::graph::VaultGraph;

const SECTION_LIMIT: usize = 10;
const CLUSTER_LIMIT: usize = 5;
const CLUSTER_SAMPLE_LIMIT: usize = 5;

pub struct ReportInput<'a> {
    pub vault_path: String,
    pub note_count: usize,
    pub folder_count: usize,
    pub wiki_link_count: usize,
    pub skipped_file_count: usize,
    pub stale_threshold_days: u64,
    pub graph: &'a VaultGraph,
    pub analysis: &'a VaultAnalysis,
}

pub fn render_report(input: &ReportInput<'_>) -> String {
    render_report_inner(input, false)
}

pub fn render_report_colored(input: &ReportInput<'_>) -> String {
    render_report_inner(input, true)
}

pub fn render_markdown_report(input: &ReportInput<'_>) -> String {
    let report = render_report(input);
    let headings = [
        "Summary",
        "Top findings",
        "Broken links",
        "Ambiguous links",
        "Orphan notes",
        "Stale notes",
        "Hub notes",
        "Clusters",
        "Suggested links",
        "Next actions",
    ];
    let mut lines = Vec::new();

    for (index, line) in report.lines().enumerate() {
        if index == 0 {
            lines.push(format!("# {line}"));
        } else if headings.contains(&line) {
            lines.push(format!("## {line}"));
        } else {
            lines.push(line.to_string());
        }
    }

    lines.push(String::new());
    lines.join("\n")
}

fn render_report_inner(input: &ReportInput<'_>, colored: bool) -> String {
    let mut lines = Vec::new();
    lines.push(format_heading("rust-vault-map scan report", colored));
    lines.push(format!("Scanned path: {}", input.vault_path));
    lines.push(String::new());

    lines.push(format_section("Summary", colored));
    lines.push(format!("Markdown notes: {}", input.note_count));
    lines.push(format!("Folders: {}", input.folder_count));
    lines.push(format!("Obsidian links: {}", input.wiki_link_count));
    lines.push(format!("Broken links: {}", input.graph.broken_links.len()));
    lines.push(format!(
        "Ambiguous links: {}",
        input.graph.ambiguous_links.len()
    ));
    lines.push(format!(
        "Orphan notes: {}",
        input.analysis.orphan_notes.len()
    ));
    lines.push(format!("Stale notes: {}", input.analysis.stale_notes.len()));
    lines.push(format!(
        "Stale threshold: {} days",
        input.stale_threshold_days
    ));
    match input.analysis.oldest_note_age_days {
        Some(age) => lines.push(format!("Oldest scanned note age: {age} days")),
        None => lines.push("Oldest scanned note age: unknown".to_string()),
    }
    lines.push(format!("Skipped files: {}", input.skipped_file_count));
    lines.push(String::new());

    lines.push(format_section("Top findings", colored));
    if input.graph.broken_links.is_empty()
        && input.graph.ambiguous_links.is_empty()
        && input.analysis.orphan_notes.is_empty()
        && input.analysis.suggested_links.is_empty()
    {
        lines.push("No urgent cleanup found.".to_string());
    } else {
        lines.push(format_finding(
            &format!("{} broken links", input.graph.broken_links.len()),
            input.graph.broken_links.is_empty(),
            colored,
        ));
        lines.push(format_finding(
            &format!("{} ambiguous links", input.graph.ambiguous_links.len()),
            input.graph.ambiguous_links.is_empty(),
            colored,
        ));
        lines.push(format_finding(
            &format!("{} orphan notes", input.analysis.orphan_notes.len()),
            input.analysis.orphan_notes.is_empty(),
            colored,
        ));
        lines.push(format_finding(
            &format!("{} suggested links", input.analysis.suggested_links.len()),
            input.analysis.suggested_links.is_empty(),
            colored,
        ));
    }
    lines.push(String::new());

    lines.push(format_section("Broken links", colored));
    if input.graph.broken_links.is_empty() {
        lines.push("No broken links found.".to_string());
    } else {
        for broken in input.graph.broken_links.iter().take(SECTION_LIMIT) {
            lines.push(format!("{} -> {}", broken.source_path, broken.target));
        }
        push_omitted(&mut lines, input.graph.broken_links.len());
    }
    lines.push(String::new());

    lines.push(format_section("Ambiguous links", colored));
    if input.graph.ambiguous_links.is_empty() {
        lines.push("No ambiguous links found.".to_string());
    } else {
        for ambiguous in input.graph.ambiguous_links.iter().take(SECTION_LIMIT) {
            lines.push(format!("{} -> {}", ambiguous.source_path, ambiguous.target));
            lines.push(format!("  candidates: {}", ambiguous.candidates.join(", ")));
        }
        push_omitted(&mut lines, input.graph.ambiguous_links.len());
    }
    lines.push(String::new());

    lines.push(format_section("Orphan notes", colored));
    push_string_list(
        &mut lines,
        &input.analysis.orphan_notes,
        "No orphan notes found.",
    );
    lines.push(String::new());

    lines.push(format_section("Stale notes", colored));
    if input.analysis.stale_notes.is_empty() {
        lines.push(format!(
            "No stale notes found because no scanned note is older than {} days.",
            input.stale_threshold_days
        ));
    } else {
        for stale in input.analysis.stale_notes.iter().take(SECTION_LIMIT) {
            lines.push(stale.path.clone());
        }
        push_omitted(&mut lines, input.analysis.stale_notes.len());
    }
    lines.push(String::new());

    lines.push(format_section("Hub notes", colored));
    if input.analysis.hub_notes.is_empty() {
        lines.push("No hub notes found.".to_string());
    } else {
        for hub in input.analysis.hub_notes.iter().take(SECTION_LIMIT) {
            lines.push(format!(
                "{} (inbound: {}, outbound: {})",
                hub.path, hub.inbound, hub.outbound
            ));
        }
        push_omitted(&mut lines, input.analysis.hub_notes.len());
    }
    lines.push(String::new());

    lines.push(format_section("Clusters", colored));
    if input.analysis.clusters.is_empty() {
        lines.push("No clusters found.".to_string());
    } else {
        let multi_note_count = input
            .analysis
            .clusters
            .iter()
            .filter(|cluster| cluster.notes.len() > 1)
            .count();
        let single_note_count = input
            .analysis
            .clusters
            .iter()
            .filter(|cluster| cluster.notes.len() == 1)
            .count();
        lines.push(format!("Clusters: {} total", input.analysis.clusters.len()));
        lines.push(format!("{multi_note_count} multi-note clusters"));
        lines.push(format!("{single_note_count} one-note clusters"));

        for cluster in input
            .analysis
            .clusters
            .iter()
            .filter(|cluster| cluster.notes.len() > 1)
            .take(CLUSTER_LIMIT)
        {
            let samples = cluster
                .notes
                .iter()
                .take(CLUSTER_SAMPLE_LIMIT)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!(
                "{} notes; samples: {}",
                cluster.notes.len(),
                samples
            ));
        }
        if multi_note_count > CLUSTER_LIMIT {
            lines.push(format!(
                "... {} more multi-note clusters omitted",
                multi_note_count - CLUSTER_LIMIT
            ));
        }
    }
    lines.push(String::new());

    lines.push(format_section("Suggested links", colored));
    if input.analysis.suggested_links.is_empty() {
        lines.push("No suggested links found.".to_string());
    } else {
        push_grouped_suggestions(&mut lines, &input.analysis.suggested_links);
    }
    lines.push(String::new());

    lines.push(format_section("Next actions", colored));
    if !input.graph.broken_links.is_empty() {
        lines.push("Fix broken links first.".to_string());
    } else if !input.analysis.orphan_notes.is_empty() {
        lines.push("Review orphan notes for missing connections.".to_string());
    } else if !input.analysis.suggested_links.is_empty() {
        lines.push("Review suggested links.".to_string());
    } else {
        lines.push("Vault graph looks tidy.".to_string());
    }

    lines.join("\n")
}

fn format_heading(text: &str, colored: bool) -> String {
    if colored {
        format!("\x1b[1;36m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

fn format_section(text: &str, colored: bool) -> String {
    if colored {
        format!("\x1b[1;34m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

fn format_finding(text: &str, is_empty: bool, colored: bool) -> String {
    if !colored {
        return text.to_string();
    }

    if is_empty {
        format!("\x1b[32m{text}\x1b[0m")
    } else {
        format!("\x1b[33m{text}\x1b[0m")
    }
}

fn push_string_list(lines: &mut Vec<String>, values: &[String], empty_message: &str) {
    if values.is_empty() {
        lines.push(empty_message.to_string());
        return;
    }

    for value in values.iter().take(SECTION_LIMIT) {
        lines.push(value.clone());
    }
    push_omitted(lines, values.len());
}

fn push_omitted(lines: &mut Vec<String>, total: usize) {
    if total > SECTION_LIMIT {
        lines.push(format!("... {} more omitted", total - SECTION_LIMIT));
    }
}

fn push_grouped_suggestions(lines: &mut Vec<String>, suggestions: &[SuggestedLink]) {
    let mut grouped: BTreeMap<&str, Vec<&SuggestedLink>> = BTreeMap::new();
    for suggestion in suggestions {
        grouped
            .entry(suggestion.source_path.as_str())
            .or_default()
            .push(suggestion);
    }

    let mut shown = 0;
    for (source, source_suggestions) in grouped {
        if shown >= SECTION_LIMIT {
            break;
        }

        lines.push(source.to_string());
        for suggestion in source_suggestions {
            if shown >= SECTION_LIMIT {
                break;
            }

            lines.push(format!(
                "  - {} -> {}",
                suggestion.mention, suggestion.target_path
            ));
            shown += 1;
        }
    }

    if suggestions.len() > shown {
        lines.push(format!("... {} more omitted", suggestions.len() - shown));
    }
}
