use std::collections::{BTreeMap, BTreeSet};
use std::time::SystemTime;

use crate::parser::WikiLink;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GraphNote {
    pub path: String,
    pub title: String,
    pub body: String,
    pub links: Vec<WikiLink>,
    pub modified: Option<SystemTime>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LinkEdge {
    pub source_path: String,
    pub target_path: String,
    pub target_text: String,
    pub alias: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BrokenLink {
    pub source_path: String,
    pub target: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct AmbiguousLink {
    pub source_path: String,
    pub target: String,
    pub candidates: Vec<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct VaultGraph {
    pub notes: Vec<GraphNote>,
    pub edges: Vec<LinkEdge>,
    pub broken_links: Vec<BrokenLink>,
    pub ambiguous_links: Vec<AmbiguousLink>,
}

impl VaultGraph {
    pub fn inbound_count(&self, path: &str) -> usize {
        self.edges
            .iter()
            .filter(|edge| edge.target_path == path)
            .count()
    }

    pub fn outbound_count(&self, path: &str) -> usize {
        self.edges
            .iter()
            .filter(|edge| edge.source_path == path)
            .count()
    }

    pub fn note_paths(&self) -> BTreeSet<String> {
        self.notes.iter().map(|note| note.path.clone()).collect()
    }
}

pub fn build_graph(mut notes: Vec<GraphNote>) -> VaultGraph {
    notes.sort_by(|left, right| left.path.cmp(&right.path));

    let resolver = Resolver::new(&notes);
    let mut edges = Vec::new();
    let mut broken_links = Vec::new();
    let mut ambiguous_links = Vec::new();

    for note in &notes {
        for link in &note.links {
            match resolver.resolve(&link.target) {
                Resolution::Resolved(target_path) => edges.push(LinkEdge {
                    source_path: note.path.clone(),
                    target_path,
                    target_text: link.target.clone(),
                    alias: link.alias.clone(),
                }),
                Resolution::Broken => broken_links.push(BrokenLink {
                    source_path: note.path.clone(),
                    target: link.target.clone(),
                }),
                Resolution::Ambiguous(candidates) => ambiguous_links.push(AmbiguousLink {
                    source_path: note.path.clone(),
                    target: link.target.clone(),
                    candidates,
                }),
            }
        }
    }

    edges.sort_by(|left, right| {
        (
            left.source_path.as_str(),
            left.target_path.as_str(),
            left.target_text.as_str(),
        )
            .cmp(&(
                right.source_path.as_str(),
                right.target_path.as_str(),
                right.target_text.as_str(),
            ))
    });
    broken_links.sort_by(|left, right| {
        (left.source_path.as_str(), left.target.as_str())
            .cmp(&(right.source_path.as_str(), right.target.as_str()))
    });
    ambiguous_links.sort_by(|left, right| {
        (left.source_path.as_str(), left.target.as_str())
            .cmp(&(right.source_path.as_str(), right.target.as_str()))
    });

    VaultGraph {
        notes,
        edges,
        broken_links,
        ambiguous_links,
    }
}

struct Resolver {
    by_path: BTreeMap<String, String>,
    by_stem: BTreeMap<String, Vec<String>>,
}

enum Resolution {
    Resolved(String),
    Broken,
    Ambiguous(Vec<String>),
}

impl Resolver {
    fn new(notes: &[GraphNote]) -> Self {
        let mut by_path = BTreeMap::new();
        let mut by_stem: BTreeMap<String, Vec<String>> = BTreeMap::new();

        for note in notes {
            by_path.insert(note.path.clone(), note.path.clone());
            by_path.insert(
                note.path.trim_end_matches(".md").to_string(),
                note.path.clone(),
            );
            by_stem
                .entry(stem(&note.path))
                .or_default()
                .push(note.path.clone());
        }

        Self { by_path, by_stem }
    }

    fn resolve(&self, target: &str) -> Resolution {
        let normalized = target.trim().replace('\\', "/");
        if let Some(path) = self.by_path.get(&normalized) {
            return Resolution::Resolved(path.clone());
        }

        let with_extension = format!("{normalized}.md");
        if let Some(path) = self.by_path.get(&with_extension) {
            return Resolution::Resolved(path.clone());
        }

        match self.by_stem.get(&normalized) {
            Some(candidates) if candidates.len() == 1 => {
                Resolution::Resolved(candidates[0].clone())
            }
            Some(candidates) => Resolution::Ambiguous(candidates.clone()),
            None => Resolution::Broken,
        }
    }
}

fn stem(path: &str) -> String {
    path.trim_end_matches(".md")
        .rsplit('/')
        .next()
        .unwrap_or(path)
        .to_string()
}
