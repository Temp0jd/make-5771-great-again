#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod mascot;
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
    install_panic_hook();

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

/// Writes panic details to `crash.log` next to the executable so GUI
/// failures (which otherwise abort silently) can be reported and diagnosed.
fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let payload = info
            .payload()
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| info.payload().downcast_ref::<String>().map(String::as_str))
            .unwrap_or("<unknown>");
        let location = info
            .location()
            .map(|location| format!("{}:{}", location.file(), location.line()))
            .unwrap_or_else(|| "<unknown location>".to_owned());
        let line = format!(
            "{} PANIC at {location}: {payload}\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
        );
        eprint!("{line}");
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("crash.log")
        {
            use std::io::Write;
            let _ = file.write_all(line.as_bytes());
        }
    }));
}
