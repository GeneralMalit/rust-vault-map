#[derive(Debug, Clone, Eq, PartialEq)]
pub struct WikiLink {
    pub target: String,
    pub alias: Option<String>,
    pub raw: String,
}

pub fn extract_wiki_links(markdown: &str) -> Vec<WikiLink> {
    let mut links = Vec::new();
    let mut remaining = markdown;

    while let Some(start) = remaining.find("[[") {
        let after_start = &remaining[start + 2..];
        let Some(end) = after_start.find("]]") else {
            break;
        };

        let inner = &after_start[..end];
        if !inner.trim().is_empty() {
            let raw = format!("[[{inner}]]");
            let (target, alias) = split_target_alias(inner);
            links.push(WikiLink { target, alias, raw });
        }

        remaining = &after_start[end + 2..];
    }

    links
}

fn split_target_alias(inner: &str) -> (String, Option<String>) {
    match inner.split_once('|') {
        Some((target, alias)) => (target.trim().to_string(), Some(alias.trim().to_string())),
        None => (inner.trim().to_string(), None),
    }
}
