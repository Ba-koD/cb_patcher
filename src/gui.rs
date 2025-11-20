use eframe::egui;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use crate::github::GitHubClient;
use crate::patcher::Patcher;
use crate::fs_utils::find_isaac_game_path;

#[derive(Default)]
enum AppState {
    #[default]
    Idle,
    Checking,
    Syncing,
    Done,
    Error(String),
}

pub struct PatcherApp {
    game_path: Option<PathBuf>,
    target_mod_path: Option<PathBuf>,
    state: AppState,
    status_message: String,
    progress_log: Arc<Mutex<Vec<String>>>,
    github_client: GitHubClient,
    repo_branch: String,
}

impl Default for PatcherApp {
    fn default() -> Self {
        let client = GitHubClient::new("Ba-koD", "conch_blessing");
        let mut app = Self {
            game_path: None,
            target_mod_path: None,
            state: AppState::Idle,
            status_message: "Ready".to_string(),
            progress_log: Arc::new(Mutex::new(Vec::new())),
            github_client: client,
            repo_branch: "main".to_string(),
        };
        
        // Try load config or auto-detect
        if let Some(path) = load_config() {
            app.game_path = Some(path);
        } else if let Some(path) = find_isaac_game_path() {
            app.game_path = Some(path.clone());
            let _ = save_config(&path);
        }
        
        app
    }
}

impl PatcherApp {
    fn check_mod_folder(&mut self) {
        let Some(game_path) = &self.game_path else { return };
        let mods_path = game_path.join("mods");
        
        if !mods_path.exists() {
            self.status_message = "Mods folder not found inside game directory.".to_string();
            self.target_mod_path = None;
            return;
        }

        self.state = AppState::Checking;
        self.status_message = "Fetching metadata...".to_string();
        
        match self.github_client.fetch_metadata_id(&self.repo_branch) {
            Ok(id) => {
                // Look for conch_blessing_{id}
                let expected_name = format!("conch_blessing_{}", id);
                let specific_path = mods_path.join(&expected_name);
                
                if specific_path.exists() {
                    self.target_mod_path = Some(specific_path);
                    self.status_message = format!("Found mod: {}", expected_name);
                } else {
                    // Fallback check: just "conch_blessing"?
                    let fallback = mods_path.join("conch_blessing");
                    if fallback.exists() {
                        self.target_mod_path = Some(fallback);
                        self.status_message = "Found mod: conch_blessing".to_string();
                    } else {
                        // Check for any conch_blessing_*
                        if let Ok(entries) = std::fs::read_dir(&mods_path) {
                            let mut found = None;
                            for entry in entries.flatten() {
                                let name = entry.file_name().to_string_lossy().to_string();
                                if name.starts_with("conch_blessing") {
                                    found = Some(mods_path.join(name));
                                    break;
                                }
                            }
                            if let Some(p) = found {
                                self.target_mod_path = Some(p);
                                self.status_message = "Found mod (generic match)".to_string();
                            } else {
                                self.target_mod_path = None;
                                self.status_message = "Mod not found! Please install it first.".to_string();
                            }
                        }
                    }
                }
            },
            Err(e) => {
                self.status_message = format!("Failed to fetch metadata: {}", e);
                self.state = AppState::Error(e.to_string());
            }
        }
        
        if self.target_mod_path.is_some() {
            self.state = AppState::Idle;
        }
    }

    fn start_patching(&mut self) {
        let Some(target) = &self.target_mod_path else { return };
        let target = target.clone();
        let client = self.github_client.clone();
        let branch = self.repo_branch.clone();
        let log = self.progress_log.clone();
        
        self.state = AppState::Syncing;
        self.status_message = "Patching...".to_string();
        
        thread::spawn(move || {
            let patcher = Patcher::new(client, target);
            let log_err = log.clone();
            
            let logger = move |msg: String| {
                if let Ok(mut l) = log.lock() {
                    l.push(msg);
                }
            };
            
            if let Err(e) = patcher.sync(&branch, Some(logger)) {
                if let Ok(mut l) = log_err.lock() {
                    l.push(format!("Error: {}", e));
                }
            }
        });
    }
}

impl eframe::App for PatcherApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Conch Blessing Patcher");
            
            ui.horizontal(|ui| {
                ui.label("Game Path:");
                if let Some(path) = &self.game_path {
                    ui.label(path.to_string_lossy());
                } else {
                    ui.label("Not selected");
                }
                
                if ui.button("Select Folder").clicked() {
                    if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                        self.game_path = Some(folder.clone());
                        let _ = save_config(&folder);
                        self.check_mod_folder();
                    }
                }
            });

            ui.separator();
            
            if let Some(target) = &self.target_mod_path {
                ui.label(format!("Target Mod: {:?}", target.file_name().unwrap()));
            } else if self.game_path.is_some() {
                ui.colored_label(egui::Color32::RED, &self.status_message);
                if ui.button("Re-scan").clicked() {
                    self.check_mod_folder();
                }
            }
            
            ui.separator();
            
            // Status
            ui.label(&self.status_message);
            
            ui.add_space(10.0);
            
            if matches!(self.state, AppState::Idle) && self.target_mod_path.is_some() {
                if ui.button("Update / Patch").clicked() {
                    self.start_patching();
                }
            } else if matches!(self.state, AppState::Syncing) {
                ui.spinner();
                ui.label("Patching in progress...");
            }
            
            // Logs
            let logs = self.progress_log.lock().unwrap();
            egui::ScrollArea::vertical().show(ui, |ui| {
                for log in logs.iter() {
                    ui.label(log);
                }
            });
            
            if let Some(last) = logs.last() {
                if last.contains("Update complete!") && matches!(self.state, AppState::Syncing) {
                    self.state = AppState::Done;
                    self.status_message = "Update Successful!".to_string();
                } else if last.contains("Error:") && matches!(self.state, AppState::Syncing) {
                    self.state = AppState::Error("Failed".to_string());
                    self.status_message = "Update Failed!".to_string();
                }
            }
        });
    }
}

pub fn run() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([600.0, 400.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Conch Blessing Patcher",
        options,
        Box::new(|_cc| Box::new(PatcherApp::default())),
    )
}

#[cfg(target_os = "windows")]
fn save_config(path: &Path) -> anyhow::Result<()> {
    use winreg::enums::*;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey("Software\\Ba-koD\\cb_patcher")?;
    key.set_value("IsaacPath", &path.to_string_lossy().as_ref())?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn load_config() -> Option<PathBuf> {
    use winreg::enums::*;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu.open_subkey("Software\\Ba-koD\\cb_patcher").ok()?;
    let path_str: String = key.get_value("IsaacPath").ok()?;
    Some(PathBuf::from(path_str))
}

#[cfg(not(target_os = "windows"))]
fn save_config(_path: &Path) -> anyhow::Result<()> {
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn load_config() -> Option<PathBuf> {
    None
}
