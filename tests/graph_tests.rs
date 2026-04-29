use rust_vault_map::graph::{build_graph, GraphNote};
use rust_vault_map::parser::WikiLink;

#[test]
fn graph_resolves_valid_links_preserves_aliases_and_tracks_degrees() {
    let graph = build_graph(vec![
        note(
            "Index.md",
            vec![
                link("Rust", None),
                link("Projects/Vault Map", Some("Vault Map")),
            ],
        ),
        note("Rust.md", vec![link("Index", None)]),
        note("Projects/Vault Map.md", vec![]),
    ]);

    assert_eq!(graph.edges.len(), 3);
    let vault_map_edge = graph
        .edges
        .iter()
        .find(|edge| edge.target_path == "Projects/Vault Map.md")
        .expect("vault map edge");
    assert_eq!(vault_map_edge.source_path, "Index.md");
    assert_eq!(vault_map_edge.alias.as_deref(), Some("Vault Map"));
    assert_eq!(graph.outbound_count("Index.md"), 2);
    assert_eq!(graph.inbound_count("Index.md"), 1);
    assert_eq!(graph.inbound_count("Projects/Vault Map.md"), 1);
}

#[test]
fn graph_records_broken_links_deterministically() {
    let graph = build_graph(vec![
        note(
            "Index.md",
            vec![link("Missing", None), link("Also Missing", None)],
        ),
        note("Rust.md", vec![]),
    ]);

    let broken: Vec<_> = graph
        .broken_links
        .iter()
        .map(|broken| (broken.source_path.as_str(), broken.target.as_str()))
        .collect();

    assert_eq!(
        broken,
        vec![("Index.md", "Also Missing"), ("Index.md", "Missing")]
    );
}

fn note(path: &str, links: Vec<WikiLink>) -> GraphNote {
    GraphNote {
        path: path.to_string(),
        title: path
            .trim_end_matches(".md")
            .rsplit('/')
            .next()
            .unwrap()
            .to_string(),
        body: String::new(),
        links,
        modified: None,
    }
}

fn link(target: &str, alias: Option<&str>) -> WikiLink {
    let raw = match alias {
        Some(alias) => format!("[[{target}|{alias}]]"),
        None => format!("[[{target}]]"),
    };

    WikiLink {
        target: target.to_string(),
        alias: alias.map(str::to_string),
        raw,
    }
}
