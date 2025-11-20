use walkdir::WalkDir;
use sha1::{Sha1, Digest};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};

pub fn calculate_sha1(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha1::new();
    let mut buffer = [0; 1024];

    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }

    let result = hasher.finalize();
    Ok(hex::encode(result))
}

// GitHub calculates blob SHA1 as: "blob <size>\0<content>"
// So we need to replicate this if we want to match GitHub's SHA.
// Wait, the user said "compare with github repo".
// GitHub's tree API returns the SHA of the blob.
// The blob SHA is `sha1("blob " + filesize + "\0" + content)`.
// So we must implement this specific hash calculation.

pub fn calculate_github_sha1(path: &Path) -> Result<String> {
    let metadata = fs::metadata(path)?;
    let size = metadata.len();
    let mut file = File::open(path)?;
    let mut hasher = Sha1::new();

    let header = format!("blob {}\0", size);
    hasher.update(header.as_bytes());

    let mut buffer = [0; 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }

    let result = hasher.finalize();
    Ok(hex::encode(result))
}

pub fn scan_local_files(root: &Path) -> Result<Vec<(String, String)>> {
    let mut files = Vec::new();
    for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let path = entry.path();
            // Get path relative to root
            let relative_path = path.strip_prefix(root)?.to_string_lossy().replace("\\", "/");
            // Skip hidden files like .git
            if relative_path.starts_with(".") {
                continue;
            }
            let sha = calculate_github_sha1(path).context(format!("Failed to hash {:?}", path))?;
            files.push((relative_path, sha));
        }
    }
    Ok(files)
}
