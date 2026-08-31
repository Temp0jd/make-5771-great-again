#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod model;
mod platform;
mod runner;
mod storage;
mod template_editor;
mod theme;
mod vision;

use app::Make5771App;
use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([980.0, 700.0])
            .with_min_inner_size([860.0, 620.0])
            .with_decorations(false)
            .with_title("Make 5771 Great Again"),
        ..Default::default()
    };

    eframe::run_native(
        "Make 5771 Great Again",
        options,
        Box::new(|cc| Ok(Box::new(Make5771App::new(cc)))),
    )
}
