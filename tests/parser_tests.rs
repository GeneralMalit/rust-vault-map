use rust_vault_map::parser::{extract_wiki_links, WikiLink};

#[test]
fn extracts_required_obsidian_wiki_link_variants() {
    let links = extract_wiki_links(
        "See [[Note]], [[Note|Alias]], [[Folder/Note]], and [[Folder/Note|Folder Alias]].",
    );

    assert_eq!(
        links,
        vec![
            WikiLink {
                target: "Note".to_string(),
                alias: None,
                raw: "[[Note]]".to_string(),
            },
            WikiLink {
                target: "Note".to_string(),
                alias: Some("Alias".to_string()),
                raw: "[[Note|Alias]]".to_string(),
            },
            WikiLink {
                target: "Folder/Note".to_string(),
                alias: None,
                raw: "[[Folder/Note]]".to_string(),
            },
            WikiLink {
                target: "Folder/Note".to_string(),
                alias: Some("Folder Alias".to_string()),
                raw: "[[Folder/Note|Folder Alias]]".to_string(),
            },
        ]
    );
}

#[test]
fn extracts_multiple_links_per_line_in_source_order() {
    let links = extract_wiki_links("[[Alpha]] and [[Beta|B]] connect to [[Folder/Gamma]].");

    let targets: Vec<_> = links.iter().map(|link| link.target.as_str()).collect();

    assert_eq!(targets, vec!["Alpha", "Beta", "Folder/Gamma"]);
}
