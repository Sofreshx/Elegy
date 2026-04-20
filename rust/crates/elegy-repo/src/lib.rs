use serde::Serialize;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;

const MAX_LOG_COUNT: u32 = 100;
const LOG_FIELD_SEPARATOR: char = '\u{1f}';

#[derive(Debug, Error)]
pub enum RepoError {
    #[error("failed to determine the current directory: {source}")]
    CurrentDirectory {
        #[source]
        source: io::Error,
    },
    #[error("git executable was not found")]
    GitNotInstalled,
    #[error("path is not a git repository: {path}")]
    NotGitRepository { path: String },
    #[error("git command failed: git {args}: {stderr}")]
    GitCommandFailed {
        args: String,
        stderr: String,
        exit_code: Option<i32>,
    },
    #[error("failed to launch git: {source}")]
    Io {
        #[source]
        source: io::Error,
    },
}

#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RepoStatus {
    pub repo_root: String,
    pub current_branch: Option<String>,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub entries: Vec<RepoStatusEntry>,
    pub staged_count: u32,
    pub unstaged_count: u32,
    pub untracked_count: u32,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RepoStatusEntry {
    pub path: String,
    pub index_status: String,
    pub worktree_status: String,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RepoDiffSummary {
    pub base_ref: Option<String>,
    pub compared_against: String,
    pub files_changed: u32,
    pub insertions: u32,
    pub deletions: u32,
    pub files: Vec<RepoDiffFile>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RepoDiffFile {
    pub path: String,
    pub insertions: u32,
    pub deletions: u32,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RepoBranches {
    pub current_branch: Option<String>,
    pub branches: Vec<RepoBranch>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RepoBranch {
    pub name: String,
    pub is_current: bool,
    pub upstream: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RepoLog {
    pub commits: Vec<RepoCommit>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RepoCommit {
    pub short_hash: String,
    pub hash: String,
    pub author_name: String,
    pub author_email: String,
    pub subject: String,
    pub committed_at_utc: String,
}

pub fn status(repo: Option<&Path>) -> Result<RepoStatus, RepoError> {
    let repo_root = resolve_repo_root(repo)?;
    let output = run_git(
        Some(repo_root.as_path()),
        &["status", "--porcelain=1", "--branch"],
    )?;

    let mut lines = output.lines();
    let header = parse_status_header(lines.next().unwrap_or_default());
    let mut entries = Vec::new();
    let mut staged_count = 0_u32;
    let mut unstaged_count = 0_u32;
    let mut untracked_count = 0_u32;

    for line in lines {
        if line.len() < 3 {
            continue;
        }

        let mut chars = line.chars();
        let index_status = chars.next().unwrap_or(' ');
        let worktree_status = chars.next().unwrap_or(' ');
        if chars.next() != Some(' ') {
            continue;
        }

        let raw_path = &line[3..];
        let path = parse_status_path(raw_path, index_status, worktree_status);

        if index_status == '?' && worktree_status == '?' {
            untracked_count += 1;
        } else {
            if index_status != ' ' {
                staged_count += 1;
            }
            if worktree_status != ' ' {
                unstaged_count += 1;
            }
        }

        entries.push(RepoStatusEntry {
            path,
            index_status: index_status.to_string(),
            worktree_status: worktree_status.to_string(),
        });
    }

    Ok(RepoStatus {
        repo_root: repo_root.display().to_string(),
        current_branch: header.current_branch,
        upstream: header.upstream,
        ahead: header.ahead,
        behind: header.behind,
        entries,
        staged_count,
        unstaged_count,
        untracked_count,
    })
}

pub fn diff_summary(repo: Option<&Path>, base: Option<&str>) -> Result<RepoDiffSummary, RepoError> {
    let repo_root = resolve_repo_root(repo)?;
    let base_ref = base.and_then(non_empty_trimmed);

    let compared_against = match base_ref {
        Some(base_ref) => format!("{base_ref}...HEAD"),
        None => "HEAD".to_string(),
    };

    let numstat_target;
    let args: Vec<&str> = if let Some(base_ref) = base_ref {
        numstat_target = format!("{base_ref}...HEAD");
        vec!["diff", "--numstat", numstat_target.as_str()]
    } else {
        vec!["diff", "--numstat", "HEAD"]
    };

    let output = run_git(Some(repo_root.as_path()), &args)?;
    let mut files = Vec::new();
    let mut insertions = 0_u32;
    let mut deletions = 0_u32;

    for line in output.lines() {
        if line.trim().is_empty() {
            continue;
        }

        let mut parts = line.splitn(3, '\t');
        let file_insertions = parse_numstat_count(parts.next().unwrap_or_default());
        let file_deletions = parse_numstat_count(parts.next().unwrap_or_default());
        let path = parts.next().unwrap_or_default().trim().to_string();

        if path.is_empty() {
            continue;
        }

        insertions = insertions.saturating_add(file_insertions);
        deletions = deletions.saturating_add(file_deletions);
        files.push(RepoDiffFile {
            path,
            insertions: file_insertions,
            deletions: file_deletions,
        });
    }

    Ok(RepoDiffSummary {
        base_ref: base_ref.map(ToOwned::to_owned),
        compared_against,
        files_changed: u32::try_from(files.len()).unwrap_or(u32::MAX),
        insertions,
        deletions,
        files,
    })
}

pub fn branches(repo: Option<&Path>) -> Result<RepoBranches, RepoError> {
    let repo_root = resolve_repo_root(repo)?;
    let output = run_git(
        Some(repo_root.as_path()),
        &[
            "branch",
            "--format=%(HEAD)|%(refname:short)|%(upstream:short)",
        ],
    )?;

    let mut current_branch = None;
    let mut parsed_branches = Vec::new();

    for line in output.lines() {
        if line.trim().is_empty() {
            continue;
        }

        let mut parts = line.splitn(3, '|');
        let head_marker = parts.next().unwrap_or_default().trim();
        let name = parts.next().unwrap_or_default().trim();
        let upstream = parts.next().unwrap_or_default().trim();

        if name.is_empty() {
            continue;
        }

        let is_current = head_marker == "*";
        if is_current {
            current_branch = Some(name.to_string());
        }

        parsed_branches.push(RepoBranch {
            name: name.to_string(),
            is_current,
            upstream: non_empty_trimmed(upstream).map(ToOwned::to_owned),
        });
    }

    Ok(RepoBranches {
        current_branch,
        branches: parsed_branches,
    })
}

pub fn log(repo: Option<&Path>, count: u32) -> Result<RepoLog, RepoError> {
    let repo_root = resolve_repo_root(repo)?;
    if count == 0 {
        return Ok(RepoLog {
            commits: Vec::new(),
        });
    }

    let bounded_count = count.min(MAX_LOG_COUNT);
    let count_arg = bounded_count.to_string();
    let output = run_git(
        Some(repo_root.as_path()),
        &[
            "log",
            "-n",
            count_arg.as_str(),
            "--date=iso-strict",
            "--pretty=format:%H%x1f%h%x1f%an%x1f%ae%x1f%ad%x1f%s",
        ],
    )?;

    let mut commits = Vec::new();
    for line in output.lines() {
        if line.trim().is_empty() {
            continue;
        }

        let mut fields = line.split(LOG_FIELD_SEPARATOR);
        let hash = fields.next().unwrap_or_default().trim();
        let short_hash = fields.next().unwrap_or_default().trim();
        let author_name = fields.next().unwrap_or_default().trim();
        let author_email = fields.next().unwrap_or_default().trim();
        let committed_at_utc = fields.next().unwrap_or_default().trim();
        let subject = fields.next().unwrap_or_default().trim();

        if hash.is_empty() {
            continue;
        }

        commits.push(RepoCommit {
            short_hash: short_hash.to_string(),
            hash: hash.to_string(),
            author_name: author_name.to_string(),
            author_email: author_email.to_string(),
            subject: subject.to_string(),
            committed_at_utc: committed_at_utc.to_string(),
        });
    }

    Ok(RepoLog { commits })
}

fn resolve_repo_root(repo: Option<&Path>) -> Result<PathBuf, RepoError> {
    let output = run_git(repo, &["rev-parse", "--show-toplevel"])?;
    let repo_root = output.lines().next().unwrap_or_default().trim();
    if repo_root.is_empty() {
        return Err(RepoError::GitCommandFailed {
            args: "rev-parse --show-toplevel".to_string(),
            stderr: "git did not report a repository root".to_string(),
            exit_code: None,
        });
    }

    Ok(PathBuf::from(repo_root))
}

fn run_git(repo: Option<&Path>, args: &[&str]) -> Result<String, RepoError> {
    let mut command = Command::new("git");
    command.args(args);
    if let Some(repo) = repo {
        command.current_dir(repo);
    }

    let output = command.output().map_err(|source| {
        if source.kind() == io::ErrorKind::NotFound {
            RepoError::GitNotInstalled
        } else {
            RepoError::Io { source }
        }
    })?;

    if output.status.success() {
        return Ok(normalize_newlines(&String::from_utf8_lossy(&output.stdout)));
    }

    let stderr = normalize_newlines(&String::from_utf8_lossy(&output.stderr));
    let stdout = normalize_newlines(&String::from_utf8_lossy(&output.stdout));
    let message = if stderr.trim().is_empty() {
        stdout.trim().to_string()
    } else {
        stderr.trim().to_string()
    };

    if is_not_git_repository(&message) {
        return Err(RepoError::NotGitRepository {
            path: repo_context_path(repo)?.display().to_string(),
        });
    }

    Err(RepoError::GitCommandFailed {
        args: args.join(" "),
        stderr: message,
        exit_code: output.status.code(),
    })
}

fn repo_context_path(repo: Option<&Path>) -> Result<PathBuf, RepoError> {
    match repo {
        Some(repo) => Ok(repo.to_path_buf()),
        None => std::env::current_dir().map_err(|source| RepoError::CurrentDirectory { source }),
    }
}

fn normalize_newlines(input: &str) -> String {
    input.replace("\r\n", "\n")
}

fn parse_numstat_count(value: &str) -> u32 {
    if value.trim() == "-" {
        0
    } else {
        value.trim().parse::<u32>().ok().unwrap_or(0)
    }
}

fn parse_status_path(raw_path: &str, index_status: char, worktree_status: char) -> String {
    let trimmed = raw_path.trim();
    let is_rename_like = matches!(index_status, 'R' | 'C') || matches!(worktree_status, 'R' | 'C');
    if is_rename_like && trimmed.contains(" -> ") {
        return trimmed
            .rsplit_once(" -> ")
            .map(|(_, path)| path.to_string())
            .unwrap_or_else(|| trimmed.to_string());
    }

    trimmed.to_string()
}

fn is_not_git_repository(message: &str) -> bool {
    let lowered = message.to_ascii_lowercase();
    lowered.contains("not a git repository")
        || lowered.contains("outside repository")
        || lowered.contains("must be run in a work tree")
}

fn non_empty_trimmed(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

#[derive(Default)]
struct ParsedStatusHeader {
    current_branch: Option<String>,
    upstream: Option<String>,
    ahead: u32,
    behind: u32,
}

fn parse_status_header(line: &str) -> ParsedStatusHeader {
    let content = line.strip_prefix("## ").unwrap_or(line).trim();
    if content.is_empty() || content == "HEAD (no branch)" || content.starts_with("HEAD (") {
        return ParsedStatusHeader::default();
    }

    let content = content
        .strip_prefix("No commits yet on ")
        .unwrap_or(content)
        .trim();

    let mut parsed = ParsedStatusHeader::default();
    if let Some((branch_name, rest)) = content.split_once("...") {
        parsed.current_branch = non_empty_trimmed(branch_name).map(ToOwned::to_owned);
        let (upstream, relation) = split_upstream_and_relation(rest);
        parsed.upstream = non_empty_trimmed(upstream).map(ToOwned::to_owned);
        parse_branch_relation(relation, &mut parsed);
        return parsed;
    }

    parsed.current_branch = non_empty_trimmed(content).map(ToOwned::to_owned);
    parsed
}

fn split_upstream_and_relation(rest: &str) -> (&str, Option<&str>) {
    if let Some((upstream, relation)) = rest.split_once(" [") {
        (upstream.trim(), Some(relation.trim_end_matches(']').trim()))
    } else {
        (rest.trim(), None)
    }
}

fn parse_branch_relation(relation: Option<&str>, parsed: &mut ParsedStatusHeader) {
    let Some(relation) = relation else {
        return;
    };

    for token in relation.split(',') {
        let token = token.trim();
        if let Some(value) = token.strip_prefix("ahead ") {
            parsed.ahead = value.trim().parse::<u32>().ok().unwrap_or(0);
        } else if let Some(value) = token.strip_prefix("behind ") {
            parsed.behind = value.trim().parse::<u32>().ok().unwrap_or(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{branches, diff_summary, log, status, RepoError};
    use std::error::Error;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestRepo {
        root: PathBuf,
        remote: PathBuf,
        current_branch: String,
        initial_head: String,
    }

    impl TestRepo {
        fn new() -> Result<Self, Box<dyn Error>> {
            let base = unique_temp_path("elegy-repo-tests");
            let remote = unique_temp_path("elegy-repo-remote");
            fs::create_dir_all(&base)?;
            run_git(None, &["init", base.to_string_lossy().as_ref()])?;
            run_git(
                Some(base.as_path()),
                &["config", "user.name", "Elegy Tests"],
            )?;
            run_git(
                Some(base.as_path()),
                &["config", "user.email", "elegy-tests@example.invalid"],
            )?;

            fs::write(base.join("tracked.txt"), "line one\n")?;
            run_git(Some(base.as_path()), &["add", "tracked.txt"])?;
            run_git(Some(base.as_path()), &["commit", "-m", "initial commit"])?;

            let current_branch = run_git(Some(base.as_path()), &["branch", "--show-current"])?
                .trim()
                .to_string();
            let initial_head = run_git(Some(base.as_path()), &["rev-parse", "HEAD"])?
                .trim()
                .to_string();

            run_git(None, &["init", "--bare", remote.to_string_lossy().as_ref()])?;
            run_git(
                Some(base.as_path()),
                &["remote", "add", "origin", remote.to_string_lossy().as_ref()],
            )?;
            run_git(
                Some(base.as_path()),
                &["push", "-u", "origin", current_branch.as_str()],
            )?;

            Ok(Self {
                root: base,
                remote,
                current_branch,
                initial_head,
            })
        }

        fn path(&self) -> &Path {
            self.root.as_path()
        }

        fn current_branch(&self) -> &str {
            self.current_branch.as_str()
        }

        fn initial_head(&self) -> &str {
            self.initial_head.as_str()
        }

        fn write_file(&self, relative_path: &str, contents: &str) -> Result<(), Box<dyn Error>> {
            fs::write(self.root.join(relative_path), contents)?;
            Ok(())
        }

        fn git(&self, args: &[&str]) -> Result<String, Box<dyn Error>> {
            run_git(Some(self.root.as_path()), args)
        }
    }

    impl Drop for TestRepo {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
            let _ = fs::remove_dir_all(&self.remote);
        }
    }

    #[test]
    fn status_reports_branch_counts_and_entries() -> Result<(), Box<dyn Error>> {
        let repo = TestRepo::new()?;
        repo.write_file("tracked.txt", "line one\nline two\n")?;
        repo.git(&["add", "tracked.txt"])?;
        repo.git(&["commit", "-m", "ahead commit"])?;

        repo.write_file("tracked.txt", "line one\nline two\nline three\n")?;
        repo.git(&["add", "tracked.txt"])?;
        repo.write_file("tracked.txt", "line one\nline two\nline three\nline four\n")?;
        repo.write_file("untracked.txt", "surprise\n")?;

        let repo_status = status(Some(repo.path()))?;

        assert_eq!(
            repo_status.current_branch.as_deref(),
            Some(repo.current_branch())
        );
        assert_eq!(
            repo_status.upstream.as_deref(),
            Some(format!("origin/{}", repo.current_branch()).as_str())
        );
        assert_eq!(repo_status.ahead, 1);
        assert_eq!(repo_status.behind, 0);
        assert_eq!(repo_status.staged_count, 1);
        assert_eq!(repo_status.unstaged_count, 1);
        assert_eq!(repo_status.untracked_count, 1);
        assert!(repo_status
            .entries
            .iter()
            .any(|entry| entry.path == "tracked.txt"
                && entry.index_status == "M"
                && entry.worktree_status == "M"));
        assert!(repo_status
            .entries
            .iter()
            .any(|entry| entry.path == "untracked.txt"
                && entry.index_status == "?"
                && entry.worktree_status == "?"));

        Ok(())
    }

    #[test]
    fn diff_summary_supports_explicit_base_refs() -> Result<(), Box<dyn Error>> {
        let repo = TestRepo::new()?;
        repo.write_file("tracked.txt", "line one\nline two\n")?;
        repo.write_file("added.txt", "brand new\n")?;
        repo.git(&["add", "tracked.txt", "added.txt"])?;
        repo.git(&["commit", "-m", "feature commit"])?;

        let summary = diff_summary(Some(repo.path()), Some(repo.initial_head()))?;

        assert_eq!(summary.base_ref.as_deref(), Some(repo.initial_head()));
        assert_eq!(
            summary.compared_against,
            format!("{}...HEAD", repo.initial_head())
        );
        assert_eq!(summary.files_changed, 2);
        assert!(summary.insertions >= 2);
        assert!(summary.files.iter().any(|file| file.path == "tracked.txt"));
        assert!(summary.files.iter().any(|file| file.path == "added.txt"));

        Ok(())
    }

    #[test]
    fn branches_and_log_report_structured_history() -> Result<(), Box<dyn Error>> {
        let repo = TestRepo::new()?;
        repo.write_file("tracked.txt", "line one\nline two\n")?;
        repo.git(&["add", "tracked.txt"])?;
        repo.git(&["commit", "-m", "second commit"])?;
        repo.git(&["branch", "feature/test"])?;

        let repo_branches = branches(Some(repo.path()))?;
        assert_eq!(
            repo_branches.current_branch.as_deref(),
            Some(repo.current_branch())
        );
        assert!(repo_branches
            .branches
            .iter()
            .any(|branch| branch.name == repo.current_branch() && branch.is_current));
        assert!(repo_branches
            .branches
            .iter()
            .any(|branch| branch.name == "feature/test" && !branch.is_current));

        let repo_log = log(Some(repo.path()), 200)?;
        assert_eq!(repo_log.commits.len(), 2);
        assert_eq!(repo_log.commits[0].subject, "second commit");
        assert_eq!(repo_log.commits[1].subject, "initial commit");
        assert!(!repo_log.commits[0].hash.is_empty());
        assert!(!repo_log.commits[0].short_hash.is_empty());
        assert!(!repo_log.commits[0].committed_at_utc.is_empty());

        Ok(())
    }

    #[test]
    fn non_git_paths_return_typed_errors() -> Result<(), Box<dyn Error>> {
        let path = unique_temp_path("elegy-repo-non-git");
        fs::create_dir_all(&path)?;

        let result = status(Some(path.as_path()));
        assert!(matches!(result, Err(RepoError::NotGitRepository { .. })));

        let _ = fs::remove_dir_all(path);
        Ok(())
    }

    fn unique_temp_path(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()))
    }

    fn run_git(repo: Option<&Path>, args: &[&str]) -> Result<String, Box<dyn Error>> {
        let mut command = Command::new("git");
        command.args(args);
        if let Some(repo) = repo {
            command.current_dir(repo);
        }

        let output = command.output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).replace("\r\n", "\n");
            let stdout = String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n");
            let message = if stderr.trim().is_empty() {
                stdout
            } else {
                stderr
            };

            return Err(format!(
                "git {} failed with {:?}: {}",
                args.join(" "),
                output.status.code(),
                message.trim()
            )
            .into());
        }

        Ok(String::from_utf8_lossy(&output.stdout)
            .replace("\r\n", "\n")
            .trim()
            .to_string())
    }
}
