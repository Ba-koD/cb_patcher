#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // Hide console window on Windows in release

mod github;
mod fs_utils;
mod patcher;
mod gui;

use anyhow::Result;

fn main() -> Result<()> {
    gui::run().map_err(|e| anyhow::anyhow!("GUI Error: {}", e))
}
