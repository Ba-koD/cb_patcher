mod github;
mod fs_utils;
mod patcher;

use clap::Parser;
use std::path::PathBuf;
use std::io::{self, Write};
use directories::UserDirs;
use anyhow::Result;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the game or mods folder. If not provided, attempts auto-detection.
    #[arg(short, long)]
    path: Option<PathBuf>,

    /// GitHub branch to sync with
    #[arg(short, long, default_value = "main")]
    branch: String,
}

#[cfg(target_os = "windows")]
fn find_steam_path_from_registry() -> Option<PathBuf> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let steam = hkcu.open_subkey("Software\\Valve\\Steam").ok()?;
    let path_str: String = steam.get_value("SteamPath").ok()?;
    
    Some(PathBuf::from(path_str))
}

fn find_steam_from_path_env() -> Option<PathBuf> {
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

fn find_isaac_mods_path() -> Option<PathBuf> {
    // 1. Try Windows Registry (Windows only)
    #[cfg(target_os = "windows")]
    {
        if let Some(steam_path) = find_steam_path_from_registry() {
            let mods_path = steam_path.join("steamapps/common/The Binding of Isaac Rebirth/mods");
            if mods_path.exists() {
                return Some(mods_path);
            }
        }
    }

    // 2. Try PATH environment variable
    if let Some(steam_path) = find_steam_from_path_env() {
        let mods_path = steam_path.join("steamapps/common/The Binding of Isaac Rebirth/mods");
        if mods_path.exists() {
            return Some(mods_path);
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
            // Check for game path inside Steam/Library
            // Note: Mac path is slightly different for the game app itself, 
            // but usually mods are in "~/Library/Application Support/Binding of Isaac Rebirth/mods"
            // which is NOT inside Steam apps usually on Mac (it's in the save data folder).
            // But for Windows/Linux structure:
            let mods_path = base_path.join("steamapps/common/The Binding of Isaac Rebirth/mods");
            if mods_path.exists() {
                return Some(mods_path);
            }
        }
    }

    // 3. Check specific Mac save data path (standard location for mods on Mac)
    if let Some(user_dirs) = UserDirs::new() {
        let mac_mods = user_dirs.home_dir().join("Library/Application Support/Binding of Isaac Rebirth/mods");
        if mac_mods.exists() {
            return Some(mac_mods);
        }
    }

    None
}

fn main() -> Result<()> {
    let args = Args::parse();

    let mod_path = if let Some(p) = args.path {
        p
    } else {
        println!("Attempting to find Isaac mods folder...");
        if let Some(p) = find_isaac_mods_path() {
            println!("Found mods folder at: {:?}", p);
            p
        } else {
            // Ask user
            print!("Could not find mods folder automatically. Please enter the path to the 'mods' folder: ");
            io::stdout().flush()?;
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            PathBuf::from(input.trim())
        }
    };

    // Target specific mod folder
    let target_mod_path = mod_path.join("conch_blessing");
    
    // If it doesn't exist, create it (fresh install)
    if !target_mod_path.exists() {
        println!("Mod folder not found. Creating: {:?}", target_mod_path);
        std::fs::create_dir_all(&target_mod_path)?;
    }

    let client = github::GitHubClient::new("Ba-koD", "conch_blessing");
    let patcher = patcher::Patcher::new(client, target_mod_path);

    patcher.sync(&args.branch)?;

    println!("Press Enter to exit...");
    let mut _s = String::new();
    io::stdin().read_line(&mut _s)?;

    Ok(())
}
