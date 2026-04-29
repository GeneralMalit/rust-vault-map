use rust_vault_map::interactive::{interactive_prompt, run_interactive_report_with_io};

#[test]
fn interactive_report_shows_overview_and_selected_section() {
    let report = "\
rust-vault-map scan report
Scanned path: fixtures/vaults/v1

Summary
Markdown notes: 9
Broken links: 1

Top findings
1 broken links

Broken links
Index.md -> Missing Note

Orphan notes
Orphan.md
";
    let mut output = Vec::new();

    run_interactive_report_with_io(report, "1\nq\n".as_bytes(), &mut output).expect("interactive");

    let output = String::from_utf8(output).expect("utf8");
    assert!(output.contains("rust-vault-map interactive scan"));
    assert!(output.contains("Markdown notes: 9"));
    assert!(output.contains("Use arrow keys to choose a section:"));
    assert!(output.contains("Index.md -> Missing Note"));
}

#[test]
fn interactive_report_can_extract_sections_from_colored_report() {
    let report = "\
\u{1b}[1;36mrust-vault-map scan report\u{1b}[0m
Scanned path: fixtures/vaults/v1

\u{1b}[1;34mSummary\u{1b}[0m
Markdown notes: 9

\u{1b}[1;34mTop findings\u{1b}[0m
1 broken links

\u{1b}[1;34mBroken links\u{1b}[0m
Index.md -> Missing Note

\u{1b}[1;34mOrphan notes\u{1b}[0m
Orphan.md
";
    let mut output = Vec::new();

    run_interactive_report_with_io(report, "1\nq\n".as_bytes(), &mut output).expect("interactive");

    let output = String::from_utf8(output).expect("utf8");
    assert!(output.contains("Index.md -> Missing Note"));
}

#[test]
fn interactive_prompt_tells_users_to_use_arrow_keys() {
    assert!(interactive_prompt().contains("arrow keys"));
}
