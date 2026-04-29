use std::fs;

use rust_vault_map::scanner::scan_vault;
use tempfile::TempDir;

#[test]
fn scan_vault_discovers_markdown_files_recursively_and_skips_hidden_build_dirs() {
    let temp = TempDir::new().expect("tempdir");
    let root = temp.path();

    fs::write(root.join("Index.md"), "# Index").expect("write Index.md");
    fs::create_dir(root.join("Projects")).expect("create Projects");
    fs::write(root.join("Projects").join("Vault Map.md"), "# Vault Map")
        .expect("write nested note");
    fs::write(root.join("Projects").join("notes.txt"), "ignore").expect("write txt");

    fs::create_dir(root.join(".obsidian")).expect("create .obsidian");
    fs::write(root.join(".obsidian").join("Ignored.md"), "# Ignored").expect("write ignored");
    fs::create_dir(root.join("target")).expect("create target");
    fs::write(root.join("target").join("Ignored.md"), "# Ignored").expect("write target");

    let scan = scan_vault(root).expect("scan succeeds");
    let paths: Vec<_> = scan
        .notes
        .iter()
        .map(|note| note.relative_path.as_str())
        .collect();

    assert_eq!(paths, vec!["Index.md", "Projects/Vault Map.md"]);
    assert_eq!(scan.folder_count, 1);
}
