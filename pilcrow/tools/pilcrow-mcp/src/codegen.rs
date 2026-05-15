use crate::workspace::{display_path, find_out_dir, list_files_recursive, resolve_against};
use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Debug, Clone, Serialize)]
pub struct CodegenBuildResult {
    pub success: bool,
    pub manifest_path: String,
    pub stdout: String,
    pub stderr: String,
    pub out_dir: Option<String>,
    pub generated_files: Vec<crate::workspace::FileInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CodegenListResult {
    pub out_dir: String,
    pub files: Vec<crate::workspace::FileInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CodegenReadResult {
    pub out_dir: String,
    pub file: String,
    pub text: String,
}

pub fn codegen_build(project_root: &Path, manifest: Option<&str>) -> Result<CodegenBuildResult> {
    let manifest_path = manifest
        .map(|path| resolve_against(project_root, path))
        .unwrap_or_else(|| project_root.join("sandbox/Cargo.toml"));
    if !manifest_path.exists() {
        bail!("manifest not found: {}", manifest_path.display());
    }

    let output = Command::new("cargo")
        .arg("build")
        .arg("--manifest-path")
        .arg(&manifest_path)
        .current_dir(project_root)
        .output()
        .context("failed to execute cargo build")?;

    let success = output.status.success();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let (out_dir, generated_files) = if success {
        match find_out_dir(&manifest_path) {
            Ok(out_dir) => {
                let files = list_files_recursive(&out_dir)?;
                (Some(display_path(&out_dir)), files)
            }
            Err(_) => (None, vec![]),
        }
    } else {
        (None, vec![])
    };

    Ok(CodegenBuildResult {
        success,
        manifest_path: display_path(&manifest_path),
        stdout,
        stderr,
        out_dir,
        generated_files,
    })
}

pub fn codegen_list(project_root: &Path, manifest: Option<&str>) -> Result<CodegenListResult> {
    let manifest_path = default_manifest(project_root, manifest);
    let out_dir = find_out_dir(&manifest_path)?;
    Ok(CodegenListResult {
        out_dir: display_path(&out_dir),
        files: list_files_recursive(&out_dir)?,
    })
}

pub fn codegen_read(
    project_root: &Path,
    manifest: Option<&str>,
    file: &str,
    head: Option<usize>,
    tail: Option<usize>,
) -> Result<CodegenReadResult> {
    let manifest_path = default_manifest(project_root, manifest);
    let out_dir = find_out_dir(&manifest_path)?;
    let path = out_dir.join(file);
    ensure_inside(&out_dir, &path)?;
    if !path.exists() {
        bail!("generated file not found: {file}");
    }

    let text = fs::read_to_string(&path)
        .with_context(|| format!("failed to read generated file {}", path.display()))?;
    let text = slice_lines(&text, head, tail);

    Ok(CodegenReadResult {
        out_dir: display_path(&out_dir),
        file: file.to_string(),
        text,
    })
}

fn default_manifest(project_root: &Path, manifest: Option<&str>) -> PathBuf {
    manifest
        .map(|path| resolve_against(project_root, path))
        .unwrap_or_else(|| project_root.join("sandbox/Cargo.toml"))
}

fn ensure_inside(root: &Path, path: &Path) -> Result<()> {
    let root = root.canonicalize()?;
    let parent = path.parent().context("path has no parent")?;
    let parent = if parent.exists() {
        parent.canonicalize()?
    } else {
        root.clone()
    };
    if !parent.starts_with(root) {
        bail!("path traversal not allowed: {}", path.display());
    }
    Ok(())
}

fn slice_lines(text: &str, head: Option<usize>, tail: Option<usize>) -> String {
    let lines: Vec<&str> = text.lines().collect();
    if let Some(head) = head {
        lines.into_iter().take(head).collect::<Vec<_>>().join("\n")
    } else if let Some(tail) = tail {
        let start = lines.len().saturating_sub(tail);
        lines.into_iter().skip(start).collect::<Vec<_>>().join("\n")
    } else {
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slices_head_and_tail() {
        assert_eq!(slice_lines("a\nb\nc\n", Some(2), None), "a\nb");
        assert_eq!(slice_lines("a\nb\nc\n", None, Some(2)), "b\nc");
    }
}
