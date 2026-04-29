use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{bail, Context, Result};
use walkdir::{DirEntry, WalkDir};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct NoteMetadata {
    pub path: PathBuf,
    pub relative_path: String,
    pub modified: Option<SystemTime>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct VaultScan {
    pub notes: Vec<NoteMetadata>,
    pub folder_count: usize,
    pub skipped_files: Vec<SkippedFile>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SkippedFile {
    pub relative_path: String,
    pub reason: String,
}

pub fn scan_vault(vault_path: &Path) -> Result<VaultScan> {
    if !vault_path.exists() {
        bail!("vault path does not exist: {}", vault_path.display());
    }

    if !vault_path.is_dir() {
        bail!("vault path is not a directory: {}", vault_path.display());
    }

    let mut notes = Vec::new();
    let mut folders = BTreeSet::new();

    for entry in WalkDir::new(vault_path)
        .into_iter()
        .filter_entry(|entry| should_visit(entry, vault_path))
    {
        let entry = entry
            .with_context(|| format!("failed to read entry under {}", vault_path.display()))?;
        if !entry.file_type().is_file()
            || entry.path().extension().and_then(|ext| ext.to_str()) != Some("md")
        {
            continue;
        }

        let relative_path = normalize_relative_path(vault_path, entry.path())?;
        if let Some(parent) = parent_folder(&relative_path) {
            folders.insert(parent);
        }

        let modified = entry
            .metadata()
            .ok()
            .and_then(|metadata| metadata.modified().ok());
        notes.push(NoteMetadata {
            path: entry.path().to_path_buf(),
            relative_path,
            modified,
        });
    }

    notes.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));

    Ok(VaultScan {
        notes,
        folder_count: folders.len(),
        skipped_files: Vec::new(),
    })
}

fn should_visit(entry: &DirEntry, vault_path: &Path) -> bool {
    if entry.path() == vault_path {
        return true;
    }

    if !entry.file_type().is_dir() {
        return true;
    }

    let name = entry.file_name().to_string_lossy();
    !name.starts_with('.') && !matches!(name.as_ref(), "target" | "node_modules" | "dist" | "build")
}

fn normalize_relative_path(root: &Path, path: &Path) -> Result<String> {
    let relative = path
        .strip_prefix(root)
        .with_context(|| format!("{} is not under {}", path.display(), root.display()))?;

    Ok(relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/"))
}

fn parent_folder(relative_path: &str) -> Option<String> {
    let (folder, _) = relative_path.rsplit_once('/')?;
    Some(folder.to_string())
}
