//! The centre of the window: font view toolbars, tiles/list/details views and the
//! sample dock (Text Sample, Characters Table, Waterfall, Tags, Search).

use egui::{Align, Align2, Color32, CornerRadius, Frame, Layout, Margin, Panel, Rect, Sense, Stroke, StrokeKind, Vec2};

use crate::app::*;
use crate::fontdb::{weight_badge, FontFace};
use crate::icons as I;
use crate::platform;
use crate::render::{Job, Preview};
use crate::theme::{bold, icon_font, Palette};
use crate::widgets::*;

fn ppp(ui: &egui::Ui) -> f32 {
    ui.ctx().pixels_per_point()
}

/// Draw a rendered preview texture inside `rect` (left/centre aligned), tinted.
fn draw_preview(ui: &egui::Ui, pal: &Palette, p: Preview, rect: Rect, tint: Color32, center: bool) {
    match p {
        Preview::Ready(tex, size) => {
            let mut s = size;
            if s.x > rect.width() || s.y > rect.height() {
                // Clip instead of scaling so text stays crisp.
                s = Vec2::new(s.x.min(rect.width()), s.y.min(rect.height()));
            }
            let pos = if center {
                rect.center() - s / 2.0
            } else {
                egui::pos2(rect.left(), rect.center().y - s.y / 2.0)
            };
            let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(s.x / size.x.max(1.0), s.y / size.y.max(1.0)));
            ui.painter().with_clip_rect(rect).image(tex, Rect::from_min_size(pos, s), uv, tint);
        }
        Preview::Pending => {
            let r = Rect::from_min_size(egui::pos2(rect.left(), rect.center().y - 3.0), Vec2::new((rect.width() * 0.35).min(160.0), 6.0));
            ui.painter().rect_filled(r, CornerRadius::same(3), pal.raised);
        }
        Preview::Failed => {
            ui.painter().text(rect.left_center(), Align2::LEFT_CENTER, "Preview not available", egui::FontId::proportional(11.5), pal.faint);
        }
    }
}

fn job(f: &FontFace, text: String, px_pts: f32, max_w_pts: f32, ppp: f32) -> Job {
    Job {
        face: f.key.clone(),
        text,
        px: (px_pts * ppp).round().max(4.0) as u32,
        wrap: 0,
        max_w: (max_w_pts * ppp).round().max(8.0) as u32,
        center: false,
    }
}

// --------------------------------------------------------------------------------------------

pub fn main_view(app: &mut FontApp, ui: &mut egui::Ui) {
    let pal = app.pal;
    Panel::top("view_toolbar")
        .exact_size(40.0)
        .frame(Frame::new().fill(pal.panel).inner_margin(Margin::symmetric(8, 0)))
        .show(ui, |ui| view_toolbar(app, ui));
    Panel::top("action_bar")
        .exact_size(36.0)
        .frame(Frame::new().fill(pal.panel).inner_margin(Margin::symmetric(8, 0)))
        .show(ui, |ui| action_bar(app, ui));
    egui::CentralPanel::default()
        .frame(Frame::new().fill(pal.canvas).inner_margin(Margin::same(10)))
        .show(ui, |ui| {
            if app.view.is_empty() {
                empty_state(app, ui);
                return;
            }
            match app.cfg.view_mode {
                ViewMode::Tiles => tiles(app, ui),
                ViewMode::List => list(app, ui),
                ViewMode::Details => details(app, ui),
            }
        });
}

fn empty_state(app: &FontApp, ui: &mut egui::Ui) {
    let pal = app.pal;
    let msg = if app.scanner.busy {
        "Reading fonts…"
    } else if app.source == Source::Worklist {
        "Current worklist is empty. Add files to the worklist to see them here."
    } else {
        "There are no items to display in this view."
    };
    ui.add_space(30.0);
    ui.vertical_centered(|ui| {
        ui.label(egui::RichText::new(I::TYPE).font(icon_font(40.0)).color(pal.faint));
        ui.add_space(8.0);
        ui.label(egui::RichText::new(msg).color(pal.muted).size(14.0));
        ui.label(
            egui::RichText::new("Select a folder, collection or group on the left, or use Find Fonts.")
                .color(pal.faint)
                .size(12.0),
        );
    });
}

fn view_toolbar(app: &mut FontApp, ui: &mut egui::Ui) {
    let pal = app.pal;
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for (m, icon, tip) in [
            (ViewMode::List, I::LIST, "List: font names in their own typeface"),
            (ViewMode::Tiles, I::LAYOUT_GRID, "Tiles"),
            (ViewMode::Details, I::TABLE, "Details"),
        ] {
            if icon_button(ui, &pal, icon, tip, app.cfg.view_mode == m).clicked() {
                app.cfg.view_mode = m;
                app.mark_cfg_dirty();
            }
        }
        vsep(ui, &pal);
        let before = app.cfg.display;
        egui::ComboBox::from_id_salt("display_text")
            .selected_text(app.cfg.display.label())
            .width(110.0)
            .show_ui(ui, |ui| {
                for d in [DisplayText::FontName, DisplayText::Pangram, DisplayText::Alphabet, DisplayText::Digits, DisplayText::Custom] {
                    ui.selectable_value(&mut app.cfg.display, d, d.label());
                }
            });
        let r = ui.add(egui::TextEdit::singleline(&mut app.cfg.custom_text).hint_text("Enter your own text").desired_width(170.0));
        if r.changed() {
            app.cfg.display = if app.cfg.custom_text.is_empty() { DisplayText::FontName } else { DisplayText::Custom };
        }
        if before != app.cfg.display || r.lost_focus() {
            app.mark_cfg_dirty();
        }
        vsep(ui, &pal);
        ui.label(egui::RichText::new("A").size(11.0).color(pal.muted));
        ui.spacing_mut().slider_width = 90.0;
        let r = ui.add(egui::Slider::new(&mut app.cfg.preview_size, 10.0..=120.0).show_value(false));
        ui.label(egui::RichText::new("A").size(17.0).color(pal.muted));
        let r2 = ui.add(egui::DragValue::new(&mut app.cfg.preview_size).range(8.0..=200.0).speed(0.5).fixed_decimals(0));
        if r.drag_stopped() || r2.lost_focus() {
            app.mark_cfg_dirty();
        }
        app.cfg.preview_size = app.cfg.preview_size.round();
        vsep(ui, &pal);
        color_button(ui, &pal, "Text colour", &mut app.cfg.preview_color, pal.text, I::BASELINE);
        color_button(ui, &pal, "Background colour", &mut app.cfg.preview_bg, pal.card, I::PAINT_BUCKET);
        vsep(ui, &pal);
        if app.cfg.view_mode == ViewMode::Tiles {
            ui.label(egui::RichText::new("Width").size(12.0).color(pal.muted));
            if ui.add(egui::DragValue::new(&mut app.cfg.tile_width).range(160.0..=1200.0).speed(2.0)).lost_focus() {
                app.mark_cfg_dirty();
            }
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let icon = if app.cfg.sort_desc { I::ARROW_UP_DOWN } else { I::ARROW_DOWN_A_Z };
            if icon_button(ui, &pal, icon, if app.cfg.sort_desc { "Descending" } else { "Ascending" }, app.cfg.sort_desc).clicked() {
                app.cfg.sort_desc = !app.cfg.sort_desc;
                app.view_dirty = true;
            }
            let before = app.cfg.sort;
            egui::ComboBox::from_id_salt("sort_by").selected_text(format!("Sort: {}", app.cfg.sort.label())).width(140.0).show_ui(ui, |ui| {
                for s in SortBy::ALL {
                    ui.selectable_value(&mut app.cfg.sort, s, s.label());
                }
            });
            if before != app.cfg.sort {
                app.view_dirty = true;
                app.mark_cfg_dirty();
            }
        });
    });
}

fn color_button(ui: &mut egui::Ui, pal: &Palette, tip: &str, value: &mut Option<[u8; 3]>, default: Color32, icon: &str) {
    let current = value.map(|c| Color32::from_rgb(c[0], c[1], c[2])).unwrap_or(default);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(36.0, 26.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(5), pal.hover);
    }
    ui.painter().text(rect.left_center() + Vec2::new(11.0, -1.0), Align2::CENTER_CENTER, icon, icon_font(14.0), pal.muted);
    let sw = Rect::from_min_size(rect.left_center() + Vec2::new(21.0, -6.0), Vec2::splat(12.0));
    ui.painter().rect_filled(sw, CornerRadius::same(3), current);
    ui.painter().rect_stroke(sw, CornerRadius::same(3), Stroke::new(1.0, pal.faint), StrokeKind::Outside);
    let popup_id = ui.make_persistent_id(tip);
    let resp = resp.on_hover_text(tip);
    egui::Popup::from_toggle_button_response(&resp).id(popup_id).show(|ui| {
        let mut c = current;
        ui.label(tip);
        if egui::color_picker::color_picker_color32(ui, &mut c, egui::color_picker::Alpha::Opaque) {
            *value = Some([c.r(), c.g(), c.b()]);
        }
        if ui.button("Reset to theme colour").clicked() {
            *value = None;
        }
    });
}

fn action_bar(app: &mut FontApp, ui: &mut egui::Ui) {
    let pal = app.pal;
    let ctx = ui.ctx().clone();
    let has = !app.selected_ids().is_empty();
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let b = |ui: &mut egui::Ui, icon: &str, tip: &str, en: bool| icon_button_sized(ui, &pal, icon, tip, false, en).clicked();
        if b(ui, I::CIRCLE_CHECK, "Install selected fonts", has) {
            app.install_selected(&ctx);
        }
        if b(ui, I::ZAP, "Activate temporarily", has) {
            app.activate_selected(&ctx);
        }
        if b(ui, I::POWER_OFF, "Uninstall or deactivate", has) {
            app.request_uninstall();
        }
        vsep(ui, &pal);
        if b(ui, I::LAYERS, "Add to groups", has) {
            app.modal = Modal::AddToGroups { checked: Default::default(), new_name: String::new() };
        }
        if b(ui, I::LIBRARY, "Add to library", has) {
            app.add_selected_to_library(&ctx);
        }
        if b(ui, I::LIST_PLUS, "Add to worklist", has) {
            app.add_selected_to_worklist();
        }
        vsep(ui, &pal);
        if b(ui, I::TAG, "Edit tags", has) {
            app.bottom_tab = BottomTab::Tags;
            app.cfg.show_bottom = true;
        }
        if b(ui, I::INFO, "Properties", has) {
            app.right_tab = RightTab::Properties;
            app.cfg.show_right = true;
        }
        if b(ui, I::GIT_COMPARE, "Compare selected fonts (2–4)", app.selected_ids().len() >= 2) {
            app.compare_ids = app.selected_ids().into_iter().take(4).collect();
            app.compare_open = true;
        }
        if b(ui, I::BASELINE, "Show kerning pairs", has) {
            app.kerning_open = true;
            app.kerning_cache = None;
        }
        if b(ui, I::COPY, "Copy font name", has) {
            let names: Vec<String> = app.selected_ids().iter().map(|&i| app.faces[i].display_name().to_string()).collect();
            ctx.copy_text(names.join("\n"));
            app.toast(&ctx, "Font name copied");
        }
        if b(ui, I::EXTERNAL_LINK, "Show in Explorer", app.focus.is_some()) {
            if let Some(i) = app.focus {
                platform::reveal(&app.faces[i].key.path);
            }
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                egui::RichText::new(format!("{} fonts{}", app.view.len(), if app.selection.len() > 1 { format!(" · {} selected", app.selection.len()) } else { String::new() }))
                    .size(12.0)
                    .color(pal.muted),
            );
        });
    });
}

fn handle_item(app: &mut FontApp, ui: &egui::Ui, id: usize, resp: &egui::Response) {
    if resp.clicked() {
        let m = ui.input(|i| i.modifiers);
        app.click_select(id, m);
    }
    if resp.secondary_clicked() && !app.selection.contains(&id) {
        app.click_select(id, egui::Modifiers::NONE);
    }
    if resp.double_clicked() {
        app.right_tab = RightTab::Properties;
        app.cfg.show_right = true;
    }
    let ctx = ui.ctx().clone();
    resp.context_menu(|ui| {
        item_menu(app, ui, &ctx);
    });
}

fn item_menu(app: &mut FontApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    let b = |ui: &mut egui::Ui, icon: &str, label: &str| ui.button(format!("{icon}  {label}")).clicked();
    if b(ui, I::CIRCLE_CHECK, "Install") {
        app.install_selected(ctx);
        ui.close();
    }
    if b(ui, I::ZAP, "Activate Temporarily") {
        app.activate_selected(ctx);
        ui.close();
    }
    if b(ui, I::POWER_OFF, "Uninstall or Deactivate…") {
        app.request_uninstall();
        ui.close();
    }
    ui.separator();
    if b(ui, I::LAYERS, "Add to Groups…") {
        app.modal = Modal::AddToGroups { checked: Default::default(), new_name: String::new() };
        ui.close();
    }
    if b(ui, I::LIBRARY, "Add to Library") {
        app.add_selected_to_library(ctx);
        ui.close();
    }
    if b(ui, I::LIST_PLUS, "Add to Worklist") {
        app.add_selected_to_worklist();
        ui.close();
    }
    ui.separator();
    if b(ui, I::COPY, "Copy Font Name") {
        let names: Vec<String> = app.selected_ids().iter().map(|&i| app.faces[i].display_name().to_string()).collect();
        ctx.copy_text(names.join("\n"));
        ui.close();
    }
    if b(ui, I::CLIPBOARD, "Copy File Path") {
        let names: Vec<String> = app.selected_ids().iter().map(|&i| app.faces[i].key.path.display().to_string()).collect();
        ctx.copy_text(names.join("\n"));
        ui.close();
    }
    if b(ui, I::EXTERNAL_LINK, "Show in Explorer") {
        if let Some(i) = app.focus {
            platform::reveal(&app.faces[i].key.path);
        }
        ui.close();
    }
    ui.separator();
    if ui.add_enabled(app.selected_ids().len() >= 2, egui::Button::new(format!("{}  Compare…", I::GIT_COMPARE))).clicked() {
        app.compare_ids = app.selected_ids().into_iter().take(4).collect();
        app.compare_open = true;
        ui.close();
    }
    if b(ui, I::INFO, "Properties") {
        app.right_tab = RightTab::Properties;
        app.cfg.show_right = true;
        ui.close();
    }
}

fn header_line(app: &FontApp, ui: &egui::Ui, f: &FontFace, rect: Rect, selected: bool) {
    let pal = app.pal;
    let st = app.status(f);
    let mut x = rect.left() + 4.0;
    let ot = f.is_postscript() || f.opentype_layout;
    let tr = badge(ui, &pal, egui::pos2(x, rect.center().y - 7.5), if ot { "O" } else { "T" }, Color32::TRANSPARENT, if ot { pal.accent_text } else { pal.muted });
    x = tr.right() + 6.0;
    let name = f.display_name();
    let galley = ui.painter().layout_no_wrap(name.to_string(), bold(12.0), pal.text);
    let name_rect = Rect::from_min_size(egui::pos2(x, rect.center().y - 9.0), Vec2::new(galley.size().x + 12.0, 18.0));
    // Installed fonts get a coloured name chip (green installed, orange activated).
    match st {
        Status::Installed => {
            ui.painter().rect_filled(name_rect, CornerRadius::same(4), pal.green.gamma_multiply(if pal.dark { 0.28 } else { 0.18 }));
        }
        Status::Activated => {
            ui.painter().rect_filled(name_rect, CornerRadius::same(4), pal.orange.gamma_multiply(if pal.dark { 0.28 } else { 0.2 }));
        }
        Status::NotInstalled => {}
    }
    let clip = Rect::from_min_max(rect.min, egui::pos2(rect.right() - 34.0, rect.bottom()));
    let col = if selected { pal.accent_text } else { pal.text };
    ui.painter().with_clip_rect(clip).galley(egui::pos2(x + 6.0, rect.center().y - galley.size().y / 2.0), galley, col);
    let wb = weight_badge(f.weight, f.italic);
    if !wb.is_empty() {
        let g = ui.painter().layout_no_wrap(wb.clone(), bold(10.0), pal.muted);
        badge(ui, &pal, egui::pos2(rect.right() - g.size().x - 12.0, rect.center().y - 7.5), &wb, pal.raised, pal.muted);
    }
}

fn tiles(app: &mut FontApp, ui: &mut egui::Ui) {
    let pal = app.pal;
    let avail = ui.available_width();
    let gap = 10.0;
    let tile_w = app.cfg.tile_width.min(avail - 4.0).max(140.0);
    let cols = (((avail + gap) / (tile_w + gap)).floor() as usize).max(1);
    let size = app.cfg.preview_size;
    let preview_h = (size * 1.55 + 18.0).max(44.0);
    let tile_h = 24.0 + preview_h;
    let row_h = tile_h + gap;
    let n = app.view.len();
    let rows = n.div_ceil(cols);
    let pp = ppp(ui);
    let tint = app.preview_tint();
    let bg = app.preview_bg();
    let mut scroll = egui::ScrollArea::vertical().id_salt("tiles").auto_shrink([false, false]);
    if app.scroll_to_focus {
        if let Some(pos) = app.focus.and_then(|f| app.view.iter().position(|&x| x == f)) {
            let y = (pos / cols) as f32 * row_h;
            let vh = ui.available_height();
            let cur = ui.data(|d| d.get_temp::<f32>(egui::Id::new("tiles_off"))).unwrap_or(0.0);
            if y < cur || y + row_h > cur + vh {
                scroll = scroll.vertical_scroll_offset((y - vh / 2.0 + row_h / 2.0).max(0.0));
            }
        }
        app.scroll_to_focus = false;
    }
    let out = scroll.show_rows(ui, row_h, rows, |ui, range| {
        for r in range {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for c in 0..cols {
                    let idx = r * cols + c;
                    if idx >= n {
                        break;
                    }
                    let id = app.view[idx];
                    let (rect, resp) = ui.allocate_exact_size(Vec2::new(tile_w, tile_h), Sense::click());
                    if !ui.is_rect_visible(rect) {
                        continue;
                    }
                    let selected = app.selection.contains(&id);
                    let f = app.faces[id].clone();
                    let head = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 22.0));
                    let body = Rect::from_min_max(egui::pos2(rect.left(), rect.top() + 24.0), rect.max);
                    if selected {
                        ui.painter().rect_filled(rect.expand(3.0), CornerRadius::same(7), pal.accent_soft);
                    } else if resp.hovered() {
                        ui.painter().rect_filled(rect.expand(3.0), CornerRadius::same(7), pal.hover.gamma_multiply(0.35));
                    }
                    header_line(app, ui, &f, head, selected);
                    ui.painter().rect_filled(body, CornerRadius::same(5), bg);
                    let stroke = if selected { Stroke::new(1.5, pal.accent) } else { Stroke::new(1.0, pal.border) };
                    ui.painter().rect_stroke(body, CornerRadius::same(5), stroke, StrokeKind::Inside);
                    let inner = body.shrink2(Vec2::new(10.0, 4.0));
                    let p = app.renderer.get(job(&f, app.preview_text(&f), size, inner.width(), pp));
                    draw_preview(ui, &pal, p, inner, tint, false);
                    handle_item(app, ui, id, &resp);
                }
            });
            ui.add_space(gap - ui.spacing().item_spacing.y);
        }
    });
    ui.data_mut(|d| d.insert_temp(egui::Id::new("tiles_off"), out.state.offset.y));
}

fn list(app: &mut FontApp, ui: &mut egui::Ui) {
    let pal = app.pal;
    let size = app.cfg.preview_size;
    let row_h = (size * 1.5).max(30.0) + 8.0;
    let n = app.view.len();
    let pp = ppp(ui);
    let tint = app.preview_tint();
    let mut scroll = egui::ScrollArea::vertical().id_salt("list").auto_shrink([false, false]);
    if app.scroll_to_focus {
        if let Some(pos) = app.focus.and_then(|f| app.view.iter().position(|&x| x == f)) {
            let y = pos as f32 * row_h;
            let vh = ui.available_height();
            let cur = ui.data(|d| d.get_temp::<f32>(egui::Id::new("list_off"))).unwrap_or(0.0);
            if y < cur || y + row_h > cur + vh {
                scroll = scroll.vertical_scroll_offset((y - vh / 2.0).max(0.0));
            }
        }
        app.scroll_to_focus = false;
    }
    let out = scroll.show_rows(ui, row_h, n, |ui, range| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for idx in range {
            let id = app.view[idx];
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), row_h), Sense::click());
            let selected = app.selection.contains(&id);
            let f = app.faces[id].clone();
            if selected {
                ui.painter().rect_filled(rect, CornerRadius::same(5), pal.accent_soft);
            } else if resp.hovered() {
                ui.painter().rect_filled(rect, CornerRadius::same(5), pal.hover.gamma_multiply(0.4));
            }
            ui.painter().line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(0.5, pal.border));
            let st = app.status(&f);
            dot(ui, rect.left_center() + Vec2::new(10.0, 0.0), 4.0, app.status_color(st));
            let name_rect = Rect::from_min_max(rect.min + Vec2::new(22.0, 0.0), egui::pos2(rect.left() + 240.0, rect.bottom()));
            ui.painter().with_clip_rect(name_rect).text(
                name_rect.left_center() - Vec2::new(0.0, 7.0),
                Align2::LEFT_CENTER,
                f.display_name(),
                egui::FontId::proportional(12.5),
                if selected { pal.accent_text } else { pal.text },
            );
            ui.painter().with_clip_rect(name_rect).text(
                name_rect.left_center() + Vec2::new(0.0, 8.0),
                Align2::LEFT_CENTER,
                format!("{} · {}", f.kind, f.file_name),
                egui::FontId::proportional(11.0),
                pal.muted,
            );
            let pr = Rect::from_min_max(egui::pos2(rect.left() + 250.0, rect.top() + 2.0), rect.max - Vec2::new(8.0, 2.0));
            if pr.width() > 20.0 {
                let p = app.renderer.get(job(&f, app.preview_text(&f), size, pr.width(), pp));
                draw_preview(ui, &pal, p, pr, tint, false);
            }
            handle_item(app, ui, id, &resp);
        }
    });
    ui.data_mut(|d| d.insert_temp(egui::Id::new("list_off"), out.state.offset.y));
}

fn details(app: &mut FontApp, ui: &mut egui::Ui) {
    let pal = app.pal;
    let cols: [(&str, f32, Option<SortBy>); 8] = [
        ("File Name", 170.0, Some(SortBy::FileName)),
        ("Preview", 200.0, None),
        ("Font Name", 190.0, Some(SortBy::FontName)),
        ("Family", 150.0, Some(SortBy::Family)),
        ("Location", 220.0, Some(SortBy::Location)),
        ("Version", 110.0, None),
        ("Size", 70.0, Some(SortBy::FileSize)),
        ("File Type", 120.0, Some(SortBy::Type)),
    ];
    let total_w: f32 = cols.iter().map(|c| c.1).sum();
    let row_h = 30.0;
    let pp = ppp(ui);
    let tint = app.preview_tint();
    egui::ScrollArea::horizontal().id_salt("details_h").auto_shrink([false, false]).show(ui, |ui| {
        ui.set_min_width(total_w);
        // Header
        let (hrect, _) = ui.allocate_exact_size(Vec2::new(total_w.max(ui.available_width()), 26.0), Sense::hover());
        ui.painter().rect_filled(hrect, CornerRadius::same(4), pal.panel);
        let mut x = hrect.left();
        for (name, w, sort) in cols {
            let r = Rect::from_min_size(egui::pos2(x, hrect.top()), Vec2::new(w, 26.0));
            let resp = ui.interact(r, ui.id().with(("hdr", name)), Sense::click());
            let active = sort == Some(app.cfg.sort);
            ui.painter().text(r.left_center() + Vec2::new(8.0, 0.0), Align2::LEFT_CENTER, name, bold(11.5), if active { pal.accent_text } else { pal.muted });
            if active {
                let ic = if app.cfg.sort_desc { I::CHEVRON_DOWN } else { I::CHEVRON_UP };
                ui.painter().text(r.right_center() - Vec2::new(10.0, 0.0), Align2::CENTER_CENTER, ic, icon_font(11.0), pal.accent_text);
            }
            if let (true, Some(s)) = (resp.clicked(), sort) {
                if app.cfg.sort == s {
                    app.cfg.sort_desc = !app.cfg.sort_desc;
                } else {
                    app.cfg.sort = s;
                    app.cfg.sort_desc = false;
                }
                app.view_dirty = true;
            }
            ui.painter().line_segment([r.right_top() + Vec2::new(0.0, 6.0), r.right_bottom() - Vec2::new(0.0, 6.0)], Stroke::new(1.0, pal.border));
            x += w;
        }
        let n = app.view.len();
        egui::ScrollArea::vertical().id_salt("details_v").auto_shrink([false, false]).show_rows(ui, row_h, n, |ui, range| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for idx in range {
                let id = app.view[idx];
                let f = app.faces[id].clone();
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(total_w.max(ui.available_width()), row_h), Sense::click());
                let selected = app.selection.contains(&id);
                if selected {
                    ui.painter().rect_filled(rect, CornerRadius::same(4), pal.accent_soft);
                } else if resp.hovered() {
                    ui.painter().rect_filled(rect, CornerRadius::same(4), pal.hover.gamma_multiply(0.4));
                } else if idx % 2 == 1 {
                    ui.painter().rect_filled(rect, 0.0, pal.panel.gamma_multiply(0.45));
                }
                let mut x = rect.left();
                let text_col = if selected { pal.accent_text } else { pal.text };
                let st = app.status(&f);
                for (ci, (_, w, _)) in cols.iter().enumerate() {
                    let r = Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(*w, row_h)).shrink2(Vec2::new(8.0, 3.0));
                    let painter = ui.painter().with_clip_rect(r);
                    let txt = match ci {
                        0 => {
                            dot(ui, r.left_center() + Vec2::new(4.0, 0.0), 3.5, app.status_color(st));
                            painter.text(r.left_center() + Vec2::new(14.0, 0.0), Align2::LEFT_CENTER, &f.file_name, egui::FontId::proportional(12.0), text_col);
                            None
                        }
                        1 => {
                            let p = app.renderer.get(job(&f, f.display_name().to_string(), 18.0, r.width(), pp));
                            draw_preview(ui, &pal, p, r, tint, false);
                            None
                        }
                        2 => Some(f.display_name().to_string()),
                        3 => Some(f.family.clone()),
                        4 => Some(f.folder.display().to_string()),
                        5 => Some(f.version.replace("Version ", "")),
                        6 => Some(human_size(f.file_size)),
                        _ => Some(f.kind.to_string()),
                    };
                    if let Some(t) = txt {
                        let align = if ci == 6 { Align2::RIGHT_CENTER } else { Align2::LEFT_CENTER };
                        let pos = if ci == 6 { r.right_center() } else { r.left_center() };
                        painter.text(pos, align, t, egui::FontId::proportional(12.0), if ci == 2 { text_col } else { pal.muted });
                    }
                    x += w;
                }
                handle_item(app, ui, id, &resp);
            }
        });
    });
}

// --------------------------------------------------------------------------------------------
// Sample dock

pub fn bottom_panel(app: &mut FontApp, ui: &mut egui::Ui) {
    let pal = app.pal;
    let max_h = (ui.available_height() - 180.0).max(140.0);
    app.cfg.bottom_height = app.cfg.bottom_height.clamp(120.0, max_h);
    let resp = Panel::bottom("sampledock")
        .exact_size(app.cfg.bottom_height)
        .frame(Frame::new().fill(pal.panel))
        .show(ui, |ui| {
            dock_header(ui, &pal, |ui| {
                tab_strip(
                    ui,
                    &pal,
                    &mut app.bottom_tab,
                    &[
                        (BottomTab::TextSample, "Text Sample"),
                        (BottomTab::Characters, "Characters Table"),
                        (BottomTab::Waterfall, "Waterfall"),
                        (BottomTab::Tags, "Tags"),
                        (BottomTab::Search, "Search"),
                    ],
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if icon_button(ui, &pal, I::X, "Hide panel", false).clicked() {
                        app.cfg.show_bottom = false;
                    }
                    if let Some(i) = app.focus {
                        ui.add(egui::Label::new(egui::RichText::new(app.faces[i].display_name()).size(12.0).color(pal.muted)).truncate());
                    }
                });
            });
            let inner = ui.available_rect_before_wrap().shrink2(Vec2::new(10.0, 6.0));
            ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| match app.bottom_tab {
                BottomTab::TextSample => text_sample(app, ui),
                BottomTab::Characters => characters(app, ui),
                BottomTab::Waterfall => waterfall(app, ui),
                BottomTab::Tags => {
                    egui::ScrollArea::vertical().id_salt("tags_pane").auto_shrink([false, false]).show(ui, |ui| tags_pane(app, ui));
                }
                BottomTab::Search => search_pane(app, ui),
            });
        });
    // Drag the top edge to resize the sample dock.
    let edge = Rect::from_min_max(resp.response.rect.left_top() - Vec2::new(0.0, 3.0), resp.response.rect.right_top() + Vec2::new(0.0, 3.0));
    let h = ui.interact(edge, ui.id().with("sampledock_resize"), Sense::drag());
    if h.hovered() || h.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        ui.painter().line_segment([edge.left_center(), edge.right_center()], Stroke::new(2.0, pal.accent));
    }
    if h.dragged() {
        app.cfg.bottom_height = (app.cfg.bottom_height - h.drag_delta().y).clamp(120.0, max_h);
    }
    if h.drag_stopped() {
        app.mark_cfg_dirty();
    }
}

fn no_selection(pal: &Palette, ui: &mut egui::Ui) {
    ui.add_space(8.0);
    ui.label(egui::RichText::new("There are no items to display in this view. Select a font above.").color(pal.faint));
}

fn text_sample(app: &mut FontApp, ui: &mut egui::Ui) {
    let pal = app.pal;
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt("sample_pick").selected_text("Text Sample").width(110.0).show_ui(ui, |ui| {
            for s in app.cfg.sample_texts.clone() {
                let short: String = s.chars().take(48).collect();
                if ui.selectable_label(app.cfg.text_sample == s, short).clicked() {
                    app.cfg.text_sample = s.clone();
                    app.mark_cfg_dirty();
                }
            }
        });
        ui.add(egui::TextEdit::singleline(&mut app.cfg.text_sample).hint_text("Enter your own text").desired_width(260.0));
        vsep(ui, &pal);
        ui.label(egui::RichText::new("A").size(11.0).color(pal.muted));
        ui.spacing_mut().slider_width = 90.0;
        ui.add(egui::Slider::new(&mut app.cfg.sample_size, 8.0..=144.0).show_value(false));
        ui.label(egui::RichText::new("A").size(17.0).color(pal.muted));
        ui.add(egui::DragValue::new(&mut app.cfg.sample_size).range(6.0..=300.0).fixed_decimals(0));
        app.cfg.sample_size = app.cfg.sample_size.round();
    });
    ui.add_space(6.0);
    let Some(id) = app.focus else { return no_selection(&pal, ui) };
    let f = app.faces[id].clone();
    let pp = ppp(ui);
    let tint = app.preview_tint();
    egui::ScrollArea::vertical().id_salt("sample_scroll").auto_shrink([false, false]).show(ui, |ui| {
        let w = (ui.available_width() - 12.0).max(50.0);
        let wrap_bucket = ((w * pp) / 24.0).floor() * 24.0;
        let j = Job {
            face: f.key.clone(),
            text: app.cfg.text_sample.clone(),
            px: (app.cfg.sample_size * pp).round() as u32,
            wrap: wrap_bucket as u32,
            max_w: 0,
            center: false,
        };
        match app.renderer.get(j) {
            Preview::Ready(tex, size) => {
                let (rect, _) = ui.allocate_exact_size(Vec2::new(w, size.y), Sense::hover());
                let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                ui.painter().image(tex, Rect::from_min_size(rect.min, size), uv, tint);
            }
            Preview::Pending => {
                ui.add(egui::Spinner::new().color(pal.accent));
            }
            Preview::Failed => {
                ui.label(egui::RichText::new("This font cannot be previewed.").color(pal.faint));
            }
        }
    });
}

const BLOCKS: &[(&str, u32, u32)] = &[
    ("Basic Latin", 0x20, 0x7F),
    ("Latin-1 Supplement", 0x80, 0xFF),
    ("Latin Extended-A", 0x100, 0x17F),
    ("Latin Extended-B", 0x180, 0x24F),
    ("IPA Extensions", 0x250, 0x2AF),
    ("Spacing Modifiers", 0x2B0, 0x2FF),
    ("Combining Marks", 0x300, 0x36F),
    ("Greek and Coptic", 0x370, 0x3FF),
    ("Cyrillic", 0x400, 0x52F),
    ("Armenian", 0x530, 0x58F),
    ("Hebrew", 0x590, 0x5FF),
    ("Arabic", 0x600, 0x6FF),
    ("Devanagari", 0x900, 0x97F),
    ("Thai", 0xE00, 0xE7F),
    ("Georgian", 0x10A0, 0x10FF),
    ("Latin Extended Additional", 0x1E00, 0x1EFF),
    ("Greek Extended", 0x1F00, 0x1FFF),
    ("General Punctuation", 0x2000, 0x206F),
    ("Super/Subscripts", 0x2070, 0x209F),
    ("Currency Symbols", 0x20A0, 0x20CF),
    ("Letterlike Symbols", 0x2100, 0x214F),
    ("Number Forms", 0x2150, 0x218F),
    ("Arrows", 0x2190, 0x21FF),
    ("Mathematical Operators", 0x2200, 0x22FF),
    ("Misc Technical", 0x2300, 0x23FF),
    ("Box Drawing", 0x2500, 0x257F),
    ("Block Elements", 0x2580, 0x259F),
    ("Geometric Shapes", 0x25A0, 0x25FF),
    ("Misc Symbols", 0x2600, 0x26FF),
    ("Dingbats", 0x2700, 0x27BF),
    ("CJK Symbols", 0x3000, 0x303F),
    ("Hiragana & Katakana", 0x3040, 0x30FF),
    ("CJK Ideographs", 0x4E00, 0x9FFF),
    ("Hangul", 0xAC00, 0xD7AF),
    ("Private Use Area", 0xE000, 0xF8FF),
    ("Alphabetic Presentation", 0xFB00, 0xFB4F),
    ("Specials & Forms", 0xFE00, 0xFFFF),
    ("Supplementary Planes", 0x10000, 0x10FFFF),
];

fn characters(app: &mut FontApp, ui: &mut egui::Ui) {
    let pal = app.pal;
    let Some(id) = app.focus else {
        return no_selection(&pal, ui);
    };
    let cps = app.codepoints_for(id);
    let present: Vec<usize> = BLOCKS.iter().enumerate().filter(|(_, b)| cps.iter().any(|&c| c >= b.1 && c <= b.2)).map(|(i, _)| i).collect();
    ui.horizontal(|ui| {
        let label = if app.char_block == 0 { "All defined characters".to_string() } else { BLOCKS[app.char_block - 1].0.to_string() };
        egui::ComboBox::from_id_salt("char_block").selected_text(label).width(200.0).show_ui(ui, |ui| {
            ui.selectable_value(&mut app.char_block, 0, "All defined characters");
            for &i in &present {
                ui.selectable_value(&mut app.char_block, i + 1, BLOCKS[i].0);
            }
        });
        vsep(ui, &pal);
        ui.label(egui::RichText::new("A").size(11.0).color(pal.muted));
        ui.spacing_mut().slider_width = 90.0;
        ui.add(egui::Slider::new(&mut app.cfg.char_size, 16.0..=96.0).show_value(false));
        ui.label(egui::RichText::new("A").size(17.0).color(pal.muted));
        ui.add(egui::DragValue::new(&mut app.cfg.char_size).range(12.0..=128.0).fixed_decimals(0));
        app.cfg.char_size = app.cfg.char_size.round();
        vsep(ui, &pal);
        ui.label(egui::RichText::new(format!("{} characters", cps.len())).size(12.0).color(pal.muted));
        if let Some(cp) = app.char_selected {
            if let Some(ch) = char::from_u32(cp) {
                vsep(ui, &pal);
                ui.label(egui::RichText::new(format!("U+{cp:04X}  “{ch}”  ·  double-click a cell to copy")).size(12.0).color(pal.text));
            }
        }
    });
    ui.add_space(4.0);
    let list: Vec<u32> = if app.char_block == 0 {
        cps
    } else {
        let b = BLOCKS[app.char_block - 1];
        cps.into_iter().filter(|&c| c >= b.1 && c <= b.2).collect()
    };
    let f = app.faces[id].clone();
    let cell = (app.cfg.char_size * 2.2).max(56.0);
    let avail = ui.available_width() - 10.0;
    let cols = ((avail / cell).floor() as usize).max(1);
    let rows = list.len().div_ceil(cols);
    let pp = ppp(ui);
    let tint = app.preview_tint();
    let ctx = ui.ctx().clone();
    egui::ScrollArea::vertical().id_salt("chars").auto_shrink([false, false]).show_rows(ui, cell, rows, |ui, range| {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        for r in range {
            ui.horizontal(|ui| {
                for c in 0..cols {
                    let i = r * cols + c;
                    if i >= list.len() {
                        break;
                    }
                    let cp = list[i];
                    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(cell), Sense::click());
                    let sel = app.char_selected == Some(cp);
                    ui.painter().rect_filled(rect.shrink(1.0), CornerRadius::same(3), if sel { pal.accent_soft } else if resp.hovered() { pal.hover.gamma_multiply(0.5) } else { pal.card });
                    ui.painter().rect_stroke(rect.shrink(1.0), CornerRadius::same(3), Stroke::new(1.0, if sel { pal.accent } else { pal.border }), StrokeKind::Inside);
                    ui.painter().text(rect.left_top() + Vec2::new(5.0, 4.0), Align2::LEFT_TOP, format!("U+{cp:04X}"), egui::FontId::proportional(9.5), pal.faint);
                    if let Some(ch) = char::from_u32(cp) {
                        let glyph_box = rect.shrink2(Vec2::new(4.0, 12.0)).translate(Vec2::new(0.0, 6.0));
                        let j = Job {
                            face: f.key.clone(),
                            text: ch.to_string(),
                            px: (app.cfg.char_size * pp).round() as u32,
                            wrap: 0,
                            max_w: (glyph_box.width().min(glyph_box.height()) * pp).round() as u32,
                            center: true,
                        };
                        let p = app.renderer.get(j);
                        draw_preview(ui, &pal, p, glyph_box, tint, true);
                        if resp.clicked() {
                            app.char_selected = Some(cp);
                        }
                        if resp.double_clicked() {
                            ctx.copy_text(ch.to_string());
                            app.toast(&ctx, format!("Copied “{ch}” (U+{cp:04X})"));
                        }
                    }
                }
            });
        }
    });
}

fn waterfall(app: &mut FontApp, ui: &mut egui::Ui) {
    let pal = app.pal;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Sizes").size(12.0).color(pal.muted));
        let mut s = app.cfg.waterfall_sizes.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", ");
        if ui.add(egui::TextEdit::singleline(&mut s).desired_width(220.0)).changed() {
            let v: Vec<u32> = s.split(|c: char| c == ',' || c.is_whitespace()).filter_map(|x| x.trim().parse().ok()).filter(|&x| (4..=300).contains(&x)).collect();
            if !v.is_empty() {
                app.cfg.waterfall_sizes = v;
                app.mark_cfg_dirty();
            }
        }
        ui.label(egui::RichText::new("Text").size(12.0).color(pal.muted));
        ui.add(egui::TextEdit::singleline(&mut app.cfg.text_sample).desired_width(f32::INFINITY));
    });
    ui.add_space(4.0);
    let Some(id) = app.focus else { return no_selection(&pal, ui) };
    let f = app.faces[id].clone();
    let pp = ppp(ui);
    let tint = app.preview_tint();
    egui::ScrollArea::vertical().id_salt("waterfall").auto_shrink([false, false]).show(ui, |ui| {
        for &sz in &app.cfg.waterfall_sizes.clone() {
            let h = sz as f32 * 1.45 + 6.0;
            let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::hover());
            ui.painter().text(rect.left_center(), Align2::LEFT_CENTER, format!("{sz} px"), egui::FontId::proportional(11.0), pal.faint);
            let pr = Rect::from_min_max(rect.min + Vec2::new(54.0, 0.0), rect.max);
            let p = app.renderer.get(job(&f, app.cfg.text_sample.clone(), sz as f32, pr.width(), pp));
            draw_preview(ui, &pal, p, pr, tint, false);
            ui.painter().line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(0.5, pal.border));
        }
    });
}

fn tags_pane(app: &mut FontApp, ui: &mut egui::Ui) {
    let pal = app.pal;
    let ids = app.selected_ids();
    let Some(&first) = ids.first() else { return no_selection(&pal, ui) };
    let key = app.faces[first].key.id_string();
    let mut t = app.cfg.tags.get(&key).cloned().unwrap_or_default();
    let mut changed = false;
    ui.columns(3, |cols| {
        let ui = &mut cols[0];
        ui.label(egui::RichText::new("Rating").size(12.0).color(pal.muted));
        changed |= stars(ui, &pal, &mut t.rating);
        ui.add_space(8.0);
        ui.label(egui::RichText::new("Caption").size(12.0).color(pal.muted));
        changed |= ui.add(egui::TextEdit::multiline(&mut t.caption).hint_text("Enter caption").desired_rows(3).desired_width(f32::INFINITY)).changed();

        let ui = &mut cols[1];
        ui.label(egui::RichText::new("Assigned Keywords").size(12.0).color(pal.muted));
        ui.horizontal(|ui| {
            let r = ui.add(egui::TextEdit::singleline(&mut app.keyword_input).hint_text("Enter keyword").desired_width(ui.available_width() - 34.0));
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if (icon_button(ui, &pal, I::PLUS, "Add keyword", false).clicked() || enter) && !app.keyword_input.trim().is_empty() {
                let k = app.keyword_input.trim().to_string();
                if !t.keywords.contains(&k) {
                    t.keywords.push(k);
                    changed = true;
                }
                app.keyword_input.clear();
            }
        });
        Frame::new().fill(pal.deep).corner_radius(CornerRadius::same(5)).inner_margin(Margin::same(6)).show(ui, |ui| {
            ui.set_min_height(70.0);
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                let mut remove = None;
                for (i, k) in t.keywords.iter().enumerate() {
                    if ui.add(egui::Button::new(format!("{k}  {}", I::X)).corner_radius(CornerRadius::same(10))).on_hover_text("Remove").clicked() {
                        remove = Some(i);
                    }
                }
                if let Some(i) = remove {
                    t.keywords.remove(i);
                    changed = true;
                }
                if t.keywords.is_empty() {
                    ui.label(egui::RichText::new("No keywords").color(pal.faint).size(12.0));
                }
            });
        });
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Quick tags (used by Filters)").size(11.5).color(pal.faint));
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 3.0);
            for tag in crate::filters::USAGE_TAGS.iter().chain(crate::filters::MOOD_TAGS.iter()) {
                let pos = t.keywords.iter().position(|k| k.eq_ignore_ascii_case(tag));
                let text = egui::RichText::new(*tag).size(11.5).color(if pos.is_some() { pal.text } else { pal.muted });
                let b = egui::Button::new(text)
                    .fill(if pos.is_some() { pal.accent_soft } else { pal.deep })
                    .stroke(Stroke::new(1.0, if pos.is_some() { pal.accent } else { pal.border }))
                    .corner_radius(CornerRadius::same(10));
                if ui.add(b).on_hover_text(if pos.is_some() { "Remove this tag" } else { "Add this tag" }).clicked() {
                    match pos {
                        Some(i) => { t.keywords.remove(i); }
                        None => t.keywords.push(tag.to_string()),
                    }
                    changed = true;
                }
            }
        });

        let ui = &mut cols[2];
        ui.label(egui::RichText::new("Assigned Categories").size(12.0).color(pal.muted));
        Frame::new().fill(pal.deep).corner_radius(CornerRadius::same(5)).inner_margin(Margin::same(6)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::Grid::new("cats").num_columns(2).spacing(Vec2::new(12.0, 2.0)).show(ui, |ui| {
                for (i, c) in CATEGORIES.iter().enumerate() {
                    let mut on = t.categories.contains(*c);
                    if ui.checkbox(&mut on, *c).changed() {
                        if on {
                            t.categories.insert(c.to_string());
                        } else {
                            t.categories.remove(*c);
                        }
                        changed = true;
                    }
                    if i % 2 == 1 {
                        ui.end_row();
                    }
                }
            });
        });
    });
    if ids.len() > 1 {
        ui.label(egui::RichText::new(format!("Changes apply to all {} selected fonts", ids.len())).size(11.5).color(pal.faint));
    }
    if changed {
        for i in ids {
            let k = app.faces[i].key.id_string();
            app.cfg.tags.insert(k, t.clone());
        }
        app.mark_cfg_dirty();
    }
}

fn search_pane(app: &mut FontApp, ui: &mut egui::Ui) {
    let pal = app.pal;
    let tabs = ["Search by font name", "Keywords", "Categories", "Rating", "Properties", "Foundry", "Caption"];
    ui.horizontal(|ui| {
        for (i, t) in tabs.iter().enumerate() {
            if ui.add(egui::Button::selectable(app.search.tab == i, *t)).clicked() {
                app.search.tab = i;
            }
        }
    });
    ui.add_space(6.0);
    let mut go = false;
    let enter = |r: &egui::Response, ui: &egui::Ui| r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(320.0);
            match app.search.tab {
                0 => {
                    ui.label(egui::RichText::new("Font name or part of the name").size(12.0).color(pal.muted));
                    let r = ui.add(egui::TextEdit::singleline(&mut app.search.name).hint_text("Enter font name").desired_width(300.0));
                    go |= enter(&r, ui);
                }
                1 => {
                    ui.label(egui::RichText::new("Keyword").size(12.0).color(pal.muted));
                    let r = ui.add(egui::TextEdit::singleline(&mut app.search.keyword).hint_text("Enter keyword").desired_width(300.0));
                    go |= enter(&r, ui);
                }
                2 => {
                    ui.label(egui::RichText::new("Fonts in all of these categories").size(12.0).color(pal.muted));
                    egui::Grid::new("s_cats").num_columns(3).show(ui, |ui| {
                        for (i, c) in CATEGORIES.iter().enumerate() {
                            let mut on = app.search.categories.contains(*c);
                            if ui.checkbox(&mut on, *c).changed() {
                                if on {
                                    app.search.categories.insert(c.to_string());
                                } else {
                                    app.search.categories.remove(*c);
                                }
                            }
                            if i % 3 == 2 {
                                ui.end_row();
                            }
                        }
                    });
                }
                3 => {
                    ui.label(egui::RichText::new("Minimum rating").size(12.0).color(pal.muted));
                    stars(ui, &pal, &mut app.search.min_rating);
                }
                4 => {
                    egui::Grid::new("s_props").num_columns(2).spacing(Vec2::new(10.0, 4.0)).show(ui, |ui| {
                        combo(ui, "Weight", &mut app.search.weight, &["Any", "Light", "Regular / Medium", "Bold and heavier"]);
                        combo(ui, "Slant", &mut app.search.italic, &["Any", "Upright", "Italic"]);
                        combo(ui, "Spacing", &mut app.search.mono, &["Any", "Proportional", "Monospaced"]);
                        combo(ui, "Status", &mut app.search.status, &["Any", "Installed", "Activated", "Not installed"]);
                    });
                }
                5 => {
                    ui.label(egui::RichText::new("Foundry, vendor ID or designer").size(12.0).color(pal.muted));
                    let r = ui.add(egui::TextEdit::singleline(&mut app.search.foundry).hint_text("e.g. Microsoft, ADBE").desired_width(300.0));
                    go |= enter(&r, ui);
                }
                _ => {
                    ui.label(egui::RichText::new("Caption contains").size(12.0).color(pal.muted));
                    let r = ui.add(egui::TextEdit::singleline(&mut app.search.caption).hint_text("Enter caption text").desired_width(300.0));
                    go |= enter(&r, ui);
                }
            }
            ui.add_space(8.0);
            if icon_text_button(ui, &pal, I::SEARCH, "Search", true).clicked() {
                go = true;
            }
        });
        if app.search.tab == 0 {
            ui.add_space(20.0);
            ui.vertical(|ui| {
                for (i, l) in ["All Properties", "Family", "Full Name", "PostScript Name", "File Name"].iter().enumerate() {
                    ui.radio_value(&mut app.search.name_field, i, *l);
                }
            });
        }
    });
    ui.add_space(4.0);
    ui.label(egui::RichText::new("Searches every font Fontabulous knows about: installed fonts, collections, library, worklist and folders you have opened.").size(11.5).color(pal.faint));
    if go {
        app.run_search();
    }
}

fn combo(ui: &mut egui::Ui, label: &str, v: &mut usize, opts: &[&str]) {
    ui.label(label);
    egui::ComboBox::from_id_salt(label).selected_text(opts[*v]).width(160.0).show_index(ui, v, opts.len(), |i| opts[i]);
    ui.end_row();
}
