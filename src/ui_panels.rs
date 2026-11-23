//! Window chrome (title bar with menus, context bar, status bar) and the left/right docks.

use egui::{Align, Align2, Color32, CornerRadius, Frame, Layout, Margin, Panel, Sense, Stroke, Vec2};
use std::path::PathBuf;

use crate::app::*;
use crate::fontdb::{self, family_class_name, panose_family_name, panose_proportion_name, panose_serif_name, weight_name, width_name};
use crate::icons as I;
use crate::platform;
use crate::theme::{bold, icon_font, ThemeMode};
use crate::widgets::*;

// --------------------------------------------------------------------------------------------
// Title bar

pub fn title_bar(app: &mut FontApp, ui: &mut egui::Ui) {
    let pal = app.pal;
    Panel::top("titlebar")
        .exact_size(36.0)
        .frame(Frame::new().fill(pal.title).inner_margin(Margin::symmetric(8, 0)))
        .show_separator_line(false)
        .show(ui, |ui| {
            let bar_rect = ui.max_rect();
            // Drag / double-click on empty title area moves / maximises the window.
            let bg = ui.interact(bar_rect, ui.id().with("title_drag"), Sense::click_and_drag());
            if bg.drag_started() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            if bg.double_clicked() {
                let max = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
            }
            ui.horizontal_centered(|ui| {
                // App mark: the Fontabulous logo.
                // 17 px tall (was 22): smaller so it sits lighter in the bar.
                ui.add_space(2.0);
                let (r, _) = ui.allocate_exact_size(Vec2::new(13.0, 17.0), Sense::hover());
                paint_logo(ui.painter(), r, pal.accent);
                ui.add_space(2.0);
                ui.add_space(6.0);
                egui::MenuBar::new().ui(ui, |ui| {
                    menus(app, ui);
                });
            });
            // Centered title.
            ui.painter().text(
                bar_rect.center(),
                Align2::CENTER_CENTER,
                "Fontabulous",
                bold(12.5),
                pal.muted,
            );
            // Right side controls.
            let right = egui::Rect::from_min_max(egui::pos2(bar_rect.right() - 260.0, bar_rect.top()), bar_rect.max);
            ui.scope_builder(egui::UiBuilder::new().max_rect(right).layout(Layout::right_to_left(Align::Center)), |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                window_button(ui, &pal, I::X, true).clicked().then(|| ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close));
                let max = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
                if window_button(ui, &pal, if max { I::MINIMIZE_2 } else { I::SQUARE }, false).clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
                }
                if window_button(ui, &pal, I::MINUS, false).clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                }
                ui.add_space(8.0);
                let (icon, tip, next) = if pal.dark {
                    (I::SUN, "Switch to light theme", ThemeMode::Light)
                } else {
                    (I::MOON, "Switch to dark theme", ThemeMode::Dark)
                };
                if icon_button(ui, &pal, icon, tip, false).clicked() {
                    let ctx = ui.ctx().clone();
                    app.set_theme(&ctx, next);
                }
                if icon_button(ui, &pal, I::SEARCH, "Find Fonts (Ctrl+F)", app.find.open).clicked() {
                    app.find.open = !app.find.open;
                }
            });
        });
}

fn window_button(ui: &mut egui::Ui, pal: &crate::theme::Palette, icon: &str, close: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(40.0, 30.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(4), if close { pal.red } else { pal.hover });
    }
    let col = if resp.hovered() && close { Color32::WHITE } else { pal.muted };
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, icon, icon_font(14.0), col);
    resp
}

fn mi(ui: &mut egui::Ui, icon: &str, label: &str) -> bool {
    ui.button(format!("{icon}  {label}")).clicked()
}

fn mi_en(ui: &mut egui::Ui, enabled: bool, icon: &str, label: &str) -> bool {
    ui.add_enabled(enabled, egui::Button::new(format!("{icon}  {label}"))).clicked()
}

fn menus(app: &mut FontApp, ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    let has_sel = !app.selected_ids().is_empty();
    ui.menu_button("File", |ui| {
        if mi(ui, I::FOLDER_OPEN, "Open Folder…") {
            if let Some(d) = rfd::FileDialog::new().pick_folder() {
                app.left_tab = LeftTab::Folders;
                app.folder_path_edit = d.display().to_string();
                go_folder(app, d);
            }
        }
        if mi(ui, I::LIST_PLUS, "Add Font Files to Worklist…") {
            if let Some(files) = rfd::FileDialog::new().add_filter("Fonts", fontdb::FONT_EXTS).pick_files() {
                app.add_files_to_worklist(files);
                app.set_source(Source::Worklist);
            }
        }
        ui.separator();
        if mi_en(ui, true, I::FILE_CODE, "Export to HTML Page…") {
            app.export_html(&ctx);
        }
        if mi_en(ui, has_sel, I::COPY, "Copy Font Files to Folder…") {
            app.copy_selected_to_folder(&ctx);
        }
        if mi_en(ui, has_sel, I::EXTERNAL_LINK, "Show in Explorer") {
            if let Some(i) = app.focus {
                platform::reveal(&app.faces[i].key.path);
            }
        }
        ui.separator();
        if mi(ui, I::X, "Exit") {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    });
    ui.menu_button("Edit", |ui| {
        if mi(ui, I::SQUARE_CHECK_BIG, "Select All    Ctrl+A") {
            app.selection = app.view.iter().copied().collect();
        }
        if mi(ui, I::ARROW_UP_DOWN, "Invert Selection") {
            let cur = app.selection.clone();
            app.selection = app.view.iter().copied().filter(|i| !cur.contains(i)).collect();
        }
        if mi(ui, I::X, "Clear Selection") {
            app.selection.clear();
        }
        ui.separator();
        if mi_en(ui, has_sel, I::COPY, "Copy Font Name") {
            let names: Vec<String> = app.selected_ids().iter().map(|&i| app.faces[i].display_name().to_string()).collect();
            ctx.copy_text(names.join("\n"));
        }
        if mi_en(ui, has_sel, I::CLIPBOARD, "Copy File Path") {
            let names: Vec<String> = app.selected_ids().iter().map(|&i| app.faces[i].key.path.display().to_string()).collect();
            ctx.copy_text(names.join("\n"));
        }
    });
    ui.menu_button("View", |ui| {
        for (m, icon, label) in [(ViewMode::List, I::LIST, "List"), (ViewMode::Tiles, I::LAYOUT_GRID, "Tiles"), (ViewMode::Details, I::TABLE, "Details")] {
            if ui.add(egui::Button::selectable(app.cfg.view_mode == m, format!("{icon}  {label}"))).clicked() {
                app.cfg.view_mode = m;
                app.mark_cfg_dirty();
            }
        }
        ui.separator();
        let mut l = app.cfg.show_left;
        let mut b = app.cfg.show_bottom;
        let mut r = app.cfg.show_right;
        ui.checkbox(&mut l, "Fonts panel");
        ui.checkbox(&mut b, "Sample panel");
        ui.checkbox(&mut r, "Task panel");
        if (l, b, r) != (app.cfg.show_left, app.cfg.show_bottom, app.cfg.show_right) {
            app.cfg.show_left = l;
            app.cfg.show_bottom = b;
            app.cfg.show_right = r;
            app.mark_cfg_dirty();
        }
        ui.separator();
        for (t, icon, label) in [
            (BottomTab::TextSample, I::TEXT, "Text Sample"),
            (BottomTab::Characters, I::GRID_3X3, "Characters Table"),
            (BottomTab::Waterfall, I::WAVES, "Waterfall"),
            (BottomTab::Tags, I::TAGS, "Tags"),
            (BottomTab::Search, I::SEARCH, "Search"),
        ] {
            if mi(ui, icon, label) {
                app.bottom_tab = t;
                app.cfg.show_bottom = true;
            }
        }
        if mi(ui, I::INFO, "Properties") {
            app.right_tab = RightTab::Properties;
            app.cfg.show_right = true;
        }
    });
    ui.menu_button("Fonts", |ui| {
        if mi_en(ui, has_sel, I::CIRCLE_CHECK, "Install") {
            app.install_selected(&ctx);
        }
        if mi_en(ui, has_sel, I::ZAP, "Activate Temporarily") {
            app.activate_selected(&ctx);
        }
        if mi_en(ui, has_sel, I::POWER_OFF, "Uninstall or Deactivate…") {
            app.request_uninstall();
        }
        ui.separator();
        if mi_en(ui, has_sel, I::LAYERS, "Add to Groups…    Ctrl+G") {
            app.modal = Modal::AddToGroups { checked: Default::default(), new_name: String::new() };
        }
        if mi_en(ui, has_sel, I::LIBRARY, "Add to Library    Ctrl+L") {
            app.add_selected_to_library(&ctx);
        }
        if mi_en(ui, has_sel, I::LIST_PLUS, "Add to Worklist") {
            app.add_selected_to_worklist();
        }
        ui.separator();
        if mi(ui, I::FOLDER_PLUS, "Attach My Collection…") {
            app.attach_collection();
        }
        if let Source::Collection(p) = app.source.clone() {
            if mi(ui, I::FOLDER_TREE, "Detach My Collection") {
                app.cfg.collections.retain(|c| c != &p);
                app.set_source(Source::Installed);
                app.mark_cfg_dirty();
            }
            if mi(ui, I::REFRESH_CW, "Synchronize") {
                app.sync_collection(&p);
            }
        }
        ui.separator();
        if mi_en(ui, app.selected_ids().len() >= 2, I::GIT_COMPARE, "Compare…") {
            app.compare_ids = app.selected_ids().into_iter().take(4).collect();
            app.compare_open = true;
        }
        if mi_en(ui, has_sel, I::BASELINE, "Show Kerning Pairs…") {
            app.kerning_open = true;
            app.kerning_cache = None;
        }
        ui.separator();
        ui.menu_button(format!("{}  Sort By", I::ARROW_DOWN_A_Z), |ui| {
            for s in SortBy::ALL {
                if ui.add(egui::Button::selectable(app.cfg.sort == s, s.label())).clicked() {
                    app.cfg.sort = s;
                    app.view_dirty = true;
                    app.mark_cfg_dirty();
                }
            }
            ui.separator();
            if ui.add(egui::Button::selectable(app.cfg.sort_desc, "Descending")).clicked() {
                app.cfg.sort_desc = !app.cfg.sort_desc;
                app.view_dirty = true;
            }
        });
    });
    ui.menu_button("Group", |ui| {
        if mi(ui, I::PLUS, "New Group…") {
            app.modal = Modal::NewGroup { name: String::new(), add_selected: has_sel };
        }
        if mi(ui, I::FOLDER_TREE, "Create Groups from Folders Structure…") {
            app.create_groups_from_folders(&ctx);
        }
        if let Source::Group(g) = app.source.clone() {
            ui.separator();
            if mi(ui, I::PENCIL, "Rename Group…") {
                app.modal = Modal::RenameGroup { old: g.clone(), name: g.clone() };
            }
            if mi_en(ui, has_sel, I::MINUS, "Remove Selected from Group") {
                let keys: Vec<_> = app.selected_ids().iter().map(|&i| app.faces[i].key.clone()).collect();
                if let Some(v) = app.cfg.groups.get_mut(&g) {
                    v.retain(|k| !keys.contains(k));
                }
                app.view_dirty = true;
                app.mark_cfg_dirty();
            }
            if mi(ui, I::TRASH_2, "Delete Group") {
                app.cfg.groups.remove(&g);
                app.set_source(Source::Installed);
                app.mark_cfg_dirty();
            }
        }
    });
    ui.menu_button("Worklist", |ui| {
        if mi(ui, I::LIST_PLUS, "Add Files…") {
            if let Some(files) = rfd::FileDialog::new().add_filter("Fonts", fontdb::FONT_EXTS).pick_files() {
                app.add_files_to_worklist(files);
            }
            app.set_source(Source::Worklist);
        }
        if mi_en(ui, has_sel, I::LIST_PLUS, "Add Selected Fonts") {
            app.add_selected_to_worklist();
        }
        if mi(ui, I::LIST_X, "Clear Worklist") {
            app.cfg.worklist.clear();
            app.view_dirty = true;
            app.mark_cfg_dirty();
        }
        ui.separator();
        if mi(ui, I::FOLDER_OPEN, "Open Worklist…") {
            app.open_worklist();
        }
        if mi(ui, I::SAVE, "Save Worklist As…") {
            app.save_worklist(&ctx);
        }
    });
    ui.menu_button("Tools", |ui| {
        if mi(ui, I::FOLDER_SEARCH, "Find Fonts…    Ctrl+F") {
            app.find.open = true;
        }
        if mi(ui, I::COPY_CHECK, "Search for Duplicates…") {
            app.dups.open = true;
        }
        ui.separator();
        if mi(ui, I::FILE_CODE, "Export to HTML…") {
            app.export_html(&ctx);
        }
        ui.separator();
        if mi(ui, I::MONITOR, "Show Installed Fonts") {
            app.set_source(Source::Installed);
        }
        if mi(ui, I::ZAP, "Show Activated Fonts") {
            app.set_source(Source::Activated);
        }
        if mi(ui, I::TRIANGLE_ALERT, "Detect Font Problems…") {
            app.detect_problems();
        }
        ui.separator();
        if mi(ui, I::UPLOAD, "Export Tags…") {
            app.export_tags(&ctx);
        }
        if mi(ui, I::DOWNLOAD, "Import Tags…") {
            app.import_tags(&ctx);
        }
        ui.separator();
        ui.menu_button(format!("{}  Theme", I::PALETTE), |ui| {
            if ui.add(egui::Button::selectable(app.cfg.theme == ThemeMode::Light, format!("{}  Light", I::SUN))).clicked() {
                app.set_theme(&ctx, ThemeMode::Light);
            }
            if ui.add(egui::Button::selectable(app.cfg.theme == ThemeMode::Dark, format!("{}  Dark", I::MOON))).clicked() {
                app.set_theme(&ctx, ThemeMode::Dark);
            }
        });
        if mi(ui, I::SETTINGS, "Options…") {
            app.options_open = true;
        }
    });
    ui.menu_button("Help", |ui| {
        if mi(ui, I::INFO, "About Fontabulous") {
            app.about_open = true;
        }
    });
}

// --------------------------------------------------------------------------------------------
// Context bar (quick find, activation filters, panel toggles)

pub fn context_bar(app: &mut FontApp, ui: &mut egui::Ui) {
    let pal = app.pal;
    Panel::top("contextbar")
        .exact_size(40.0)
        .frame(Frame::new().fill(pal.panel).inner_margin(Margin::symmetric(10, 0)).stroke(Stroke::NONE))
        .show_separator_line(true)
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.label(egui::RichText::new(I::SEARCH).font(icon_font(14.0)).color(pal.muted));
                let r = ui.add(
                    egui::TextEdit::singleline(&mut app.quick_find)
                        .hint_text("Find fonts by name…")
                        .desired_width(220.0),
                );
                if r.changed() {
                    app.view_dirty = true;
                }
                if !app.quick_find.is_empty() && icon_button(ui, &pal, I::X, "Clear", false).clicked() {
                    app.quick_find.clear();
                    app.view_dirty = true;
                }
                vsep(ui, &pal);
                ui.label(egui::RichText::new("Show").color(pal.muted).size(12.0));
                let (a, b, c) = (app.show_installed, app.show_activated, app.show_not_installed);
                dot_chip(ui, &pal, pal.green, "Installed", &mut app.show_installed);
                dot_chip(ui, &pal, pal.orange, "Activated", &mut app.show_activated);
                dot_chip(ui, &pal, pal.grey, "Not installed", &mut app.show_not_installed);
                if (a, b, c) != (app.show_installed, app.show_activated, app.show_not_installed) {
                    app.view_dirty = true;
                }
                vsep(ui, &pal);
                let icon = match app.source {
                    Source::Installed => I::MONITOR,
                    Source::Folder(_) | Source::Collection(_) => I::FOLDER_OPEN,
                    Source::Group(_) => I::LAYERS,
                    Source::Worklist => I::LIST,
                    Source::Results(_) => I::SEARCH,
                    _ => I::TYPE,
                };
                ui.label(egui::RichText::new(icon).font(icon_font(14.0)).color(pal.accent_text));
                ui.add(egui::Label::new(egui::RichText::new(app.source_title()).color(pal.text).strong()).truncate());

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    if icon_button(ui, &pal, I::PANEL_RIGHT, "Task panel", app.cfg.show_right).clicked() {
                        app.cfg.show_right = !app.cfg.show_right;
                        app.mark_cfg_dirty();
                    }
                    if icon_button(ui, &pal, I::PANEL_BOTTOM, "Sample panel", app.cfg.show_bottom).clicked() {
                        app.cfg.show_bottom = !app.cfg.show_bottom;
                        app.mark_cfg_dirty();
                    }
                    if icon_button(ui, &pal, I::PANEL_LEFT, "Fonts panel", app.cfg.show_left).clicked() {
                        app.cfg.show_left = !app.cfg.show_left;
                        app.mark_cfg_dirty();
                    }
                    ui.label(egui::RichText::new("Workspace").color(pal.muted).size(12.0));
                });
            });
        });
}

// --------------------------------------------------------------------------------------------
// Status bar

pub fn status_bar(app: &mut FontApp, ui: &mut egui::Ui) {
    let pal = app.pal;
    Panel::bottom("statusbar")
        .exact_size(26.0)
        .frame(Frame::new().fill(pal.title).inner_margin(Margin::symmetric(10, 0)))
        .show_separator_line(true)
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                let now = ui.ctx().input(|i| i.time);
                if let Some((msg, until)) = &app.toast {
                    if now < *until {
                        ui.label(egui::RichText::new(I::CIRCLE_CHECK).font(icon_font(12.0)).color(pal.green));
                        ui.label(egui::RichText::new(msg).size(11.5).color(pal.text));
                        ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
                    } else {
                        app.toast = None;
                    }
                } else if app.scanner.busy {
                    ui.add(egui::Spinner::new().size(12.0).color(pal.accent));
                    let label = if app.scanning_label.is_empty() { "Reading fonts…".to_string() } else { app.scanning_label.clone() };
                    ui.add(egui::Label::new(egui::RichText::new(label).size(11.5).color(pal.muted)).truncate());
                } else {
                    ui.label(egui::RichText::new(I::INFO).font(icon_font(12.0)).color(pal.muted));
                    ui.label(egui::RichText::new("Click a font to select it · Ctrl+Click to add · Shift+Click for a range · Double-click to open its properties").size(11.5).color(pal.muted));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let (perm, temp) = app.counts();
                    ui.label(
                        egui::RichText::new(format!("Permanently installed: {perm}   Activated temporarily: {temp}   Total: {}", perm + temp))
                            .size(11.5)
                            .color(pal.muted),
                    );
                    vsep(ui, &pal);
                    ui.label(
                        egui::RichText::new(format!("Total: {}  (Selected: {})", app.view.len(), app.selection.len()))
                            .size(11.5)
                            .color(pal.text),
                    );
                });
            });
        });
}

// --------------------------------------------------------------------------------------------
// Left dock

pub fn left_panel(app: &mut FontApp, ui: &mut egui::Ui) {
    let pal = app.pal;
    Panel::left("leftdock")
        .default_size(310.0)
        .size_range(240.0..=560.0)
        .resizable(true)
        .frame(Frame::new().fill(pal.panel))
        .show(ui, |ui| {
            dock_header(ui, &pal, |ui| {
                tab_strip(
                    ui,
                    &pal,
                    &mut app.left_tab,
                    &[(LeftTab::Fonts, "Fonts"), (LeftTab::Filters, "Filters"), (LeftTab::Worklist, "Worklist"), (LeftTab::System, "System"), (LeftTab::Folders, "Folders")],
                );
            });
            let inner = ui.available_rect_before_wrap().shrink2(Vec2::new(6.0, 6.0));
            ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| match app.left_tab {
                LeftTab::Fonts => fonts_tree(app, ui),
                LeftTab::Worklist => worklist_tab(app, ui),
                LeftTab::System => system_tab(app, ui),
                LeftTab::Folders => folders_tab(app, ui),
                LeftTab::Filters => crate::filters::panel(app, ui),
            });
        });
}

fn fonts_tree(app: &mut FontApp, ui: &mut egui::Ui) {
    let pal = app.pal;
    let ctx = ui.ctx().clone();
    let bottom_h = 84.0;
    let tree_h = (ui.available_height() - bottom_h).max(60.0);
    egui::ScrollArea::vertical().id_salt("fonts_tree").max_height(tree_h).auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        let sel = app.source.clone();
        let n_inst = app.count_source(&Source::Installed);
        if tree_row(ui, &pal, 0, None, I::MONITOR, "Installed Fonts", Some(n_inst), sel == Source::Installed).0.clicked() {
            app.set_source(Source::Installed);
        }
        let n_act = app.activated.len();
        if tree_row(ui, &pal, 0, None, I::ZAP, "Activated Fonts", Some(n_act), sel == Source::Activated).0.clicked() {
            app.set_source(Source::Activated);
        }
        ui.add_space(4.0);

        // My Collections
        let open_id = ui.make_persistent_id("tree_collections");
        let mut open = ui.data_mut(|d| *d.get_persisted_mut_or(open_id, true));
        let total: usize = app.cfg.collections.iter().map(|c| app.count_source(&Source::Collection(c.clone()))).sum();
        let (r, t) = tree_row(ui, &pal, 0, Some(open), I::USER, "My Collections", Some(total), false);
        if r.clicked() || t {
            open = !open;
            ui.data_mut(|d| d.insert_persisted(open_id, open));
        }
        r.context_menu(|ui| {
            if ui.button(format!("{}  Attach My Collection…", I::FOLDER_PLUS)).clicked() {
                app.attach_collection();
                ui.close();
            }
        });
        if open {
            if app.cfg.collections.is_empty() {
                ui.horizontal(|ui| {
                    ui.add_space(40.0);
                    ui.label(egui::RichText::new("No collections attached yet").size(11.5).color(pal.faint));
                });
            }
            for c in app.cfg.collections.clone() {
                let src = Source::Collection(c.clone());
                let name = format!(
                    "{} ({})",
                    c.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| c.display().to_string()),
                    c.display()
                );
                let n = app.count_source(&src);
                let resp = tree_row(ui, &pal, 1, None, I::FOLDER, &name, Some(n), sel == src).0;
                if resp.clicked() {
                    app.set_source(src.clone());
                }
                resp.context_menu(|ui| {
                    if ui.button(format!("{}  Synchronize", I::REFRESH_CW)).clicked() {
                        app.sync_collection(&c);
                        ui.close();
                    }
                    if ui.button(format!("{}  Show in Explorer", I::EXTERNAL_LINK)).clicked() {
                        platform::reveal(&c);
                        ui.close();
                    }
                    if ui.button(format!("{}  Detach Collection", I::X)).clicked() {
                        app.cfg.collections.retain(|x| x != &c);
                        if app.source == src {
                            app.set_source(Source::Installed);
                        }
                        app.mark_cfg_dirty();
                        ui.close();
                    }
                });
            }
        }

        // Groups
        let gid = ui.make_persistent_id("tree_groups");
        let mut gopen = ui.data_mut(|d| *d.get_persisted_mut_or(gid, true));
        let (r, t) = tree_row(ui, &pal, 0, Some(gopen), I::LAYERS, "Groups", Some(app.cfg.groups.len()), false);
        if r.clicked() || t {
            gopen = !gopen;
            ui.data_mut(|d| d.insert_persisted(gid, gopen));
        }
        r.context_menu(|ui| {
            if ui.button(format!("{}  New Group…", I::PLUS)).clicked() {
                app.modal = Modal::NewGroup { name: String::new(), add_selected: false };
                ui.close();
            }
            if ui.button(format!("{}  Create Groups from Folders…", I::FOLDER_TREE)).clicked() {
                app.create_groups_from_folders(&ctx);
                ui.close();
            }
        });
        if gopen {
            let names: Vec<String> = app.cfg.groups.keys().cloned().collect();
            for g in names {
                let src = Source::Group(g.clone());
                let n = app.cfg.groups.get(&g).map(|v| v.len()).unwrap_or(0);
                let resp = tree_row(ui, &pal, 1, None, I::BOOKMARK, &g, Some(n), sel == src).0;
                if resp.clicked() {
                    app.set_source(src.clone());
                }
                resp.context_menu(|ui| {
                    if ui.button(format!("{}  Rename…", I::PENCIL)).clicked() {
                        app.modal = Modal::RenameGroup { old: g.clone(), name: g.clone() };
                        ui.close();
                    }
                    if ui.button(format!("{}  Add Selected Fonts", I::PLUS)).clicked() {
                        let set = [g.clone()].into_iter().collect();
                        app.add_selected_to_groups(&set);
                        ui.close();
                    }
                    if ui.button(format!("{}  Delete Group", I::TRASH_2)).clicked() {
                        app.cfg.groups.remove(&g);
                        if app.source == src {
                            app.set_source(Source::Installed);
                        }
                        app.mark_cfg_dirty();
                        ui.close();
                    }
                });
            }
        }

        // Library
        let n_lib = app.count_source(&Source::Library);
        let resp = tree_row(ui, &pal, 0, None, I::LIBRARY, "Library", Some(n_lib), sel == Source::Library).0;
        if resp.clicked() {
            app.set_source(Source::Library);
        }
        resp.context_menu(|ui| {
            if ui.button(format!("{}  Add Folder to Library…", I::FOLDER_PLUS)).clicked() {
                app.add_folder_to_library();
                ui.close();
            }
            if ui.button(format!("{}  Clear Library", I::TRASH_2)).clicked() {
                app.cfg.library.clear();
                app.view_dirty = true;
                app.mark_cfg_dirty();
                ui.close();
            }
        });

        // Recently added
        let rid = ui.make_persistent_id("tree_recent");
        let mut ropen = ui.data_mut(|d| *d.get_persisted_mut_or(rid, true));
        let (r, t) = tree_row(ui, &pal, 0, Some(ropen), I::HISTORY, "Recently Added", None, false);
        if r.clicked() || t {
            ropen = !ropen;
            ui.data_mut(|d| d.insert_persisted(rid, ropen));
        }
        if ropen {
            for s in RecentSpan::ALL {
                let src = Source::Recent(s);
                let n = app.count_source(&src);
                if tree_row(ui, &pal, 1, None, I::CLOCK, s.label(), Some(n), sel == src).0.clicked() {
                    app.set_source(src);
                }
            }
        }
    });
    ui.add_space(6.0);
    if wide_button(ui, &pal, I::FOLDER_PLUS, "Attach Collection…").clicked() {
        app.attach_collection();
    }
    ui.add_space(4.0);
    if wide_button(ui, &pal, I::FOLDER_TREE, "Create Groups from Folders Structure…").clicked() {
        app.create_groups_from_folders(&ctx);
    }
}

fn worklist_tab(app: &mut FontApp, ui: &mut egui::Ui) {
    let pal = app.pal;
    let ctx = ui.ctx().clone();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        if icon_button(ui, &pal, I::LIST_PLUS, "Add font files", false).clicked() {
            if let Some(files) = rfd::FileDialog::new().add_filter("Fonts", fontdb::FONT_EXTS).pick_files() {
                app.add_files_to_worklist(files);
            }
            app.set_source(Source::Worklist);
        }
        if icon_button(ui, &pal, I::FOLDER_PLUS, "Add a folder of fonts", false).clicked() {
            if let Some(d) = rfd::FileDialog::new().pick_folder() {
                let files: Vec<PathBuf> = std::fs::read_dir(&d)
                    .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| fontdb::is_font_file(p)).collect())
                    .unwrap_or_default();
                app.add_files_to_worklist(files);
                app.set_source(Source::Worklist);
            }
        }
        if icon_button(ui, &pal, I::SAVE, "Save worklist", false).clicked() {
            app.save_worklist(&ctx);
        }
        if icon_button(ui, &pal, I::FOLDER_OPEN, "Open worklist", false).clicked() {
            app.open_worklist();
        }
        vsep(ui, &pal);
        if icon_button(ui, &pal, I::MINUS, "Remove selected fonts from the worklist", false).clicked() {
            let paths: Vec<PathBuf> = app.selected_ids().iter().map(|&i| app.faces[i].key.path.clone()).collect();
            app.cfg.worklist.retain(|p| !paths.contains(p));
            app.view_dirty = true;
            app.mark_cfg_dirty();
        }
        if icon_button(ui, &pal, I::LIST_X, "Clear the worklist", false).clicked() {
            app.cfg.worklist.clear();
            app.view_dirty = true;
            app.mark_cfg_dirty();
        }
    });
    ui.add_space(4.0);
    let w = ui.available_width();
    egui::Grid::new("wl_head").num_columns(2).min_col_width(w * 0.45).show(ui, |ui| {
        ui.label(egui::RichText::new("File Name").color(pal.muted).size(11.5));
        ui.label(egui::RichText::new("Location").color(pal.muted).size(11.5));
        ui.end_row();
    });
    ui.separator();
    if app.cfg.worklist.is_empty() {
        ui.add_space(10.0);
        ui.label(egui::RichText::new("The worklist is empty. Add font files to try them before installing.").color(pal.faint).size(12.0));
        return;
    }
    if app.source != Source::Worklist && ui.small_button(format!("{}  Show worklist fonts", I::EYE)).clicked() {
        app.set_source(Source::Worklist);
    }
    egui::ScrollArea::vertical().id_salt("wl").auto_shrink([false, false]).show(ui, |ui| {
        for p in app.cfg.worklist.clone() {
            let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let loc = p.parent().map(|n| n.display().to_string()).unwrap_or_default();
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 22.0), Sense::click());
            if resp.hovered() {
                ui.painter().rect_filled(rect, 3.0, pal.hover.gamma_multiply(0.6));
            }
            let clip = egui::Rect::from_min_max(rect.min, egui::pos2(rect.left() + w * 0.47, rect.bottom()));
            ui.painter().with_clip_rect(clip).text(rect.left_center() + Vec2::new(4.0, 0.0), Align2::LEFT_CENTER, &name, egui::FontId::proportional(12.0), pal.text);
            ui.painter().with_clip_rect(rect).text(egui::pos2(rect.left() + w * 0.5, rect.center().y), Align2::LEFT_CENTER, &loc, egui::FontId::proportional(12.0), pal.muted);
            if resp.clicked() {
                app.set_source(Source::Worklist);
                if let Some(i) = app.faces.iter().position(|f| f.key.path == p) {
                    app.selection.clear();
                    app.selection.insert(i);
                    app.focus = Some(i);
                    app.scroll_to_focus = true;
                }
            }
        }
    });
}

fn system_tab(app: &mut FontApp, ui: &mut egui::Ui) {
    let pal = app.pal;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(I::SEARCH).font(icon_font(13.0)).color(pal.muted));
        ui.add(egui::TextEdit::singleline(&mut app.family_filter).hint_text("Filter families").desired_width(f32::INFINITY));
    });
    ui.add_space(4.0);
    let fams = app.installed_families();
    let q = app.family_filter.to_lowercase();
    let fams: Vec<_> = fams.into_iter().filter(|(n, _, _)| q.is_empty() || n.to_lowercase().contains(&q)).collect();
    let row_h = 22.0;
    egui::ScrollArea::vertical().id_salt("sysfonts").auto_shrink([false, false]).show_rows(ui, row_h, fams.len(), |ui, range| {
        for (name, count, ot) in &fams[range] {
            let src = Source::Family(name.clone());
            let selected = app.source == src;
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), row_h), Sense::click());
            if selected {
                ui.painter().rect_filled(rect, 3.0, pal.accent_soft);
            } else if resp.hovered() {
                ui.painter().rect_filled(rect, 3.0, pal.hover.gamma_multiply(0.6));
            }
            let fg = if *ot { pal.accent_text } else { pal.muted };
            badge(ui, &pal, rect.left_center() + Vec2::new(4.0, -7.5), if *ot { "O" } else { "T" }, Color32::TRANSPARENT, fg);
            ui.painter().with_clip_rect(rect.shrink2(Vec2::new(30.0, 0.0))).text(
                rect.left_center() + Vec2::new(26.0, 0.0),
                Align2::LEFT_CENTER,
                name,
                egui::FontId::proportional(12.5),
                pal.text,
            );
            ui.painter().text(rect.right_center() - Vec2::new(6.0, 0.0), Align2::RIGHT_CENTER, count.to_string(), egui::FontId::proportional(11.0), pal.faint);
            if resp.clicked() {
                app.set_source(src);
            }
        }
    });
}

pub fn go_folder(app: &mut FontApp, p: PathBuf) {
    if let Source::Folder(cur) = &app.source {
        if *cur != p {
            app.folder_back.push(cur.clone());
            app.folder_fwd.clear();
        }
    }
    // Expand every ancestor so the tree shows the folder.
    let mut a = p.parent();
    while let Some(x) = a {
        app.folder_open.insert(x.to_path_buf());
        a = x.parent();
    }
    app.folder_path_edit = p.display().to_string();
    app.cfg.last_folder = Some(p.clone());
    app.set_source(Source::Folder(p));
}

fn folders_tab(app: &mut FontApp, ui: &mut egui::Ui) {
    let pal = app.pal;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        if icon_button_sized(ui, &pal, I::ARROW_LEFT, "Back", false, !app.folder_back.is_empty()).clicked() {
            if let Some(p) = app.folder_back.pop() {
                if let Source::Folder(cur) = &app.source {
                    app.folder_fwd.push(cur.clone());
                }
                app.folder_path_edit = p.display().to_string();
                app.set_source(Source::Folder(p));
            }
        }
        if icon_button_sized(ui, &pal, I::ARROW_RIGHT, "Forward", false, !app.folder_fwd.is_empty()).clicked() {
            if let Some(p) = app.folder_fwd.pop() {
                if let Source::Folder(cur) = &app.source {
                    app.folder_back.push(cur.clone());
                }
                app.folder_path_edit = p.display().to_string();
                app.set_source(Source::Folder(p));
            }
        }
        let parent = if let Source::Folder(cur) = &app.source { cur.parent().map(|p| p.to_path_buf()) } else { None };
        if icon_button_sized(ui, &pal, I::ARROW_UP, "Up one level", false, parent.is_some()).clicked() {
            if let Some(p) = parent {
                go_folder(app, p);
            }
        }
        let r = ui.add(egui::TextEdit::singleline(&mut app.folder_path_edit).hint_text("Go to folder…").desired_width(f32::INFINITY));
        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            let p = PathBuf::from(app.folder_path_edit.trim());
            if p.is_dir() {
                go_folder(app, p);
            }
        }
    });
    ui.add_space(4.0);
    egui::ScrollArea::both().id_salt("folders").auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        let mut quick: Vec<(PathBuf, &str)> = vec![(fontdb::windows_fonts_dir(), "Windows Fonts"), (fontdb::user_fonts_dir(), "My Fonts")];
        quick.retain(|(p, _)| p.is_dir());
        for (p, label) in quick {
            let selected = app.source == Source::Folder(p.clone());
            if tree_row(ui, &pal, 0, None, I::TYPE, label, None, selected).0.clicked() {
                go_folder(app, p);
            }
        }
        ui.add_space(4.0);
        for d in fontdb::drives() {
            folder_node(app, ui, d, 0);
        }
    });
}

fn folder_node(app: &mut FontApp, ui: &mut egui::Ui, p: PathBuf, depth: usize) {
    let pal = app.pal;
    let open = app.folder_open.contains(&p);
    let name = if depth == 0 {
        p.display().to_string()
    } else {
        p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
    };
    let selected = app.source == Source::Folder(p.clone());
    let icon = if depth == 0 { I::HARD_DRIVE } else if open { I::FOLDER_OPEN } else { I::FOLDER };
    let (resp, toggled) = tree_row(ui, &pal, depth, Some(open), icon, &name, None, selected);
    if toggled {
        if open {
            app.folder_open.remove(&p);
        } else {
            app.folder_open.insert(p.clone());
        }
    } else if resp.clicked() {
        app.folder_open.insert(p.clone());
        go_folder(app, p.clone());
    }
    if selected && app.scroll_to_folder() {
        resp.scroll_to_me(Some(Align::Center));
    }
    if app.folder_open.contains(&p) {
        let kids = app.folder_children.entry(p.clone()).or_insert_with(|| fontdb::list_subdirs(&p)).clone();
        for k in kids {
            folder_node(app, ui, k, depth + 1);
        }
    }
}

impl FontApp {
    fn scroll_to_folder(&mut self) -> bool {
        false
    }
}

// --------------------------------------------------------------------------------------------
// Right dock: Task panel and Properties

pub fn right_panel(app: &mut FontApp, ui: &mut egui::Ui) {
    let pal = app.pal;
    Panel::right("rightdock")
        .default_size(300.0)
        .size_range(240.0..=520.0)
        .resizable(true)
        .frame(Frame::new().fill(pal.panel))
        .show(ui, |ui| {
            dock_header(ui, &pal, |ui| {
                tab_strip(ui, &pal, &mut app.right_tab, &[(RightTab::Tasks, "Task Panel"), (RightTab::Properties, "Properties")]);
            });
            let inner = ui.available_rect_before_wrap().shrink2(Vec2::new(6.0, 6.0));
            ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
                egui::ScrollArea::vertical().id_salt(("right", app.right_tab as u8)).auto_shrink([false, false]).show(ui, |ui| {
                    match app.right_tab {
                        RightTab::Tasks => tasks(app, ui),
                        RightTab::Properties => properties(app, ui),
                    }
                });
            });
        });
}

fn tasks(app: &mut FontApp, ui: &mut egui::Ui) {
    let pal = app.pal;
    let ctx = ui.ctx().clone();
    let has_sel = !app.selected_ids().is_empty();
    ui.spacing_mut().item_spacing.y = 1.0;
    section(ui, &pal, "sec_common", "Common Tasks", true, |ui| {
        if task_row(ui, &pal, I::MONITOR, None, "Show Installed Fonts", true).clicked() {
            app.set_source(Source::Installed);
        }
        if task_row(ui, &pal, I::ZAP, None, "Show Activated Fonts", true).clicked() {
            app.set_source(Source::Activated);
        }
        if task_row(ui, &pal, I::LIBRARY, None, "Add Fonts to Library", true).clicked() {
            app.add_folder_to_library();
        }
        if task_row(ui, &pal, I::FOLDER_SEARCH, None, "Find Fonts", true).clicked() {
            app.find.open = true;
        }
        if task_row(ui, &pal, I::COPY_CHECK, None, "Search for Duplicates", true).clicked() {
            app.dups.open = true;
        }
    });
    section(ui, &pal, "sec_selected", "Selected Fonts", true, |ui| {
        if task_row(ui, &pal, I::CIRCLE, Some(if has_sel { pal.green } else { pal.faint }), "Install these Fonts", has_sel).clicked() {
            app.install_selected(&ctx);
        }
        if task_row(ui, &pal, I::CIRCLE, Some(if has_sel { pal.orange } else { pal.faint }), "Activate Temporarily", has_sel).clicked() {
            app.activate_selected(&ctx);
        }
        if task_row(ui, &pal, I::CIRCLE, Some(if has_sel { pal.red } else { pal.faint }), "Uninstall or Deactivate these Fonts", has_sel).clicked() {
            app.request_uninstall();
        }
        if task_row(ui, &pal, I::LAYERS, None, "Add to Groups…", has_sel).clicked() {
            app.modal = Modal::AddToGroups { checked: Default::default(), new_name: String::new() };
        }
        if task_row(ui, &pal, I::GIT_COMPARE, None, "Compare Selected Fonts", app.selected_ids().len() >= 2).clicked() {
            app.compare_ids = app.selected_ids().into_iter().take(4).collect();
            app.compare_open = true;
        }
        if task_row(ui, &pal, I::COPY, None, "Copy Font Files to Folder…", has_sel).clicked() {
            app.copy_selected_to_folder(&ctx);
        }
        if task_row(ui, &pal, I::FILE_CODE, None, "Export Selected Fonts to HTML Page", has_sel).clicked() {
            app.export_html(&ctx);
        }
        if task_row(ui, &pal, I::EXTERNAL_LINK, None, "Show in Explorer", app.focus.is_some()).clicked() {
            if let Some(i) = app.focus {
                platform::reveal(&app.faces[i].key.path);
            }
        }
    });
    section(ui, &pal, "sec_custom", "Customize Program", true, |ui| {
        if task_row(ui, &pal, I::SETTINGS, None, "Program Options", true).clicked() {
            app.options_open = true;
        }
        let (icon, label, next) = if pal.dark {
            (I::SUN, "Switch to Light Theme", ThemeMode::Light)
        } else {
            (I::MOON, "Switch to Dark Theme", ThemeMode::Dark)
        };
        if task_row(ui, &pal, icon, None, label, true).clicked() {
            app.set_theme(&ctx, next);
        }
    });
    section(ui, &pal, "sec_details", "Details", true, |ui| {
        let (perm, temp) = app.counts();
        ui.add_space(2.0);
        let frame = Frame::new().fill(pal.deep).corner_radius(CornerRadius::same(6)).inner_margin(Margin::same(10));
        frame.show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new("Fonts Information").font(bold(12.5)).color(pal.text));
            ui.add_space(4.0);
            kv(ui, &pal, "Permanently installed", &perm.to_string());
            kv(ui, &pal, "Activated temporarily", &temp.to_string());
            kv(ui, &pal, "Total", &(perm + temp).to_string());
            kv(ui, &pal, "Fonts known to Fontabulous", &app.faces.len().to_string());
            if let Some(i) = app.focus {
                let f = &app.faces[i];
                ui.add_space(8.0);
                ui.label(egui::RichText::new(f.display_name()).font(bold(12.5)).color(pal.accent_text));
                kv(ui, &pal, "Style", &format!("{} · {}", weight_name(f.weight), if f.italic { "Italic" } else { "Upright" }));
                kv(ui, &pal, "Type", f.kind);
                kv(ui, &pal, "Glyphs", &f.glyph_count.to_string());
                kv(ui, &pal, "File size", &human_size(f.file_size));
            }
        });
    });
}

fn kv(ui: &mut egui::Ui, pal: &crate::theme::Palette, k: &str, v: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(k).size(12.0).color(pal.muted));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(egui::RichText::new(v).size(12.0).color(pal.text));
        });
    });
}

fn prop_row(ui: &mut egui::Ui, pal: &crate::theme::Palette, k: &str, v: &str) {
    let w = ui.available_width();
    let kw = (w * 0.42).min(150.0);
    let galley = ui.painter().layout(v.to_string(), egui::FontId::proportional(12.0), pal.text, w - kw - 8.0);
    let h = galley.size().y.max(16.0) + 6.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, 3.0, pal.hover.gamma_multiply(0.4));
    }
    ui.painter().with_clip_rect(egui::Rect::from_min_size(rect.min, Vec2::new(kw - 4.0, h))).text(
        rect.left_top() + Vec2::new(4.0, 3.0),
        Align2::LEFT_TOP,
        k,
        egui::FontId::proportional(12.0),
        pal.muted,
    );
    ui.painter().galley(rect.left_top() + Vec2::new(kw, 3.0), galley, pal.text);
    ui.painter().line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(0.5, pal.border));
    let resp = resp.on_hover_text("Click to copy");
    if resp.clicked() {
        ui.ctx().copy_text(v.to_string());
    }
}

fn properties(app: &mut FontApp, ui: &mut egui::Ui) {
    let pal = app.pal;
    let Some(i) = app.focus else {
        ui.add_space(12.0);
        ui.label(egui::RichText::new("Select a font to see its properties.").color(pal.faint));
        return;
    };
    let f = app.faces[i].clone();
    let st = app.status(&f);
    ui.spacing_mut().item_spacing.y = 0.0;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(f.display_name()).font(bold(14.0)).color(pal.text));
    });
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
        dot(ui, r.center(), 4.0, app.status_color(st));
        ui.label(egui::RichText::new(match st {
            Status::Installed => "Installed",
            Status::Activated => "Activated temporarily",
            Status::NotInstalled => "Not installed",
        }).size(12.0).color(pal.muted));
    });
    ui.add_space(6.0);
    let yn = |b: bool| if b { "Yes" } else { "No" }.to_string();
    section(ui, &pal, "p_generic", "Generic Properties", true, |ui| {
        prop_row(ui, &pal, "File Name", &f.file_name);
        prop_row(ui, &pal, "Description", f.kind);
        prop_row(ui, &pal, "File Size", &format!("{} ({} bytes)", human_size(f.file_size), f.file_size));
        prop_row(ui, &pal, "File Location", &f.folder.display().to_string());
        prop_row(ui, &pal, "Modified", &fmt_time(f.modified));
        if f.key.index > 0 {
            prop_row(ui, &pal, "Face Index", &f.key.index.to_string());
        }
    });
    section(ui, &pal, "p_names", "Names", true, |ui| {
        for (k, v) in [
            ("Family", &f.family),
            ("Subfamily", &f.subfamily),
            ("Full Name", &f.full_name),
            ("Unique Identifier", &f.unique_id),
            ("Version String", &f.version),
            ("PostScript Name", &f.postscript),
            ("Typographic Family", &f.typo_family),
            ("Typographic Subfamily", &f.typo_subfamily),
            ("Copyright", &f.copyright),
            ("Trademark", &f.trademark),
            ("Manufacturer Name", &f.manufacturer),
            ("Designer", &f.designer),
            ("Description", &f.description),
            ("License Description", &f.license),
            ("Vendor URL", &f.vendor_url),
        ] {
            if !v.is_empty() {
                prop_row(ui, &pal, k, v);
            }
        }
    });
    section(ui, &pal, "p_style", "Style & Features", true, |ui| {
        prop_row(ui, &pal, "OpenType", &yn(f.opentype_layout || f.is_postscript()));
        prop_row(ui, &pal, "Weight", &format!("{} ({})", weight_name(f.weight), f.weight));
        prop_row(ui, &pal, "Width", width_name(f.width));
        prop_row(ui, &pal, "Italic", &yn(f.italic));
        prop_row(ui, &pal, "Monospaced", &yn(f.monospaced));
        prop_row(ui, &pal, "Kerning", &yn(f.kerning));
        prop_row(ui, &pal, "Unicode", &yn(f.unicode));
        prop_row(ui, &pal, "Symbol", &yn(f.symbol));
        prop_row(ui, &pal, "Variable Font", &yn(f.variable));
        prop_row(ui, &pal, "Embedding Rights", f.embedding);
        prop_row(ui, &pal, "Family Class (IBM)", family_class_name(f.family_class));
        prop_row(ui, &pal, "Vendor ID", &f.vendor_id);
    });
    section(ui, &pal, "p_metrics", "Metrics", false, |ui| {
        prop_row(ui, &pal, "Glyphs", &f.glyph_count.to_string());
        prop_row(ui, &pal, "Units per Em", &f.units_per_em.to_string());
        prop_row(ui, &pal, "Ascender", &f.ascender.to_string());
        prop_row(ui, &pal, "Descender", &f.descender.to_string());
        prop_row(ui, &pal, "Line Gap", &f.line_gap.to_string());
        if let Some(x) = f.x_height {
            prop_row(ui, &pal, "x-Height", &x.to_string());
        }
        if let Some(c) = f.cap_height {
            prop_row(ui, &pal, "Cap Height", &c.to_string());
        }
    });
    section(ui, &pal, "p_panose", "Panose", false, |ui| {
        prop_row(ui, &pal, "Family Kind", panose_family_name(f.panose[0]));
        prop_row(ui, &pal, "Serif Style", panose_serif_name(f.panose[1]));
        prop_row(ui, &pal, "Weight", &f.panose[2].to_string());
        prop_row(ui, &pal, "Proportion", panose_proportion_name(f.panose[3]));
        prop_row(ui, &pal, "Raw", &f.panose.iter().map(|b| b.to_string()).collect::<Vec<_>>().join(" "));
    });
    section(ui, &pal, "p_tables", "Tables", false, |ui| {
        prop_row(ui, &pal, "Tables", &f.tables.iter().map(|t| t.trim()).collect::<Vec<_>>().join(", "));
    });
}

// --------------------------------------------------------------------------------------------
// Borderless window edge resizing

pub fn resize_edges(ctx: &egui::Context) {
    if ctx.input(|i| i.viewport().maximized.unwrap_or(false)) {
        return;
    }
    let rect = ctx.content_rect();
    let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) else { return };
    let m = 5.0;
    let l = pos.x - rect.left() < m;
    let r = rect.right() - pos.x < m;
    let t = pos.y - rect.top() < m;
    let b = rect.bottom() - pos.y < m;
    use egui::viewport::ResizeDirection as D;
    let dir = match (l, r, t, b) {
        (true, _, true, _) => Some(D::NorthWest),
        (_, true, true, _) => Some(D::NorthEast),
        (true, _, _, true) => Some(D::SouthWest),
        (_, true, _, true) => Some(D::SouthEast),
        (true, ..) => Some(D::West),
        (_, true, ..) => Some(D::East),
        (_, _, true, _) => Some(D::North),
        (.., true) => Some(D::South),
        _ => None,
    };
    if let Some(d) = dir {
        let icon = match d {
            D::North | D::South => egui::CursorIcon::ResizeVertical,
            D::East | D::West => egui::CursorIcon::ResizeHorizontal,
            D::NorthWest | D::SouthEast => egui::CursorIcon::ResizeNwSe,
            _ => egui::CursorIcon::ResizeNeSw,
        };
        ctx.set_cursor_icon(icon);
        if ctx.input(|i| i.pointer.primary_pressed()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(d));
        }
    }
}
