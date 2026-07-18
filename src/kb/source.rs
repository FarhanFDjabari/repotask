//! Resolve the knowledge base to a directory on disk.
//!
//! Remote git repositories are cloned into `~/.repo-task/kb/<slug>` and pinned to a
//! ref; a local directory is used in place. Remote wins when both are configured.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Serialize;

use crate::config::{KnowledgeConfig, RepoTaskConfig};
use crate::git;

pub const CACHE_ENV: &str = "REPOTASK_CACHE_DIR";

pub fn cache_root() -> PathBuf {
    if let Ok(override_path) = std::env::var(CACHE_ENV) {
        return PathBuf::from(override_path);
    }
    home_dir().join(".repo-task/kb")
}

fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

/// Readable directory name plus a hash, so similar remotes never collide.
pub fn slug_for(remote: &str) -> String {
    let tail: String = remote
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("kb")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let tail = tail.trim_matches('-').to_string();
    let tail = if tail.is_empty() {
        "kb".to_string()
    } else {
        tail
    };
    format!("{tail}-{}", short_hash(remote))
}

/// FNV-1a, truncated. Only needs to avoid collisions between cache directories.
fn short_hash(value: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in value.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    format!("{hash:016x}")[..10].to_string()
}

#[derive(Debug, Clone, Serialize)]
pub struct KnowledgeSource {
    pub path: PathBuf,
    pub kind: String,
    pub remote: String,
    pub r#ref: String,
    pub revision: String,
    pub synced_at: String,
}

pub fn resolve(config: &RepoTaskConfig, sync: Option<bool>) -> Result<KnowledgeSource> {
    let knowledge = &config.knowledge;
    if !knowledge.remote.is_empty() {
        let should_sync = sync.unwrap_or(knowledge.auto_sync);
        return resolve_remote(knowledge, should_sync);
    }
    let local = config.root.join(&knowledge.local);
    if !local.is_dir() {
        bail!(
            "Knowledge base not found at {}. Configure `knowledge.remote` or create the \
             directory, then run `repo-task kb sync`.",
            local.display()
        );
    }
    Ok(KnowledgeSource {
        path: local,
        kind: "local".into(),
        remote: String::new(),
        r#ref: String::new(),
        revision: String::new(),
        synced_at: String::new(),
    })
}

fn resolve_remote(knowledge: &KnowledgeConfig, should_sync: bool) -> Result<KnowledgeSource> {
    let path = cache_root().join(slug_for(&knowledge.remote));
    if !path.is_dir() {
        if !should_sync {
            bail!(
                "Knowledge base has never been synced to {}. Run `repo-task kb sync`.",
                path.display()
            );
        }
        clone(&knowledge.remote, &path)?;
    } else if should_sync {
        git::run(&["fetch", "--quiet", "--prune", "origin"], Some(&path))
            .context("Could not fetch knowledge base updates")?;
    }
    checkout(&path, &knowledge.r#ref)?;
    let revision = git::run(&["rev-parse", "HEAD"], Some(&path))?
        .trim()
        .to_string();
    Ok(KnowledgeSource {
        path: path.clone(),
        kind: "remote".into(),
        remote: knowledge.remote.clone(),
        r#ref: knowledge.r#ref.clone(),
        revision,
        synced_at: stamp(&path, should_sync),
    })
}

fn clone(remote: &str, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    git::run(
        &["clone", "--quiet", "--", remote, &path.to_string_lossy()],
        None,
    )
    .with_context(|| format!("Could not clone knowledge base {remote}"))?;
    Ok(())
}

/// Pin the worktree to `ref`, preferring the remote-tracking branch when one exists.
fn checkout(path: &Path, reference: &str) -> Result<()> {
    if reference.starts_with('-') {
        bail!("Invalid knowledge base ref: {reference}");
    }
    let remote_ref = format!("origin/{reference}");
    let target = if git::run_ok(&["rev-parse", "--verify", &remote_ref], Some(path)) {
        remote_ref
    } else {
        reference.to_string()
    };
    git::run(&["checkout", "--quiet", "--detach", &target], Some(path))
        .with_context(|| format!("Knowledge base ref not found: {reference}"))?;
    Ok(())
}

fn stamp(path: &Path, synced_now: bool) -> String {
    let Some(parent) = path.parent() else {
        return String::new();
    };
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let marker = parent.join(format!("{name}.synced"));
    if synced_now {
        let value = now_iso();
        let _ = std::fs::write(&marker, &value);
        return value;
    }
    std::fs::read_to_string(&marker)
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// UTC timestamp, second precision, without pulling in a date-time crate.
pub fn now_iso() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let (year, month, day, hour, minute, second) = civil_from_unix(seconds as i64);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}+00:00")
}

/// Howard Hinnant's days-from-civil algorithm, inverted.
fn civil_from_unix(seconds: i64) -> (i64, u32, u32, u32, u32, u32) {
    let days = seconds.div_euclid(86_400);
    let time = seconds.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = if month <= 2 { year + 1 } else { year };
    (
        year,
        month,
        day,
        (time / 3600) as u32,
        ((time % 3600) / 60) as u32,
        (time % 60) as u32,
    )
}
