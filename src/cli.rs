use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use is_terminal::IsTerminal;
use rand::Rng;

use crate::analysis::{analyze_graph, STALE_DAYS};
use crate::graph::{build_graph, GraphNote};
use crate::interactive::run_interactive_report;
use crate::parser::extract_wiki_links;
use crate::report::{render_markdown_report, render_report, render_report_colored, ReportInput};
use crate::scanner::scan_vault;

#[derive(Debug, Parser)]
#[command(name = "rust-vault-map")]
#[command(about = "Analyze an Obsidian-style Markdown vault")]
pub struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Scan {
        vault_path: PathBuf,
        #[arg(long, hide = true)]
        interactive: bool,
        #[arg(long)]
        report: bool,
    },
}

pub fn run() -> Result<()> {
    let args = Args::parse();

    match args.command {
        Command::Scan {
            vault_path,
            interactive,
            report,
        } => {
            if report {
                let report_path = write_markdown_report(&vault_path)?;
                println!("Report written to {}", report_path.display());
            } else if interactive || (io::stdin().is_terminal() && io::stdout().is_terminal()) {
                let report = scan_command(&vault_path)?;
                if io::stdin().is_terminal() && io::stdout().is_terminal() {
                    run_interactive_report(&report)?;
                } else {
                    eprintln!("Interactive terminal unavailable; showing report instead.");
                    println!("{report}");
                }
            } else if io::stdout().is_terminal() {
                let report = scan_command_colored(&vault_path)?;
                eprintln!("Interactive terminal unavailable; showing report instead.");
                println!("{report}");
            } else {
                let report = scan_command(&vault_path)?;
                eprintln!("Interactive terminal unavailable; showing report instead.");
                println!("{report}");
            }
            Ok(())
        }
    }
}

pub fn scan_command(vault_path: &Path) -> Result<String> {
    scan_command_with_renderer(vault_path, render_report)
}

pub fn scan_command_colored(vault_path: &Path) -> Result<String> {
    scan_command_with_renderer(vault_path, render_report_colored)
}

pub fn scan_command_markdown(vault_path: &Path) -> Result<String> {
    scan_command_with_renderer(vault_path, render_markdown_report)
}

pub fn write_markdown_report(vault_path: &Path) -> Result<PathBuf> {
    let markdown = scan_command_markdown(vault_path)?;
    let file_name = report_file_name(vault_path);
    let report_path = std::env::current_dir()
        .context("failed to resolve current directory")?
        .join(file_name);
    fs::write(&report_path, markdown)
        .with_context(|| format!("failed to write report to {}", report_path.display()))?;
    Ok(report_path)
}

fn scan_command_with_renderer(
    vault_path: &Path,
    renderer: fn(&ReportInput<'_>) -> String,
) -> Result<String> {
    let scan = scan_vault(vault_path)?;
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

    let graph = build_graph(graph_notes);
    let analysis = analyze_graph(&graph, SystemTime::now());

    let input = ReportInput {
        vault_path: vault_path.display().to_string(),
        note_count: graph.notes.len(),
        folder_count: scan.folder_count,
        wiki_link_count,
        skipped_file_count,
        stale_threshold_days: STALE_DAYS,
        graph: &graph,
        analysis: &analysis,
    };

    Ok(renderer(&input))
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

fn report_file_name(vault_path: &Path) -> String {
    let folder_name = vault_path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("vault");
    let folder_name = sanitize_file_stem(folder_name);
    let hash = rand::thread_rng().gen::<u32>();

    format!("report-{folder_name}-{hash:08x}.md")
}

fn sanitize_file_stem(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();

    if sanitized.is_empty() {
        "vault".to_string()
    } else {
        sanitized
    }
}
