use assert_cmd::Command;
use predicates::prelude::*;
use rust_vault_map::cli::scan_command_colored;
use std::fs;
use tempfile::TempDir;

#[test]
fn scan_command_reports_fixture_vault() {
    let fixture = fixture_vault();
    let root = fixture.path();

    let mut cmd = Command::cargo_bin("rust-vault-map").expect("binary");
    cmd.arg("scan").arg(root);

    cmd.assert()
        .success()
        .stderr(predicate::str::contains(
            "Interactive terminal unavailable; showing report instead.",
        ))
        .stdout(predicate::str::starts_with("rust-vault-map scan report"))
        .stdout(predicate::str::contains("rust-vault-map scan report"))
        .stdout(predicate::str::contains("Markdown notes: 9"))
        .stdout(predicate::str::contains("Folders: 3"))
        .stdout(predicate::str::contains("Obsidian links: 13"))
        .stdout(predicate::str::contains("Index.md -> Missing Note"))
        .stdout(predicate::str::contains("Orphan.md"));
}

#[test]
fn scan_report_mode_writes_markdown_report_and_prints_path() {
    let fixture = fixture_vault();
    let root = fixture.path();
    let output_dir = TempDir::new().expect("output dir");

    let mut cmd = Command::cargo_bin("rust-vault-map").expect("binary");
    cmd.current_dir(output_dir.path())
        .arg("scan")
        .arg(root)
        .arg("--report");

    let output = cmd.assert().success().get_output().stdout.clone();
    let output = String::from_utf8(output).expect("utf8");
    let report_path = output
        .trim()
        .strip_prefix("Report written to ")
        .expect("printed report path");
    let report_path = std::path::Path::new(report_path);

    assert!(report_path.exists());
    let file_name = report_path.file_name().unwrap().to_string_lossy();
    assert!(file_name.starts_with("report-"));
    assert!(file_name.ends_with(".md"));

    let report = fs::read_to_string(report_path).expect("read report");
    assert!(report.contains("# rust-vault-map scan report"));
    assert!(report.contains("Markdown notes: 9"));
    assert!(report.contains("Index.md -> Missing Note"));
    assert!(!root.join("report.md").exists());
}

#[test]
fn colored_scan_command_renders_ansi_styled_report() {
    let fixture = fixture_vault();
    let root = fixture.path();

    let output = scan_command_colored(root).expect("colored scan");

    assert!(output.contains("\u{1b}["));
    assert!(output.contains("rust-vault-map scan report"));
}

fn fixture_vault() -> TempDir {
    let temp = TempDir::new().expect("fixture tempdir");
    let root = temp.path();

    fs::create_dir(root.join("Projects")).expect("create Projects");
    fs::create_dir(root.join("Clusters")).expect("create Clusters");
    fs::create_dir(root.join("Areas")).expect("create Areas");
    fs::create_dir(root.join("target")).expect("create target");

    fs::write(
        root.join("Index.md"),
        "# Index\n[[Rust]] [[Projects/Vault Map|Vault Map]] [[Missing Note]]\n",
    )
    .expect("write Index");
    fs::write(root.join("Rust.md"), "# Rust\n[[Index]] [[Ownership]]\n").expect("write Rust");
    fs::write(root.join("Ownership.md"), "# Ownership\n[[Index]]\n").expect("write Ownership");
    fs::write(root.join("Orphan.md"), "# Orphan\n").expect("write Orphan");
    fs::write(
        root.join("Projects").join("Vault Map.md"),
        "# Vault Map\n[[Projects/Graphs]] [[Rust]]\n",
    )
    .expect("write Vault Map");
    fs::write(
        root.join("Projects").join("Graphs.md"),
        "# Graphs\n[[Projects/Vault Map]]\n",
    )
    .expect("write Graphs");
    fs::write(
        root.join("Clusters").join("Alpha.md"),
        "# Alpha\n[[Clusters/Beta]]\n",
    )
    .expect("write Alpha");
    fs::write(
        root.join("Clusters").join("Beta.md"),
        "# Beta\n[[Clusters/Alpha]]\n",
    )
    .expect("write Beta");
    fs::write(
        root.join("Areas").join("Writing.md"),
        "# Writing\n[[Index]] [[Rust]]\n",
    )
    .expect("write Writing");
    fs::write(root.join("target").join("Ignored.md"), "# Ignored\n").expect("write ignored");

    temp
}
