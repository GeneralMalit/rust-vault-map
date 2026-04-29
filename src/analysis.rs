use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::time::{Duration, SystemTime};

use crate::graph::VaultGraph;

pub const STALE_DAYS: u64 = 90;
const MAX_SUGGESTIONS: usize = 20;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct VaultAnalysis {
    pub orphan_notes: Vec<String>,
    pub stale_notes: Vec<StaleNote>,
    pub oldest_note_age_days: Option<u64>,
    pub hub_notes: Vec<HubNote>,
    pub clusters: Vec<Cluster>,
    pub suggested_links: Vec<SuggestedLink>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct StaleNote {
    pub path: String,
    pub modified: SystemTime,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct HubNote {
    pub path: String,
    pub inbound: usize,
    pub outbound: usize,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Cluster {
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SuggestedLink {
    pub source_path: String,
    pub target_path: String,
    pub mention: String,
}

pub fn analyze_graph(graph: &VaultGraph, now: SystemTime) -> VaultAnalysis {
    VaultAnalysis {
        orphan_notes: orphan_notes(graph),
        stale_notes: stale_notes(graph, now),
        oldest_note_age_days: oldest_note_age_days(graph, now),
        hub_notes: hub_notes(graph),
        clusters: clusters(graph),
        suggested_links: suggested_links(graph),
    }
}

fn orphan_notes(graph: &VaultGraph) -> Vec<String> {
    graph
        .notes
        .iter()
        .filter(|note| {
            graph.inbound_count(&note.path) == 0 && graph.outbound_count(&note.path) == 0
        })
        .map(|note| note.path.clone())
        .collect()
}

fn stale_notes(graph: &VaultGraph, now: SystemTime) -> Vec<StaleNote> {
    let threshold = Duration::from_secs(STALE_DAYS * 24 * 60 * 60);
    let mut stale: Vec<_> = graph
        .notes
        .iter()
        .filter_map(|note| {
            let modified = note.modified?;
            if now.duration_since(modified).ok()? > threshold {
                Some(StaleNote {
                    path: note.path.clone(),
                    modified,
                })
            } else {
                None
            }
        })
        .collect();

    stale.sort_by(|left, right| {
        left.modified
            .cmp(&right.modified)
            .then_with(|| left.path.cmp(&right.path))
    });
    stale
}

fn oldest_note_age_days(graph: &VaultGraph, now: SystemTime) -> Option<u64> {
    graph
        .notes
        .iter()
        .filter_map(|note| {
            let modified = note.modified?;
            Some(now.duration_since(modified).ok()?.as_secs() / 86_400)
        })
        .max()
}

fn hub_notes(graph: &VaultGraph) -> Vec<HubNote> {
    let mut hubs: Vec<_> = graph
        .notes
        .iter()
        .map(|note| HubNote {
            path: note.path.clone(),
            inbound: graph.inbound_count(&note.path),
            outbound: graph.outbound_count(&note.path),
        })
        .filter(|hub| hub.inbound + hub.outbound > 0)
        .collect();

    hubs.sort_by(|left, right| {
        (right.inbound + right.outbound)
            .cmp(&(left.inbound + left.outbound))
            .then_with(|| left.path.cmp(&right.path))
    });
    hubs
}

fn clusters(graph: &VaultGraph) -> Vec<Cluster> {
    let mut adjacency: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for note in &graph.notes {
        adjacency.entry(note.path.clone()).or_default();
    }
    for edge in &graph.edges {
        adjacency
            .entry(edge.source_path.clone())
            .or_default()
            .insert(edge.target_path.clone());
        adjacency
            .entry(edge.target_path.clone())
            .or_default()
            .insert(edge.source_path.clone());
    }

    let mut seen = BTreeSet::new();
    let mut result = Vec::new();

    for start in adjacency.keys() {
        if seen.contains(start) {
            continue;
        }

        let mut queue = VecDeque::from([start.clone()]);
        let mut notes = Vec::new();
        seen.insert(start.clone());

        while let Some(path) = queue.pop_front() {
            notes.push(path.clone());
            if let Some(neighbors) = adjacency.get(&path) {
                for neighbor in neighbors {
                    if seen.insert(neighbor.clone()) {
                        queue.push_back(neighbor.clone());
                    }
                }
            }
        }

        notes.sort();
        result.push(Cluster { notes });
    }

    result.sort_by(|left, right| {
        right
            .notes
            .len()
            .cmp(&left.notes.len())
            .then_with(|| left.notes.first().cmp(&right.notes.first()))
    });
    result
}

fn suggested_links(graph: &VaultGraph) -> Vec<SuggestedLink> {
    let existing_edges: BTreeSet<_> = graph
        .edges
        .iter()
        .map(|edge| (edge.source_path.as_str(), edge.target_path.as_str()))
        .collect();
    let mut suggestions = Vec::new();

    for source in &graph.notes {
        for target in &graph.notes {
            if source.path == target.path {
                continue;
            }
            if is_weak_suggestion_title(&target.title) {
                continue;
            }
            if existing_edges.contains(&(source.path.as_str(), target.path.as_str())) {
                continue;
            }
            if target.title.is_empty() || !source.body.contains(&target.title) {
                continue;
            }

            suggestions.push(SuggestedLink {
                source_path: source.path.clone(),
                target_path: target.path.clone(),
                mention: target.title.clone(),
            });
        }
    }

    suggestions.sort_by(|left, right| {
        (
            left.source_path.as_str(),
            left.target_path.as_str(),
            left.mention.as_str(),
        )
            .cmp(&(
                right.source_path.as_str(),
                right.target_path.as_str(),
                right.mention.as_str(),
            ))
    });
    suggestions.truncate(MAX_SUGGESTIONS);
    suggestions
}

fn is_weak_suggestion_title(title: &str) -> bool {
    let normalized = title.trim().to_ascii_lowercase();
    normalized.len() < 4
        || matches!(
            normalized.as_str(),
            "index"
                | "readme"
                | "spec"
                | "task"
                | "tasks"
                | "todo"
                | "note"
                | "notes"
                | "draft"
                | "proposal"
                | "design"
                | "context"
                | "work"
                | "changelog"
                | "license"
        )
}
