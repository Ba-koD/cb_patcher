use crate::github::GitHubClient;
use std::path::PathBuf;
use std::fs;
use std::io::{Cursor, Read};
use std::collections::HashSet;
use anyhow::Result;
use zip::ZipArchive;

pub struct Patcher {
    client: GitHubClient,
    mod_path: PathBuf,
}

impl Patcher {
    pub fn new(client: GitHubClient, mod_path: PathBuf) -> Self {
        Self {
            client,
            mod_path,
        }
    }

    pub fn sync<F>(&self, branch: &str, logger: Option<F>) -> Result<()> 
    where F: Fn(String) {
        let log = |msg: String| {
            if let Some(f) = &logger {
                f(msg.clone());
            }
            println!("{}", msg);
        };

        log("Downloading repository archive...".to_string());
        // This consumes only 1 API request (or minimal)
        let zip_data = self.client.download_repo_zip(branch)?;
        
        log("Extracting and comparing...".to_string());
        let cursor = Cursor::new(zip_data);
        let mut archive = ZipArchive::new(cursor)?;

        // Find the root folder name in the zip (e.g. "Ba-koD-conch_blessing-123456/")
        // It's usually the first entry's top-level directory.
        let root_name = {
            let mut name = String::new();
            if archive.len() > 0 {
                let file = archive.by_index(0)?;
                let path = file.name();
                if let Some(idx) = path.find('/') {
                    name = path[..idx+1].to_string();
                }
            }
            name
        };

        let mut processed_files = HashSet::new();

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let file_name = file.name().to_string(); // Full path in zip

            // Skip directories and files outside root (shouldn't happen)
            if file.is_dir() || !file_name.starts_with(&root_name) {
                continue;
            }

            // Strip root folder
            let relative_path = &file_name[root_name.len()..];
            if relative_path.is_empty() {
                continue;
            }

            // Skip .git files if any (zipball usually excludes .git dir but might include .gitignore)
            if relative_path.starts_with(".git") {
                continue;
            }

            let target_path = self.mod_path.join(relative_path);
            processed_files.insert(target_path.clone());

            // Create parent dirs
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)?;
            }

            // Read content
            let mut content = Vec::new();
            file.read_to_end(&mut content)?;

            // Compare and write if different
            let mut is_different = true;
            if target_path.exists() {
                if let Ok(local_content) = fs::read(&target_path) {
                    if local_content == content {
                        is_different = false;
                    }
                }
            }

            if is_different {
                if target_path.exists() {
                    log(format!("Updated: {}", relative_path));
                } else {
                    log(format!("New: {}", relative_path));
                }
                fs::write(&target_path, content)?;
            }
        }

        // Delete removed files
        log("Cleaning up old files...".to_string());
        // Walk dir and delete files not in processed_files
        for entry in walkdir::WalkDir::new(&self.mod_path).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                let path = entry.path().to_path_buf();
                
                // If the file is NOT in the new zip, delete it.
                // But we must be careful not to delete user config files if they exist.
                // For this mod patcher, we assume full sync.
                
                if !processed_files.contains(&path) {
                    // Check if it's a file we should ignore?
                    // e.g., ".DS_Store" or "Thumbs.db"
                    let file_name = path.file_name().unwrap_or_default().to_string_lossy();
                    if file_name == ".DS_Store" || file_name == "Thumbs.db" {
                        continue;
                    }

                    if let Ok(rel) = path.strip_prefix(&self.mod_path) {
                         log(format!("Deleted: {}", rel.display()));
                    }
                    let _ = fs::remove_file(path);
                }
            }
        }
        
        // Clean empty directories (Optional, skipping for simplicity)

        log("Update complete!".to_string());
        Ok(())
    }
}
