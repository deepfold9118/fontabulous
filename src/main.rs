#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod catalog;
mod coverage;
mod filters;
mod fontdb;
mod icons;
mod logo;
mod platform;
mod render;
mod theme;
mod ui_center;
mod ui_dialogs;
mod ui_panels;
mod widgets;

use std::path::PathBuf;

fn app_icon() -> egui::IconData {
    // The Fontabulous logo (accent-coloured bar and hook) on a transparent background.
    let n = 256u32;
    egui::IconData { rgba: logo::rasterize(n), width: n, height: n }
}

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().collect();
    let mut screenshot: Option<PathBuf> = None;
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--screenshot" && i + 1 < args.len() {
            screenshot = Some(PathBuf::from(&args[i + 1]));
            i += 1;
        }
        i += 1;
    }

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Fontabulous")
        .with_inner_size([1440.0, 860.0])
        .with_min_inner_size([900.0, 560.0])
        .with_decorations(false)
        .with_icon(app_icon());
    if screenshot.is_some() {
        viewport = viewport.with_active(false);
        #[cfg(windows)]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }

    #[allow(unused_mut)]
    let mut options = eframe::NativeOptions { viewport, ..Default::default() };
    #[cfg(target_os = "macos")]
    if screenshot.is_some() {
        options.event_loop_builder = Some(Box::new(|b| {
            use winit::platform::macos::EventLoopBuilderExtMacOS;
            b.with_activate_ignoring_other_apps(false);
        }));
    }

    eframe::run_native(
        "Fontabulous",
        options,
        Box::new(move |cc| Ok(Box::new(app::FontApp::new(cc, screenshot.clone())))),
    )
}
