use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use is_terminal::IsTerminal;
use rand::Rng;

use crate::report::{render_markdown_report, render_report, render_report_colored, ReportInput};
use crate::scan::{scan_vault_quiet, ScanOutcome};
use crate::tui::run_dashboard;

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
        #[arg(long)]
        plain: bool,
    },
}

pub fn run() -> Result<()> {
    let args = Args::parse();

    match args.command {
        Command::Scan {
            vault_path,
            interactive,
            report,
            plain,
        } => {
            if report {
                let report_path = write_markdown_report(&vault_path)?;
                println!("Report written to {}", report_path.display());
            } else if plain || !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
                let report = scan_command(&vault_path)?;
                println!("{report}");
            } else if interactive || std::io::stdout().is_terminal() {
                let outcome = scan_vault_quiet(&vault_path)?;
                run_dashboard(outcome, |outcome| {
                    export_markdown_report(outcome).map(|path| path.display().to_string())
                })?;
            } else {
                let report = scan_command(&vault_path)?;
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
    let outcome = scan_vault_quiet(vault_path)?;
    export_markdown_report(&outcome)
}

pub fn export_markdown_report(outcome: &ScanOutcome) -> Result<PathBuf> {
    let markdown = render_markdown_report(&outcome.report_input());
    let file_name = report_file_name(Path::new(&outcome.vault_path));
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
    let outcome = scan_vault_quiet(vault_path)?;
    Ok(renderer(&outcome.report_input()))
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
