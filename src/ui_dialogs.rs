//! Floating windows and modal prompts.

use egui::{Align2, CornerRadius, Rect, Sense, Stroke, StrokeKind, Vec2};
use std::collections::BTreeSet;

use crate::app::*;
use crate::fontdb::{self, weight_name, width_name, family_class_name};
use crate::icons as I;
use crate::platform;
use crate::render::{Job, Preview};
use crate::theme::{bold, icon_font, ThemeMode};
use crate::widgets::*;

fn window<'a>(title: &'a str, id: &'a str) -> egui::Window<'a> {
    egui::Window::new(title).id(egui::Id::new(id)).collapsible(false).resizable(true)
}

pub fn dialogs(app: &mut FontApp, ctx: &egui::Context) {
    find_window(app, ctx);
    dup_window(app, ctx);
    compare_window(app, ctx);
    options_window(app, ctx);
    about_window(app, ctx);
    kerning_window(app, ctx);
    problems_window(app, ctx);
    modal(app, ctx);
}

fn find_window(app: &mut FontApp, ctx: &egui::Context) {
    if !app.find.open {
        return;
    }
    let pal = app.pal;
    let mut open = true;
    if app.find.roots.is_empty() {
        app.find.roots = fontdb::drives().into_iter().map(|d| (d, false)).collect();
    }
    window(&format!("{}  Find Fonts", I::FOLDER_SEARCH), "find").open(&mut open).default_width(420.0).show(ctx, |ui| {
        ui.label(egui::RichText::new("Font name contains all the words").size(12.0).color(pal.muted));
        ui.add(egui::TextEdit::singleline(&mut app.find.words).hint_text("Leave empty to list every font").desired_width(f32::INFINITY));
        ui.add_space(6.0);
        ui.label(egui::RichText::new("Font file types").size(12.0).color(pal.muted));
        ui.horizontal(|ui| {
            for (i, l) in ["TrueType (.ttf)", "OpenType (.otf)", "Collections (.ttc)", ".otc"].iter().enumerate() {
                ui.checkbox(&mut app.find.types[i], *l);
            }
        });
        ui.add_space(6.0);
        ui.label(egui::RichText::new("Search in").size(12.0).color(pal.muted));
        egui::Frame::new().fill(pal.deep).corner_radius(CornerRadius::same(5)).inner_margin(egui::Margin::same(6)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height(90.0);
            let mut remove = None;
            for (i, (p, on)) in app.find.roots.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.checkbox(on, "");
                    ui.label(egui::RichText::new(if p.parent().is_none() { I::HARD_DRIVE } else { I::FOLDER }).font(icon_font(13.0)).color(pal.muted));
                    ui.label(p.display().to_string());
                    if p.parent().is_some() && ui.small_button(I::X).clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                app.find.roots.remove(i);
            }
        });
        ui.horizontal(|ui| {
            if ui.button(format!("{}  Add Folder…", I::FOLDER_PLUS)).clicked() {
                if let Some(d) = rfd::FileDialog::new().pick_folder() {
                    app.find.roots.push((d, true));
                }
            }
        });
        ui.checkbox(&mut app.find.subfolders, "Include subfolders");
        ui.checkbox(&mut app.find.windows_fonts, "Search in Windows Fonts folder");
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if icon_text_button(ui, &pal, I::SEARCH, "Start Search", true).clicked() {
                app.start_find();
            }
            if app.scanner.busy {
                ui.add(egui::Spinner::new().color(pal.accent));
                ui.label(egui::RichText::new("Searching… results appear in the main view as they are found").size(11.5).color(pal.muted));
            } else if matches!(app.source, Source::Results(ref t) if t == "find") {
                ui.label(egui::RichText::new(format!("{} fonts found", app.view.len())).size(12.0).color(pal.muted));
            }
        });
    });
    app.find.open = open;
}

fn dup_window(app: &mut FontApp, ctx: &egui::Context) {
    if !app.dups.open {
        return;
    }
    let pal = app.pal;
    let mut open = true;
    window(&format!("{}  Search for Duplicates", I::COPY_CHECK), "dups").open(&mut open).default_size([560.0, 460.0]).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Look in").size(12.0).color(pal.muted));
            ui.radio_value(&mut app.dups.all_known, false, "Current view");
            ui.radio_value(&mut app.dups.all_known, true, "All known fonts");
        });
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Match fonts by").size(12.0).color(pal.muted));
            ui.checkbox(&mut app.dups.by_name, "Font name");
            ui.checkbox(&mut app.dups.by_size, "File size");
            ui.checkbox(&mut app.dups.by_content, "File contents");
        });
        ui.horizontal(|ui| {
            if icon_text_button(ui, &pal, I::SCAN_SEARCH, "Start Search", true).clicked() {
                app.run_duplicates();
            }
            if app.dups.ran {
                ui.label(egui::RichText::new(format!("{} sets of duplicates", app.dups.groups.len())).color(pal.muted));
            }
        });
        ui.separator();
        let groups = app.dups.groups.clone();
        egui::ScrollArea::vertical().max_height(ui.available_height() - 40.0).auto_shrink([false, false]).show(ui, |ui| {
            if app.dups.ran && groups.is_empty() {
                ui.label(egui::RichText::new("No duplicates found.").color(pal.faint));
            }
            for g in &groups {
                ui.label(egui::RichText::new(app.faces[g[0]].display_name()).font(bold(12.5)).color(pal.text));
                for &i in g {
                    let f = app.faces[i].clone();
                    let st = app.status(&f);
                    ui.horizontal(|ui| {
                        let mut on = app.dups.checked.contains(&i);
                        if ui.checkbox(&mut on, "").changed() {
                            if on {
                                app.dups.checked.insert(i);
                            } else {
                                app.dups.checked.remove(&i);
                            }
                        }
                        let (r, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                        dot(ui, r.center(), 3.5, app.status_color(st));
                        ui.label(egui::RichText::new(f.key.path.display().to_string()).size(12.0));
                        ui.label(egui::RichText::new(format!("{} · {}", human_size(f.file_size), f.version)).size(11.0).color(pal.muted));
                    });
                }
                ui.add_space(4.0);
            }
        });
        ui.horizontal(|ui| {
            let can = !app.dups.groups.is_empty();
            if ui.add_enabled(can, egui::Button::new(format!("{}  Show all in main view", I::EYE))).clicked() {
                app.results = app.dups.groups.iter().flatten().map(|&i| app.faces[i].key.clone()).collect();
                app.set_source(Source::Results("dups".into()));
                app.cfg.sort = SortBy::FontName;
            }
            if ui.add_enabled(!app.dups.checked.is_empty(), egui::Button::new(format!("{}  Select checked fonts", I::SQUARE_CHECK_BIG))).clicked() {
                app.results = app.dups.groups.iter().flatten().map(|&i| app.faces[i].key.clone()).collect();
                app.set_source(Source::Results("dups".into()));
                app.rebuild_view();
                app.selection = app.dups.checked.iter().copied().collect();
                app.focus = app.selection.iter().next().copied();
            }
        });
    });
    app.dups.open = open;
}

fn compare_window(app: &mut FontApp, ctx: &egui::Context) {
    if !app.compare_open {
        return;
    }
    let pal = app.pal;
    let mut open = true;
    let ids: Vec<usize> = app.compare_ids.iter().copied().filter(|&i| i < app.faces.len()).collect();
    window(&format!("{}  Compare", I::GIT_COMPARE), "compare").open(&mut open).default_size([900.0, 620.0]).show(ctx, |ui| {
        if ids.len() < 2 {
            ui.label("Select two to four fonts, then choose Compare.");
            return;
        }
        let faces: Vec<_> = ids.iter().map(|&i| app.faces[i].clone()).collect();
        let yn = |b: bool| if b { "Yes" } else { "No" }.to_string();
        let rows: Vec<(&str, Vec<String>)> = vec![
            ("File Name", faces.iter().map(|f| f.file_name.clone()).collect()),
            ("File Size", faces.iter().map(|f| f.file_size.to_string()).collect()),
            ("Location", faces.iter().map(|f| f.folder.display().to_string()).collect()),
            ("Full Font Name", faces.iter().map(|f| f.full_name.clone()).collect()),
            ("Type", faces.iter().map(|f| f.kind.to_string()).collect()),
            ("Family", faces.iter().map(|f| f.family.clone()).collect()),
            ("Subfamily", faces.iter().map(|f| f.subfamily.clone()).collect()),
            ("Version String", faces.iter().map(|f| f.version.clone()).collect()),
            ("PostScript Name", faces.iter().map(|f| f.postscript.clone()).collect()),
            ("Embedding", faces.iter().map(|f| f.embedding.to_string()).collect()),
            ("Vendor ID", faces.iter().map(|f| f.vendor_id.clone()).collect()),
            ("Glyphs", faces.iter().map(|f| f.glyph_count.to_string()).collect()),
            ("Weight", faces.iter().map(|f| format!("{} ({})", weight_name(f.weight), f.weight)).collect()),
            ("Width", faces.iter().map(|f| width_name(f.width).to_string()).collect()),
            ("Italic", faces.iter().map(|f| yn(f.italic)).collect()),
            ("Kerning", faces.iter().map(|f| yn(f.kerning)).collect()),
            ("Unicode", faces.iter().map(|f| yn(f.unicode)).collect()),
            ("Monospaced", faces.iter().map(|f| yn(f.monospaced)).collect()),
            ("Family Class", faces.iter().map(|f| family_class_name(f.family_class).to_string()).collect()),
            ("Tables", faces.iter().map(|f| f.tables.len().to_string()).collect()),
        ];
        let col_w = ((ui.available_width() - 150.0) / faces.len() as f32).max(120.0);
        egui::ScrollArea::vertical().id_salt("cmp_props").max_height(ui.available_height() * 0.5).show(ui, |ui| {
            for (k, vals) in &rows {
                let differ = vals.iter().any(|v| v != &vals[0]);
                let (rect, _) = ui.allocate_exact_size(Vec2::new(150.0 + col_w * faces.len() as f32, 20.0), Sense::hover());
                ui.painter().text(rect.left_center() + Vec2::new(4.0, 0.0), Align2::LEFT_CENTER, *k, egui::FontId::proportional(12.0), pal.muted);
                for (i, v) in vals.iter().enumerate() {
                    let r = Rect::from_min_size(egui::pos2(rect.left() + 150.0 + col_w * i as f32, rect.top()), Vec2::new(col_w - 4.0, 20.0));
                    let fill = if differ { pal.accent_soft } else { pal.green.gamma_multiply(if pal.dark { 0.15 } else { 0.12 }) };
                    ui.painter().rect_filled(r, CornerRadius::same(3), fill);
                    ui.painter().with_clip_rect(r).text(r.left_center() + Vec2::new(4.0, 0.0), Align2::LEFT_CENTER, v, egui::FontId::proportional(12.0), pal.text);
                }
            }
        });
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Characters").size(12.0).color(pal.muted));
            ui.add(egui::TextEdit::singleline(&mut app.compare_text).desired_width(120.0));
            if let Some(c) = app.compare_text.chars().next() {
                ui.label(egui::RichText::new(format!("U+{:04X}", c as u32)).size(12.0).color(pal.muted));
            }
        });
        let pp = ctx.pixels_per_point();
        let h = ui.available_height().max(120.0);
        ui.horizontal(|ui| {
            for f in &faces {
                let (rect, _) = ui.allocate_exact_size(Vec2::new(col_w + 150.0 / faces.len() as f32 - 6.0, h), Sense::hover());
                ui.painter().rect_filled(rect, CornerRadius::same(6), pal.card);
                ui.painter().rect_stroke(rect, CornerRadius::same(6), Stroke::new(1.0, pal.border), StrokeKind::Inside);
                ui.painter().text(rect.left_top() + Vec2::new(8.0, 6.0), Align2::LEFT_TOP, f.display_name(), egui::FontId::proportional(11.5), pal.muted);
                let inner = rect.shrink2(Vec2::new(8.0, 24.0)).translate(Vec2::new(0.0, 8.0));
                let side = inner.width().min(inner.height());
                let job = Job {
                    face: f.key.clone(),
                    text: app.compare_text.clone(),
                    px: (side * 0.6 * pp) as u32,
                    wrap: 0,
                    max_w: (side * pp) as u32,
                    center: true,
                };
                if let Preview::Ready(tex, size) = app.renderer.get(job) {
                    let r = Rect::from_center_size(inner.center(), size);
                    ui.painter().image(tex, r, Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), app.preview_tint());
                }
            }
        });
    });
    app.compare_open = open;
}

fn options_window(app: &mut FontApp, ctx: &egui::Context) {
    if !app.options_open {
        return;
    }
    let pal = app.pal;
    let mut open = true;
    let mut theme_change = None;
    window(&format!("{}  Options", I::SETTINGS), "options").open(&mut open).default_width(460.0).show(ctx, |ui| {
        section(ui, &pal, "opt_general", "General", true, |ui| {
            ui.horizontal(|ui| {
                ui.label("Theme");
                if ui.selectable_label(app.cfg.theme == ThemeMode::Light, format!("{}  Light", I::SUN)).clicked() {
                    theme_change = Some(ThemeMode::Light);
                }
                if ui.selectable_label(app.cfg.theme == ThemeMode::Dark, format!("{}  Dark", I::MOON)).clicked() {
                    theme_change = Some(ThemeMode::Dark);
                }
            });
        });
        section(ui, &pal, "opt_main", "Main View", true, |ui| {
            egui::Grid::new("opt_grid").num_columns(2).spacing(Vec2::new(12.0, 6.0)).show(ui, |ui| {
                ui.label("Preview size");
                ui.add(egui::Slider::new(&mut app.cfg.preview_size, 10.0..=120.0));
                ui.end_row();
                ui.label("Tile width");
                ui.add(egui::Slider::new(&mut app.cfg.tile_width, 160.0..=1000.0));
                ui.end_row();
                ui.label("Text sample size");
                ui.add(egui::Slider::new(&mut app.cfg.sample_size, 8.0..=144.0));
                ui.end_row();
                ui.label("Characters table size");
                ui.add(egui::Slider::new(&mut app.cfg.char_size, 16.0..=96.0));
                ui.end_row();
            });
        });
        section(ui, &pal, "opt_samples", "Text Samples", true, |ui| {
            let mut remove = None;
            for (i, s) in app.cfg.sample_texts.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(s).desired_width(ui.available_width() - 30.0));
                    if ui.small_button(I::TRASH_2).clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                app.cfg.sample_texts.remove(i);
            }
            if ui.button(format!("{}  Add sample", I::PLUS)).clicked() {
                app.cfg.sample_texts.push("New sample text".into());
            }
        });
        ui.add_space(6.0);
        if ui.button("Save").clicked() {
            app.mark_cfg_dirty();
        }
    });
    if let Some(t) = theme_change {
        app.set_theme(ctx, t);
    }
    if !open {
        app.mark_cfg_dirty();
    }
    app.options_open = open;
}

fn about_window(app: &mut FontApp, ctx: &egui::Context) {
    if !app.about_open {
        return;
    }
    let pal = app.pal;
    let mut open = true;
    window("About Fontabulous", "about").open(&mut open).resizable(false).default_width(380.0).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(6.0);
            let (r, _) = ui.allocate_exact_size(Vec2::new(68.0, 88.0), Sense::hover());
            paint_logo(ui.painter(), r, pal.accent);
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Fontabulous").font(bold(20.0)));
            ui.label(egui::RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION"))).color(pal.muted));
            ui.add_space(8.0);
            ui.label("Browse, preview, compare, tag and install fonts.");
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Icons: Lucide (ISC). Interface font: Inter (SIL OFL 1.1).").size(11.5).color(pal.faint));
            if ui.link("lucide.dev").clicked() {
                platform::open_url("https://lucide.dev");
            }
        });
    });
    app.about_open = open;
}

fn kerning_window(app: &mut FontApp, ctx: &egui::Context) {
    if !app.kerning_open {
        return;
    }
    let pal = app.pal;
    let Some(id) = app.focus else {
        app.kerning_open = false;
        return;
    };
    if app.kerning_cache.as_ref().map(|(i, _)| *i != id).unwrap_or(true) {
        let f = &app.faces[id];
        let pairs = app.font_bytes(&f.key.path).map(|d| fontdb::kerning_pairs(&d, f.key.index, 5000)).unwrap_or_default();
        app.kerning_cache = Some((id, pairs));
    }
    let mut open = true;
    let name = app.faces[id].display_name().to_string();
    let pairs = app.kerning_cache.as_ref().map(|(_, p)| p.clone()).unwrap_or_default();
    window(&format!("{}  Kerning Pairs — {name}", I::BASELINE), "kerning").open(&mut open).default_size([340.0, 420.0]).show(ctx, |ui| {
        if pairs.is_empty() {
            ui.label(egui::RichText::new("No legacy kerning pairs found. The font may kern through its OpenType layout (GPOS) table instead.").color(pal.muted));
            return;
        }
        ui.label(egui::RichText::new(format!("{} pairs", pairs.len())).color(pal.muted));
        egui::ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, 20.0, pairs.len(), |ui, range| {
            for (a, b, v) in &pairs[range] {
                ui.horizontal(|ui| {
                    ui.add_sized([60.0, 18.0], egui::Label::new(format!("{a}{b}")));
                    ui.add_sized([120.0, 18.0], egui::Label::new(egui::RichText::new(format!("U+{:04X} U+{:04X}", *a as u32, *b as u32)).color(pal.muted).size(11.5)));
                    ui.label(egui::RichText::new(v.to_string()).color(if *v < 0 { pal.accent_text } else { pal.green }));
                });
            }
        });
    });
    app.kerning_open = open;
}

fn problems_window(app: &mut FontApp, ctx: &egui::Context) {
    if !app.problems_open {
        return;
    }
    let pal = app.pal;
    let mut open = true;
    window(&format!("{}  Detect Font Problems", I::TRIANGLE_ALERT), "problems").open(&mut open).default_size([620.0, 400.0]).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("{} possible problems among installed fonts", app.problems.len())).color(pal.muted));
            if ui.button(format!("{}  Check again", I::REFRESH_CW)).clicked() {
                app.detect_problems();
            }
        });
        ui.separator();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if app.problems.is_empty() {
                ui.label(egui::RichText::new("No problems found.").color(pal.green));
            }
            for (kind, msg) in &app.problems {
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new(kind).font(bold(12.0)).color(pal.orange));
                    ui.label(egui::RichText::new(msg).size(12.0));
                });
            }
        });
    });
    app.problems_open = open;
}

fn modal(app: &mut FontApp, ctx: &egui::Context) {
    let pal = app.pal;
    let mut close = false;
    let mut action: Option<Box<dyn FnOnce(&mut FontApp)>> = None;
    match &mut app.modal {
        Modal::None => return,
        Modal::NewGroup { name, add_selected } => {
            egui::Modal::new(egui::Id::new("m_newgroup")).show(ctx, |ui| {
                ui.set_width(320.0);
                ui.label(egui::RichText::new("New Group").font(bold(15.0)));
                ui.add_space(6.0);
                let r = ui.add(egui::TextEdit::singleline(name).hint_text("Group name").desired_width(f32::INFINITY));
                r.request_focus();
                ui.checkbox(add_selected, "Add the selected fonts to it");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let ok = !name.trim().is_empty();
                    if (ui.add_enabled(ok, egui::Button::new("Create")).clicked() || (ok && ui.input(|i| i.key_pressed(egui::Key::Enter)))) && ok {
                        let n = name.trim().to_string();
                        let add = *add_selected;
                        action = Some(Box::new(move |app: &mut FontApp| {
                            app.cfg.groups.entry(n.clone()).or_default();
                            if add {
                                app.add_selected_to_groups(&[n.clone()].into_iter().collect());
                            }
                            app.mark_cfg_dirty();
                            app.set_source(Source::Group(n));
                        }));
                        close = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            });
        }
        Modal::RenameGroup { old, name } => {
            egui::Modal::new(egui::Id::new("m_rename")).show(ctx, |ui| {
                ui.set_width(320.0);
                ui.label(egui::RichText::new("Rename Group").font(bold(15.0)));
                ui.add(egui::TextEdit::singleline(name).desired_width(f32::INFINITY));
                ui.horizontal(|ui| {
                    if ui.button("Rename").clicked() && !name.trim().is_empty() {
                        let (o, n) = (old.clone(), name.trim().to_string());
                        action = Some(Box::new(move |app: &mut FontApp| {
                            if let Some(v) = app.cfg.groups.remove(&o) {
                                app.cfg.groups.insert(n.clone(), v);
                            }
                            app.mark_cfg_dirty();
                            app.set_source(Source::Group(n));
                        }));
                        close = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            });
        }
        Modal::AddToGroups { checked, new_name } => {
            let groups: Vec<String> = app.cfg.groups.keys().cloned().collect();
            egui::Modal::new(egui::Id::new("m_addgroups")).show(ctx, |ui| {
                ui.set_width(340.0);
                ui.label(egui::RichText::new("Add to Groups").font(bold(15.0)));
                ui.add_space(4.0);
                egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                    if groups.is_empty() {
                        ui.label(egui::RichText::new("No groups yet — type a name below.").color(pal.faint));
                    }
                    for g in &groups {
                        let mut on = checked.contains(g);
                        if ui.checkbox(&mut on, g).changed() {
                            if on {
                                checked.insert(g.clone());
                            } else {
                                checked.remove(g);
                            }
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("New group");
                    ui.add(egui::TextEdit::singleline(new_name).desired_width(f32::INFINITY));
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("Add").clicked() {
                        let mut set: BTreeSet<String> = checked.clone();
                        if !new_name.trim().is_empty() {
                            set.insert(new_name.trim().to_string());
                        }
                        action = Some(Box::new(move |app: &mut FontApp| {
                            let ctx2 = set.len();
                            app.add_selected_to_groups(&set);
                            let _ = ctx2;
                        }));
                        close = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            });
        }
        Modal::ConfirmUninstall(ids) => {
            let ids2 = ids.clone();
            let names: Vec<String> = ids.iter().take(8).map(|&i| app.faces[i].display_name().to_string()).collect();
            egui::Modal::new(egui::Id::new("m_uninst")).show(ctx, |ui| {
                ui.set_width(380.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(I::TRIANGLE_ALERT).font(icon_font(18.0)).color(pal.orange));
                    ui.label(egui::RichText::new("Uninstall or deactivate fonts?").font(bold(15.0)));
                });
                ui.add_space(4.0);
                ui.label(format!("{} font(s) will be removed from Windows. Temporarily activated fonts are just deactivated; fonts you installed for yourself are deleted from your Fonts folder.", ids2.len()));
                ui.add_space(4.0);
                for n in &names {
                    ui.label(egui::RichText::new(format!("•  {n}")).color(pal.muted));
                }
                if ids2.len() > names.len() {
                    ui.label(egui::RichText::new(format!("…and {} more", ids2.len() - names.len())).color(pal.muted));
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if icon_text_button(ui, &pal, I::POWER_OFF, "Remove", true).clicked() {
                        let ctx2 = ctx.clone();
                        action = Some(Box::new(move |app: &mut FontApp| app.uninstall(&ctx2, ids2)));
                        close = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            });
        }
        Modal::Message { title, body } => {
            egui::Modal::new(egui::Id::new("m_msg")).show(ctx, |ui| {
                ui.set_width(380.0);
                ui.label(egui::RichText::new(title.as_str()).font(bold(15.0)));
                ui.add_space(4.0);
                ui.label(body.as_str());
                ui.add_space(8.0);
                if ui.button("OK").clicked() {
                    close = true;
                }
            });
        }
    }
    if close {
        app.modal = Modal::None;
    }
    if let Some(a) = action {
        a(app);
    }
}
