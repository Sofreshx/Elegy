//! Shared by the build script and qualification reader; no timestamps or paths
//! from the local machine enter the digest. Conservative shared-source coverage.
use std::{fs, io, path::Path};

use sha2::{Digest, Sha256};

pub const INPUTS: &[&str] = &[
    "Cargo.toml",
    "Cargo.lock",
    "plugins/memory/Cargo.toml",
    "plugins/memory/build.rs",
    "plugins/memory/src",
    "plugins/memory/tests",
    "plugins/memory/fixtures",
    "plugins/memory/schemas",
    "plugins/memory/integrations/codex",
    "plugins/memory/eval-harness-v1.json",
    "shared",
    "docs/specs/eval-harness-v1/spec.md",
    "docs/specs/memory-contextual-recall-v1/spec.md",
];

fn collect(root: &Path, path: &Path, files: &mut Vec<String>) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    #[cfg(windows)]
    let is_link = {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    };
    #[cfg(not(windows))]
    let is_link = metadata.file_type().is_symlink();
    if is_link {
        return Err(io::Error::other(
            "qualification source contains a link or reparse point",
        ));
    }
    if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let name = entry.file_name();
            if matches!(
                name.to_str(),
                Some("target" | ".git" | "__pycache__" | "node_modules")
            ) {
                continue;
            }
            collect(root, &entry.path(), files)?;
        }
    } else if metadata.is_file()
        && matches!(
            path.extension().and_then(|s| s.to_str()),
            Some("rs" | "toml" | "lock" | "json" | "sql" | "py" | "md")
        )
    {
        if metadata.len() > 8 * 1024 * 1024 || files.len() >= 50_000 {
            return Err(io::Error::other("qualification source exceeds bounds"));
        }
        files.push(
            path.strip_prefix(root)
                .map_err(|_| io::Error::other("source outside project"))?
                .to_string_lossy()
                .replace('\\', "/"),
        );
    }
    Ok(())
}

pub fn source_digest(root: &Path) -> io::Result<String> {
    let mut files = Vec::new();
    for input in INPUTS {
        collect(root, &root.join(input), &mut files)?;
    }
    files.sort();
    files.dedup();
    let mut hasher = Sha256::new();
    for file in files {
        let content = fs::read(root.join(&file))?;
        hasher.update((file.len() as u64).to_le_bytes());
        hasher.update(file.as_bytes());
        hasher.update((content.len() as u64).to_le_bytes());
        hasher.update(content);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source_fixture(root: &Path) {
        for input in INPUTS {
            let path = root.join(input);
            if path.extension().is_some() {
                fs::create_dir_all(path.parent().expect("parent")).expect("directories");
                fs::write(path, b"fixture").expect("file");
            } else {
                fs::create_dir_all(path).expect("directory");
            }
        }
        fs::write(root.join("plugins/memory/src/lib.rs"), b"fn first() {}\n").expect("source");
    }

    #[test]
    fn relevant_edits_and_new_sources_invalidate_but_receipts_do_not() {
        let dir = tempfile::tempdir().expect("fixture");
        source_fixture(dir.path());
        let before = source_digest(dir.path()).expect("digest");
        let evidence = dir.path().join("plugins/memory/evidence");
        fs::create_dir_all(&evidence).expect("evidence");
        fs::write(evidence.join("receipt.json"), "{}").expect("receipt");
        assert_eq!(before, source_digest(dir.path()).expect("digest"));
        fs::write(
            dir.path().join("plugins/memory/src/lib.rs"),
            b"fn second() {}\n",
        )
        .expect("edit");
        let edited = source_digest(dir.path()).expect("digest");
        assert_ne!(before, edited);
        fs::write(dir.path().join("plugins/memory/src/new.rs"), b"// new").expect("new source");
        assert_ne!(edited, source_digest(dir.path()).expect("digest"));
    }

    #[test]
    fn relocation_is_stable_and_missing_required_input_is_an_error() {
        let a = tempfile::tempdir().expect("fixture");
        let b = tempfile::tempdir().expect("fixture");
        source_fixture(a.path());
        source_fixture(b.path());
        assert_eq!(
            source_digest(a.path()).expect("a"),
            source_digest(b.path()).expect("b")
        );
        fs::remove_file(a.path().join("Cargo.lock")).expect("remove fixture file");
        assert!(source_digest(a.path()).is_err());
    }
}
