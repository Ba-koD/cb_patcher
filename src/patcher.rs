use crate::github::{GitHubClient, TreeItem};
use crate::fs_utils::scan_local_files;
use std::path::PathBuf;
use std::fs;
use std::collections::HashMap;
use anyhow::Result;

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

        log("Fetching remote tree...".to_string());
        let remote_tree = self.client.fetch_tree(branch)?;
        
        // Filter remote tree to only include blobs (files)
        let remote_files: HashMap<String, &TreeItem> = remote_tree
            .iter()
            .filter(|item| item.item_type == "blob")
            .map(|item| (item.path.clone(), item))
            .collect();

        log("Scanning local files...".to_string());
        let local_files_list = scan_local_files(&self.mod_path)?;
        let local_files: HashMap<String, String> = local_files_list.into_iter().collect();

        let mut to_download = Vec::new();
        let mut to_delete = Vec::new();

        // Check for files to download (new or changed)
        for (path, item) in &remote_files {
            if let Some(local_sha) = local_files.get(path) {
                if local_sha != &item.sha {
                    log(format!("Changed: {}", path));
                    to_download.push(item);
                }
            } else {
                log(format!("New: {}", path));
                to_download.push(item);
            }
        }

        // Check for files to delete (present locally but not remotely)
        for (path, _) in &local_files {
            if !remote_files.contains_key(path) {
                log(format!("Extra: {}", path));
                to_delete.push(path);
            }
        }

        if to_download.is_empty() && to_delete.is_empty() {
            log("Already up to date.".to_string());
            return Ok(());
        }

        log(format!("Plan: {} to download, {} to delete.", to_download.len(), to_delete.len()));

        // Execute deletions
        for path in to_delete {
            let full_path = self.mod_path.join(path);
            if full_path.exists() {
                fs::remove_file(&full_path)?;
                log(format!("Deleted: {}", path));
            }
        }

        // Execute downloads
        for item in to_download {
            log(format!("Downloading: {}", item.path));
            let content = self.client.download_file(&item.url)?;
            let full_path = self.mod_path.join(&item.path);
            
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent)?;
            }
            
            fs::write(&full_path, content)?;
        }

        log("Update complete!".to_string());
        Ok(())
    }
}
