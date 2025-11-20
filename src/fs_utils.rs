use walkdir::WalkDir;
use sha1::{Sha1, Digest};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use directories::UserDirs;

#[cfg(target_os = "windows")]
pub fn find_steam_path_from_registry() -> Option<PathBuf> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let steam = hkcu.open_subkey("Software\\Valve\\Steam").ok()?;
    let path_str: String = steam.get_value("SteamPath").ok()?;
    
    Some(PathBuf::from(path_str))
}

pub fn find_steam_from_path_env() -> Option<PathBuf> {
    if let Some(paths) = std::env::var_os("PATH") {
        for path in std::env::split_paths(&paths) {
            // Check for steam.exe (Windows) or steam (Unix)
            let steam_exe = path.join("steam.exe");
            if steam_exe.exists() {
                return Some(path);
            }
        }
    }
    None
}

pub fn find_isaac_game_path() -> Option<PathBuf> {
    // 1. Try Windows Registry (Windows only)
    #[cfg(target_os = "windows")]
    {
        if let Some(steam_path) = find_steam_path_from_registry() {
            let game_path = steam_path.join("steamapps/common/The Binding of Isaac Rebirth");
            if game_path.join("isaac-ng.exe").exists() {
                return Some(game_path);
            }
        }
    }

    // 2. Try PATH environment variable
    if let Some(steam_path) = find_steam_from_path_env() {
        let game_path = steam_path.join("steamapps/common/The Binding of Isaac Rebirth");
        if game_path.exists() { // Weak check if exe not visible in PATH lookup context
             return Some(game_path);
        }
    }

    // 3. Fallback to common Steam paths
    let common_steam_paths = [
        r"C:\Program Files (x86)\Steam",
        r"C:\Steam",
        r"D:\Steam",
        r"E:\Steam",
        // Common library paths
        r"C:\SteamLibrary",
        r"D:\SteamLibrary",
        r"E:\SteamLibrary",
    ];

    for p in common_steam_paths {
        let base_path = if p.starts_with("~") {
            if let Some(user_dirs) = UserDirs::new() {
                let home = user_dirs.home_dir();
                let suffix = &p[2..];
                home.join(suffix)
            } else {
                PathBuf::from(p)
            }
        } else {
            PathBuf::from(p)
        };

        if base_path.exists() {
            let game_path = base_path.join("steamapps/common/The Binding of Isaac Rebirth");
            // Check for game executable
            let exe_name = if cfg!(target_os = "windows") { "isaac-ng.exe" } else { "isaac-ng" }; 
            // Note: Mac might be different (Isaac-ng), Linux (isaac-ng).
            
            if game_path.join(exe_name).exists() || game_path.exists() {
                 return Some(game_path);
            }
        }
    }

    // 3. Check specific Mac save data path (standard location for mods on Mac, but game is elsewhere)
    // Skipping Mac specific game path detection for now as user emphasized Windows.
    
    None
}

#[allow(dead_code)]
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

#[allow(dead_code)]
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
