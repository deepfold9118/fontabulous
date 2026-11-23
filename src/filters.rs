//! Adobe Fonts-style browsing filters: classification, tags, recommendations,
//! typographic properties, language support and foundry. Values come from the
//! person's catalog when a font is in it, otherwise they are measured from the font.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use egui::{Align2, Color32, CornerRadius, Sense, Stroke, StrokeKind, Vec2};

use crate::app::{FontApp, Source};
use crate::catalog::Catalog;
use crate::coverage::{lang_index, script_index, LANGS, SCRIPTS};
use crate::fontdb::FontFace;
use crate::icons as I;
use crate::theme::{bold, icon_font, Palette};
use crate::widgets::section;

pub const CLASSES: &[&str] = &["Sans Serif", "Serif", "Slab Serif", "Script", "Blackletter", "Mono", "Hand", "Decorative"];
pub const MOOD_TAGS: &[&str] = &[
    "Clean", "Fun", "Friendly", "Luxury", "Quirky", "Rough", "Vintage", "Retro", "Futurism", "Art Deco", "Art Nouveau", "Stencil",
    "Calligraphic", "Comic",
];
pub const USAGE_TAGS: &[&str] = &["Headings", "Paragraph", "Branding", "Poster", "Editorial", "Signage", "Web", "Packaging"];
pub const RECS: &[&str] = &["Headings", "Paragraphs"];
pub const WEIGHTS: &[&str] = &["Light", "Regular", "Bold"];
pub const WIDTHS: &[&str] = &["Condensed", "Regular", "Extended"];
pub const LEVELS: &[&str] = &["Low", "Medium", "High"];
pub const NUMBERS: &[&str] = &["Lining", "Oldstyle"];

/// Filter values for one font face.
#[derive(Clone, Default, Debug)]
pub struct Traits {
    pub class: u16,
    pub tags: Vec<String>,
    pub recs: u8,
    pub weight: u8,
    pub width: u8,
    pub xh: u8,
    pub contrast: u8,
    pub caps_only: bool,
    pub numbers: u8,
    pub variable: bool,
    pub scripts: u32,
    pub langs: u64,
    pub foundry: String,
    pub cataloged: bool,
}

fn bit_of(list: &[&str], v: &str) -> u16 {
    list.iter().position(|x| x.eq_ignore_ascii_case(v)).map(|i| 1u16 << i).unwrap_or(0)
}

fn level_bit(v: &str) -> u8 {
    match v {
        "Low" => 1,
        "Regular" | "Medium" => 2,
        "High" => 4,
        _ => 0,
    }
}

/// Best guess at a classification when the catalog doesn't know the font.
fn guess_class(f: &FontFace) -> u16 {
    let name = format!("{} {}", f.family_name(), f.full_name).to_lowercase();
    let has = |w: &str| name.contains(w);
    if has("fraktur") || has("blackletter") || has("textura") || has("old english") || has("schwabacher") {
        return bit_of(CLASSES, "Blackletter");
    }
    if f.monospaced || has(" mono") || has("code") || has("courier") || has("consol") {
        return bit_of(CLASSES, "Mono");
    }
    if has("script") {
        return bit_of(CLASSES, "Script");
    }
    if has("hand") || has("marker") || has("brush") {
        return bit_of(CLASSES, "Hand");
    }
    if has("slab") {
        return bit_of(CLASSES, "Slab Serif");
    }
    if has("sans") {
        return bit_of(CLASSES, "Sans Serif");
    }
    match (f.family_class >> 8) & 0xff {
        1 | 2 | 3 | 4 | 7 => return bit_of(CLASSES, "Serif"),
        5 => return bit_of(CLASSES, "Slab Serif"),
        8 => return bit_of(CLASSES, "Sans Serif"),
        9 | 12 => return bit_of(CLASSES, "Decorative"),
        10 => return bit_of(CLASSES, "Script"),
        _ => {}
    }
    match f.panose[0] {
        3 => return bit_of(CLASSES, "Script"),
        4 | 5 => return bit_of(CLASSES, "Decorative"),
        2 => match f.panose[1] {
            11..=15 => return bit_of(CLASSES, "Sans Serif"),
            4..=6 => return bit_of(CLASSES, "Slab Serif"),
            2 | 3 | 7..=10 => return bit_of(CLASSES, "Serif"),
            _ => {}
        },
        _ => {}
    }
    if has("serif") {
        return bit_of(CLASSES, "Serif");
    }
    if f.symbol {
        return bit_of(CLASSES, "Decorative");
    }
    0
}

pub fn compute(f: &FontFace, cat: Option<&Catalog>) -> Traits {
    let fam = cat.and_then(|c| c.by_path.get(&f.npath).map(|&i| &c.families[i]));
    // Weight and width per face (OS/2 classes); a single-width catalog family wins for width.
    let weight = if f.weight < 350 { 1 } else if f.weight < 550 { 2 } else { 4 };
    let face_width = if f.width <= 4 { 1 } else if f.width >= 6 { 4 } else { 2 };
    let xh_measured = f.xh_ratio.map(|r| if r < 0.62 { 1 } else if r > 0.70 { 4 } else { 2 }).unwrap_or(0);
    match fam {
        Some(c) => {
            let mut class = 0;
            for v in &c.classification {
                class |= bit_of(CLASSES, v);
            }
            let mut recs = 0u8;
            for v in &c.recommendations {
                recs |= bit_of(RECS, v) as u8;
            }
            let width = if c.width.len() == 1 { level_or_width(&c.width[0]) } else { face_width };
            let mut numbers = 0u8;
            for v in &c.number_style {
                numbers |= bit_of(NUMBERS, v) as u8;
            }
            let mut scripts = 0u32;
            for v in &c.writing_systems {
                if let Some(i) = script_index(v) {
                    scripts |= 1 << i;
                }
            }
            let mut langs = 0u64;
            for v in &c.languages {
                if let Some(i) = lang_index(v) {
                    langs |= 1 << i;
                }
            }
            Traits {
                class: if class == 0 { guess_class(f) } else { class },
                tags: c.tags.clone(),
                recs,
                weight,
                width,
                xh: c.x_height.as_deref().map(level_bit).unwrap_or(xh_measured),
                contrast: c.contrast.as_deref().map(level_bit).unwrap_or(0),
                caps_only: c.caps_only,
                numbers,
                variable: c.variable || f.variable,
                scripts: if scripts == 0 { f.scripts } else { scripts },
                langs: if langs == 0 { f.langs } else { langs },
                foundry: if c.foundry.is_empty() { f.manufacturer.clone() } else { c.foundry.clone() },
                cataloged: true,
            }
        }
        None => {
            let class = guess_class(f);
            let text_class = class & (bit_of(CLASSES, "Sans Serif") | bit_of(CLASSES, "Serif") | bit_of(CLASSES, "Slab Serif")) != 0;
            let mut recs = 1u8; // Headings
            if text_class && !f.caps_only && xh_measured != 1 {
                recs |= 2;
            }
            let foundry = if !f.manufacturer.is_empty() {
                f.manufacturer.clone()
            } else if !f.designer.is_empty() {
                f.designer.clone()
            } else {
                "Unknown".into()
            };
            Traits {
                class,
                tags: Vec::new(),
                recs,
                weight,
                width: face_width,
                xh: xh_measured,
                contrast: 0,
                caps_only: f.caps_only,
                numbers: 0,
                variable: f.variable,
                scripts: f.scripts,
                langs: f.langs,
                foundry,
                cataloged: false,
            }
        }
    }
}

fn level_or_width(v: &str) -> u8 {
    match v {
        "Condensed" => 1,
        "Extended" => 4,
        _ => 2,
    }
}

/// The person's current filter choices. Within a group any choice may match;
/// every group that has a choice must match.
#[derive(Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct FilterState {
    pub class: u16,
    pub tags: BTreeSet<String>,
    pub recs: u8,
    pub weight: u8,
    pub width: u8,
    pub xh: u8,
    pub contrast: u8,
    pub numbers: u8,
    pub caps_only: bool,
    pub variable: bool,
    pub scripts: u32,
    pub langs: u64,
    pub foundries: BTreeSet<String>,
}

impl FilterState {
    pub fn is_empty(&self) -> bool {
        *self == FilterState::default()
    }

    pub fn count(&self) -> usize {
        let b = |x: u64| x.count_ones() as usize;
        b(self.class as u64)
            + self.tags.len()
            + b(self.recs as u64)
            + b(self.weight as u64)
            + b(self.width as u64)
            + b(self.xh as u64)
            + b(self.contrast as u64)
            + b(self.numbers as u64)
            + self.caps_only as usize
            + self.variable as usize
            + b(self.scripts as u64)
            + b(self.langs)
            + self.foundries.len()
    }

    pub fn matches(&self, t: &Traits, user_tags: &[String]) -> bool {
        let any = |sel: u64, have: u64| sel == 0 || sel & have != 0;
        if !any(self.class as u64, t.class as u64)
            || !any(self.recs as u64, t.recs as u64)
            || !any(self.weight as u64, t.weight as u64)
            || !any(self.width as u64, t.width as u64)
            || !any(self.xh as u64, t.xh as u64)
            || !any(self.contrast as u64, t.contrast as u64)
            || !any(self.numbers as u64, t.numbers as u64)
            || !any(self.scripts as u64, t.scripts as u64)
            || !any(self.langs, t.langs)
        {
            return false;
        }
        if self.caps_only && !t.caps_only {
            return false;
        }
        if self.variable && !t.variable {
            return false;
        }
        if !self.tags.is_empty() {
            let hit = self.tags.iter().any(|s| {
                t.tags.iter().any(|x| x.eq_ignore_ascii_case(s)) || user_tags.iter().any(|x| x.eq_ignore_ascii_case(s))
            });
            if !hit {
                return false;
            }
        }
        if !self.foundries.is_empty() && !self.foundries.contains(&t.foundry) {
            return false;
        }
        true
    }
}

/// Option counts over every known font (shown next to each chip).
#[derive(Default)]
pub struct Counts {
    pub class: Vec<usize>,
    pub tags: BTreeMap<String, usize>,
    pub recs: Vec<usize>,
    pub weight: Vec<usize>,
    pub width: Vec<usize>,
    pub xh: Vec<usize>,
    pub contrast: Vec<usize>,
    pub numbers: Vec<usize>,
    pub caps_only: usize,
    pub variable: usize,
    pub scripts: Vec<usize>,
    pub langs: Vec<usize>,
    pub foundries: BTreeMap<String, usize>,
}

pub fn counts(traits: &[Arc<Traits>], user_tags: &[Vec<String>]) -> Counts {
    let mut c = Counts {
        class: vec![0; CLASSES.len()],
        recs: vec![0; 2],
        weight: vec![0; 3],
        width: vec![0; 3],
        xh: vec![0; 3],
        contrast: vec![0; 3],
        numbers: vec![0; 2],
        scripts: vec![0; SCRIPTS.len()],
        langs: vec![0; LANGS.len()],
        ..Default::default()
    };
    let bump = |v: &mut Vec<usize>, bits: u64| {
        for (i, x) in v.iter_mut().enumerate() {
            if bits & (1 << i) != 0 {
                *x += 1;
            }
        }
    };
    for (i, t) in traits.iter().enumerate() {
        bump(&mut c.class, t.class as u64);
        bump(&mut c.recs, t.recs as u64);
        bump(&mut c.weight, t.weight as u64);
        bump(&mut c.width, t.width as u64);
        bump(&mut c.xh, t.xh as u64);
        bump(&mut c.contrast, t.contrast as u64);
        bump(&mut c.numbers, t.numbers as u64);
        bump(&mut c.scripts, t.scripts as u64);
        bump(&mut c.langs, t.langs);
        c.caps_only += t.caps_only as usize;
        c.variable += t.variable as usize;
        let mut seen = BTreeSet::new();
        for tag in t.tags.iter().chain(user_tags.get(i).into_iter().flatten()) {
            if seen.insert(tag.to_lowercase()) {
                *c.tags.entry(tag.clone()).or_default() += 1;
            }
        }
        *c.foundries.entry(t.foundry.clone()).or_default() += 1;
    }
    c
}

// ------------------------------------------------------------------------------------------
// Sidebar UI

fn chip(ui: &mut egui::Ui, pal: &Palette, label: &str, count: Option<usize>, on: bool) -> bool {
    let font = egui::FontId::proportional(12.0);
    let g = ui.painter().layout_no_wrap(label.to_string(), font.clone(), pal.text);
    let cnt = count.map(|n| n.to_string());
    let cg = cnt.as_ref().map(|s| ui.painter().layout_no_wrap(s.clone(), egui::FontId::proportional(10.5), pal.faint));
    let w = g.size().x + 20.0 + cg.as_ref().map(|c| c.size().x + 6.0).unwrap_or(0.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 24.0), Sense::click());
    let (bg, fg, stroke) = if on {
        (pal.accent_soft, pal.accent_text, Stroke::new(1.0, pal.accent))
    } else if resp.hovered() {
        (pal.hover, pal.text, Stroke::new(1.0, pal.border))
    } else {
        (Color32::TRANSPARENT, pal.text, Stroke::new(1.0, pal.border))
    };
    let disabled = count == Some(0) && !on;
    ui.painter().rect_filled(rect, CornerRadius::same(12), bg);
    ui.painter().rect_stroke(rect, CornerRadius::same(12), stroke, StrokeKind::Inside);
    ui.painter().text(rect.left_center() + Vec2::new(10.0, 0.0), Align2::LEFT_CENTER, label, font, if disabled { pal.faint } else { fg });
    if let Some(c) = cnt {
        ui.painter().text(rect.right_center() - Vec2::new(10.0, 0.0), Align2::RIGHT_CENTER, c, egui::FontId::proportional(10.5), pal.faint);
    }
    resp.clicked()
}

fn chips_bits(ui: &mut egui::Ui, pal: &Palette, labels: &[&str], counts: Option<&[usize]>, bits: &mut u64) -> bool {
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
        for (i, l) in labels.iter().enumerate() {
            let on = *bits & (1 << i) != 0;
            if chip(ui, pal, l, counts.map(|c| c[i]), on) {
                *bits ^= 1 << i;
                changed = true;
            }
        }
    });
    changed
}

fn sub(ui: &mut egui::Ui, pal: &Palette, label: &str) {
    ui.add_space(4.0);
    ui.label(egui::RichText::new(label).size(11.5).color(pal.muted));
}

fn chips_u8(ui: &mut egui::Ui, pal: &Palette, labels: &[&str], counts: &[usize], v: &mut u8) -> bool {
    let mut b = *v as u64;
    let c = chips_bits(ui, pal, labels, Some(counts), &mut b);
    *v = b as u8;
    c
}

fn search_box(ui: &mut egui::Ui, pal: &Palette, id: &str, hint: &str) -> String {
    let key = egui::Id::new(("filter_q", id));
    let mut q: String = ui.data(|d| d.get_temp(key)).unwrap_or_default();
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(I::SEARCH).font(icon_font(12.0)).color(pal.muted));
        ui.add(egui::TextEdit::singleline(&mut q).hint_text(hint).desired_width(f32::INFINITY));
    });
    ui.data_mut(|d| d.insert_temp(key, q.clone()));
    q.to_lowercase()
}

fn check_row(ui: &mut egui::Ui, pal: &Palette, label: &str, count: usize, on: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 22.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(4), pal.hover.gamma_multiply(0.6));
    }
    let bx = egui::Rect::from_center_size(rect.left_center() + Vec2::new(10.0, 0.0), Vec2::splat(13.0));
    ui.painter().rect_filled(bx, CornerRadius::same(3), if on { pal.accent } else { Color32::TRANSPARENT });
    ui.painter().rect_stroke(bx, CornerRadius::same(3), Stroke::new(1.0, if on { pal.accent } else { pal.faint }), StrokeKind::Inside);
    if on {
        ui.painter().text(bx.center(), Align2::CENTER_CENTER, I::CHECK, icon_font(10.0), Color32::WHITE);
    }
    let clip = egui::Rect::from_min_max(rect.min + Vec2::new(22.0, 0.0), rect.max - Vec2::new(44.0, 0.0));
    ui.painter().with_clip_rect(clip).text(clip.left_center(), Align2::LEFT_CENTER, label, egui::FontId::proportional(12.0), pal.text);
    ui.painter().text(rect.right_center() - Vec2::new(6.0, 0.0), Align2::RIGHT_CENTER, count.to_string(), egui::FontId::proportional(11.0), pal.faint);
    resp.clicked()
}

pub fn panel(app: &mut FontApp, ui: &mut egui::Ui) {
    let pal = app.pal;
    app.ensure_catalog(ui.ctx());
    app.ensure_traits();
    let before = app.filters.clone();
    // Header: match count, clear, catalog status.
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(I::FILTER).font(icon_font(14.0)).color(pal.accent_text));
        ui.label(egui::RichText::new("Browse by filters").font(bold(13.0)).color(pal.text));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add_enabled(!app.filters.is_empty(), egui::Button::new("Clear all")).clicked() {
                app.filters = FilterState::default();
            }
        });
    });
    let status = match (&app.catalog, &app.catalog_error, app.catalog_rx.is_some()) {
        (_, _, true) => "Loading your font catalog…".to_string(),
        (Some(c), _, _) => format!("Catalog: {} families · other fonts are measured", c.families.len()),
        (None, Some(e), _) => e.clone(),
        (None, None, _) => "No catalog found · values are measured from the fonts".into(),
    };
    ui.horizontal(|ui| {
        ui.add(egui::Label::new(egui::RichText::new(status).size(11.0).color(pal.faint)).truncate());
    });
    ui.horizontal(|ui| {
        if ui.small_button(format!("{}  Load catalog…", I::FOLDER_OPEN)).on_hover_text("Choose a catalog.json made for your font library").clicked() {
            if let Some(p) = rfd::FileDialog::new().add_filter("Font catalog", &["json"]).pick_file() {
                app.load_catalog(ui.ctx(), p);
            }
        }
        if !app.filters.is_empty() {
            ui.label(egui::RichText::new(format!("{} filters · {} fonts", app.filters.count(), app.view.len())).size(11.5).color(pal.accent_text));
        }
    });
    ui.separator();

    let counts = app.filter_counts.take().unwrap_or_default();
    egui::ScrollArea::vertical().id_salt("filters_scroll").auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        let f = &mut app.filters;
        section(ui, &pal, "flt_class", "Classification", true, |ui| {
            chips_u8_16(ui, &pal, CLASSES, &counts.class, &mut f.class);
        });
        section(ui, &pal, "flt_tags", "Tags", true, |ui| {
            for (title, list) in [("Mood & style", MOOD_TAGS), ("Usage", USAGE_TAGS)] {
                sub(ui, &pal, title);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
                    for t in list {
                        let n = counts.tags.iter().filter(|(k, _)| k.eq_ignore_ascii_case(t)).map(|(_, v)| *v).sum::<usize>();
                        let on = f.tags.contains(*t);
                        if chip(ui, &pal, t, Some(n), on) {
                            if on {
                                f.tags.remove(*t);
                            } else {
                                f.tags.insert(t.to_string());
                            }
                        }
                    }
                });
            }
            sub(ui, &pal, "More tags");
            let q = search_box(ui, &pal, "tags", "Search tags");
            let mut others: Vec<(&String, &usize)> = counts
                .tags
                .iter()
                .filter(|(k, _)| !MOOD_TAGS.iter().chain(USAGE_TAGS).any(|m| m.eq_ignore_ascii_case(k)))
                .filter(|(k, _)| q.is_empty() || k.to_lowercase().contains(&q))
                .collect();
            others.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
                for (t, n) in others.into_iter().take(if q.is_empty() { 24 } else { 60 }) {
                    let on = f.tags.contains(t);
                    if chip(ui, &pal, t, Some(*n), on) {
                        if on {
                            f.tags.remove(t);
                        } else {
                            f.tags.insert(t.clone());
                        }
                    }
                }
            });
            ui.label(egui::RichText::new("Your own keywords from the Tags pane count as tags too.").size(10.5).color(pal.faint));
        });
        section(ui, &pal, "flt_recs", "Recommendations", true, |ui| {
            chips_u8(ui, &pal, RECS, &counts.recs, &mut f.recs);
        });
        section(ui, &pal, "flt_props", "Properties", true, |ui| {
            sub(ui, &pal, "Weight");
            chips_u8(ui, &pal, WEIGHTS, &counts.weight, &mut f.weight);
            sub(ui, &pal, "Width");
            chips_u8(ui, &pal, WIDTHS, &counts.width, &mut f.width);
            sub(ui, &pal, "x-height");
            chips_u8(ui, &pal, LEVELS, &counts.xh, &mut f.xh);
            sub(ui, &pal, "Contrast");
            chips_u8(ui, &pal, LEVELS, &counts.contrast, &mut f.contrast);
            sub(ui, &pal, "Number style");
            chips_u8(ui, &pal, &["Lining figures", "Oldstyle figures"], &counts.numbers, &mut f.numbers);
            sub(ui, &pal, "Other");
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
                if chip(ui, &pal, "Caps only", Some(counts.caps_only), f.caps_only) {
                    f.caps_only = !f.caps_only;
                }
                if chip(ui, &pal, "Variable fonts", Some(counts.variable), f.variable) {
                    f.variable = !f.variable;
                }
            });
        });
        section(ui, &pal, "flt_lang", "Language Support", true, |ui| {
            sub(ui, &pal, "Writing systems");
            let names: Vec<&str> = SCRIPTS.iter().map(|s| s.0).collect();
            let mut b = f.scripts as u64;
            if chips_bits(ui, &pal, &names, Some(&counts.scripts), &mut b) {
                f.scripts = b as u32;
            }
            sub(ui, &pal, "Languages");
            let q = search_box(ui, &pal, "langs", "Search languages");
            egui::ScrollArea::vertical().id_salt("langs_list").max_height(170.0).show(ui, |ui| {
                for (i, (_, name, _)) in LANGS.iter().enumerate() {
                    if !q.is_empty() && !name.to_lowercase().contains(&q) {
                        continue;
                    }
                    let on = f.langs & (1 << i) != 0;
                    if check_row(ui, &pal, name, counts.langs[i], on) {
                        f.langs ^= 1 << i;
                    }
                }
            });
        });
        section(ui, &pal, "flt_foundry", "Foundry", true, |ui| {
            let q = search_box(ui, &pal, "foundry", "Search foundries");
            let mut list: Vec<(&String, &usize)> =
                counts.foundries.iter().filter(|(k, _)| q.is_empty() || k.to_lowercase().contains(&q)).collect();
            list.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
            // Selected foundries always stay visible.
            let selected: Vec<String> = f.foundries.iter().cloned().collect();
            egui::ScrollArea::vertical().id_salt("foundry_list").max_height(220.0).show(ui, |ui| {
                for name in &selected {
                    let n = counts.foundries.get(name).copied().unwrap_or(0);
                    if check_row(ui, &pal, name, n, true) {
                        f.foundries.remove(name);
                    }
                }
                let rest: Vec<(String, usize)> =
                    list.into_iter().filter(|(k, _)| !selected.contains(*k)).take(150).map(|(k, v)| (k.clone(), *v)).collect();
                for (name, n) in &rest {
                    if check_row(ui, &pal, name, *n, false) {
                        f.foundries.insert(name.clone());
                    }
                }
            });
        });
    });
    app.filter_counts = Some(counts);
    if app.filters != before {
        app.cfg.filters = app.filters.clone();
        app.mark_cfg_dirty();
        if app.filters.is_empty() {
            if app.source == Source::Filters {
                app.view_dirty = true;
            }
        } else if app.source != Source::Filters {
            app.set_source(Source::Filters);
        } else {
            app.view_dirty = true;
        }
    }
}

fn chips_u8_16(ui: &mut egui::Ui, pal: &Palette, labels: &[&str], counts: &[usize], v: &mut u16) {
    let mut b = *v as u64;
    chips_bits(ui, pal, labels, Some(counts), &mut b);
    *v = b as u16;
}
