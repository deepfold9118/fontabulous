//! Application state, persistence and the font-view model.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::fontdb::{self, norm, FaceKey, FontFace, InstalledInfo, ScanJob, ScanMsg, Scanner};
use crate::platform;
use crate::render::Renderer;
use crate::theme::{self, Palette, ThemeMode};

pub const CATEGORIES: &[&str] = &[
    "Calligraphic", "Condensed", "Decorative", "Handwritten", "Modern", "Monospaced", "Ornaments", "Other", "Outline",
    "Sans Serif", "Serif", "Stencil", "Symbol", "Wide",
];

pub const DEFAULT_SAMPLE: &str = "Bring me no more reports; let them fly all: Till Birnam wood remove to Dunsinane I cannot taint with fear. What's the boy Malcolm? Was he not born of woman? The spirits that know all mortal consequences have pronounced me thus.";
pub const PANGRAM: &str = "The quick brown fox jumps over the lazy dog";
pub const ALPHABET: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ abcdefghijklmnopqrstuvwxyz";
pub const DIGITS: &str = "0123456789 !@#$%&*()[]{}<>?/+-=";
pub const WATERFALL_TEXT: &str = "In culpa qui officia deserunt mollit anim id est laborum. Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.";

#[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize)]
pub struct Tags {
    pub rating: u8,
    pub caption: String,
    pub keywords: Vec<String>,
    pub categories: BTreeSet<String>,
}

impl Tags {
    pub fn is_empty(&self) -> bool {
        self.rating == 0 && self.caption.is_empty() && self.keywords.is_empty() && self.categories.is_empty()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ViewMode {
    List,
    Tiles,
    Details,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DisplayText {
    FontName,
    Pangram,
    Alphabet,
    Digits,
    Custom,
}

impl DisplayText {
    pub fn label(self) -> &'static str {
        match self {
            DisplayText::FontName => "Font Name",
            DisplayText::Pangram => "Pangram",
            DisplayText::Alphabet => "Alphabet",
            DisplayText::Digits => "Digits & Symbols",
            DisplayText::Custom => "Custom Text",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SortBy {
    FontName,
    Family,
    FileName,
    FileSize,
    Date,
    Weight,
    Type,
    Location,
}

impl SortBy {
    pub const ALL: [SortBy; 8] = [
        SortBy::FontName, SortBy::Family, SortBy::FileName, SortBy::FileSize, SortBy::Date, SortBy::Weight, SortBy::Type,
        SortBy::Location,
    ];
    pub fn label(self) -> &'static str {
        match self {
            SortBy::FontName => "Font Name",
            SortBy::Family => "Family",
            SortBy::FileName => "File Name",
            SortBy::FileSize => "File Size",
            SortBy::Date => "Date Modified",
            SortBy::Weight => "Weight",
            SortBy::Type => "File Type",
            SortBy::Location => "Location",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RecentSpan {
    Hour,
    Today,
    Yesterday,
    Week,
    Month,
    Year,
}

impl RecentSpan {
    pub const ALL: [RecentSpan; 6] =
        [RecentSpan::Hour, RecentSpan::Today, RecentSpan::Yesterday, RecentSpan::Week, RecentSpan::Month, RecentSpan::Year];
    pub fn label(self) -> &'static str {
        match self {
            RecentSpan::Hour => "Last Hour",
            RecentSpan::Today => "Today",
            RecentSpan::Yesterday => "Yesterday",
            RecentSpan::Week => "Last Week",
            RecentSpan::Month => "Last Month",
            RecentSpan::Year => "Last Year",
        }
    }
    fn contains(self, t: SystemTime) -> bool {
        let Ok(age) = SystemTime::now().duration_since(t) else { return true };
        let h = age.as_secs_f64() / 3600.0;
        match self {
            RecentSpan::Hour => h <= 1.0,
            RecentSpan::Today => h <= 24.0,
            RecentSpan::Yesterday => (24.0..48.0).contains(&h),
            RecentSpan::Week => h <= 24.0 * 7.0,
            RecentSpan::Month => h <= 24.0 * 31.0,
            RecentSpan::Year => h <= 24.0 * 365.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    None,
    Installed,
    Activated,
    Collection(PathBuf),
    Group(String),
    Library,
    Recent(RecentSpan),
    Worklist,
    Family(String),
    Folder(PathBuf),
    Results(String),
    Filters,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeftTab {
    Fonts,
    Worklist,
    System,
    Folders,
    Filters,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BottomTab {
    TextSample,
    Characters,
    Waterfall,
    Tags,
    Search,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RightTab {
    Tasks,
    Properties,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    Installed,
    Activated,
    NotInstalled,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Config {
    pub theme: ThemeMode,
    pub collections: Vec<PathBuf>,
    pub groups: BTreeMap<String, Vec<FaceKey>>,
    pub library: Vec<PathBuf>,
    pub worklist: Vec<PathBuf>,
    pub tags: HashMap<String, Tags>,
    pub sample_texts: Vec<String>,
    pub text_sample: String,
    pub custom_text: String,
    pub preview_size: f32,
    pub sample_size: f32,
    pub char_size: f32,
    pub tile_width: f32,
    pub waterfall_sizes: Vec<u32>,
    pub view_mode: ViewMode,
    pub display: DisplayText,
    pub sort: SortBy,
    pub sort_desc: bool,
    pub show_left: bool,
    pub show_right: bool,
    pub show_bottom: bool,
    pub preview_color: Option<[u8; 3]>,
    pub preview_bg: Option<[u8; 3]>,
    pub last_folder: Option<PathBuf>,
    pub recent_added: HashMap<String, u64>,
    pub catalog_path: Option<PathBuf>,
    pub left_tab: u8,
    pub filters: crate::filters::FilterState,
    pub bottom_tab: u8,
    pub right_tab: u8,
    pub bottom_height: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: ThemeMode::Dark,
            collections: Vec::new(),
            groups: BTreeMap::new(),
            library: Vec::new(),
            worklist: Vec::new(),
            tags: HashMap::new(),
            sample_texts: vec![DEFAULT_SAMPLE.into(), PANGRAM.into(), ALPHABET.into(), WATERFALL_TEXT.into()],
            text_sample: DEFAULT_SAMPLE.into(),
            custom_text: String::new(),
            preview_size: 36.0,
            sample_size: 28.0,
            char_size: 36.0,
            tile_width: 400.0,
            waterfall_sizes: vec![10, 12, 14, 18, 24, 30, 36, 48, 60, 72],
            view_mode: ViewMode::Tiles,
            display: DisplayText::FontName,
            sort: SortBy::FontName,
            sort_desc: false,
            show_left: true,
            show_right: true,
            show_bottom: true,
            preview_color: None,
            preview_bg: None,
            last_folder: None,
            recent_added: HashMap::new(),
            catalog_path: None,
            left_tab: 0,
            filters: Default::default(),
            bottom_tab: 0,
            right_tab: 0,
            bottom_height: 280.0,
        }
    }
}

#[derive(Default)]
pub struct SearchForm {
    pub tab: usize,
    pub name: String,
    pub name_field: usize,
    pub keyword: String,
    pub categories: BTreeSet<String>,
    pub min_rating: u8,
    pub weight: usize,
    pub italic: usize,
    pub mono: usize,
    pub status: usize,
    pub foundry: String,
    pub caption: String,
}

#[derive(Default)]
pub struct DupForm {
    pub open: bool,
    pub all_known: bool,
    pub by_name: bool,
    pub by_size: bool,
    pub by_content: bool,
    pub groups: Vec<Vec<usize>>,
    pub checked: HashSet<usize>,
    pub ran: bool,
}

#[derive(Default)]
pub struct FindForm {
    pub open: bool,
    pub words: String,
    pub roots: Vec<(PathBuf, bool)>,
    pub subfolders: bool,
    pub windows_fonts: bool,
    pub types: [bool; 4],
    pub active: Option<(Vec<PathBuf>, Vec<String>, bool, [bool; 4])>,
}

pub enum Modal {
    None,
    NewGroup { name: String, add_selected: bool },
    RenameGroup { old: String, name: String },
    AddToGroups { checked: BTreeSet<String>, new_name: String },
    ConfirmUninstall(Vec<usize>),
    Message { title: String, body: String },
}

pub struct FontApp {
    pub cfg: Config,
    pub pal: Palette,
    pub faces: Vec<FontFace>,
    pub face_index: HashMap<FaceKey, usize>,
    pub installed: InstalledInfo,
    pub activated: HashSet<String>,
    pub scanner: Scanner,
    pub scanning_label: String,
    pub scanned_dirs: HashSet<PathBuf>,
    pub renderer: Renderer,

    pub source: Source,
    pub view: Vec<usize>,
    pub view_dirty: bool,
    pub results: Vec<FaceKey>,
    pub selection: BTreeSet<usize>,
    pub focus: Option<usize>,
    pub anchor: Option<usize>,
    pub scroll_to_focus: bool,

    pub left_tab: LeftTab,
    pub bottom_tab: BottomTab,
    pub right_tab: RightTab,
    pub quick_find: String,
    pub show_installed: bool,
    pub show_activated: bool,
    pub show_not_installed: bool,

    // Folder browser
    pub folder_children: HashMap<PathBuf, Vec<PathBuf>>,
    pub folder_open: HashSet<PathBuf>,
    pub folder_back: Vec<PathBuf>,
    pub folder_fwd: Vec<PathBuf>,
    pub folder_path_edit: String,

    // Characters table
    pub cps_cache: Option<(usize, Vec<u32>)>,
    pub char_block: usize,
    pub char_selected: Option<u32>,

    pub keyword_input: String,
    pub search: SearchForm,
    pub dups: DupForm,
    pub find: FindForm,
    pub compare_open: bool,
    pub compare_ids: Vec<usize>,
    pub compare_text: String,
    pub options_open: bool,
    pub about_open: bool,
    pub kerning_open: bool,
    pub kerning_cache: Option<(usize, Vec<(char, char, i16)>)>,
    pub problems_open: bool,
    pub problems: Vec<(String, String)>,
    pub modal: Modal,
    pub toast: Option<(String, f64)>,
    pub family_filter: String,

    pub screenshot: Option<PathBuf>,
    pub frame_no: u64,
    pub shot_requested: bool,
    pub auto_selected: bool,
    pub cfg_dirty_at: Option<f64>,
    pub count_cache: std::cell::RefCell<HashMap<String, usize>>,
    pub filters: crate::filters::FilterState,
    pub catalog: Option<std::sync::Arc<crate::catalog::Catalog>>,
    pub catalog_rx: Option<std::sync::mpsc::Receiver<Result<crate::catalog::Catalog, String>>>,
    pub catalog_error: Option<String>,
    pub catalog_checked: bool,
    pub traits: Vec<std::sync::Arc<crate::filters::Traits>>,
    pub filter_counts: Option<crate::filters::Counts>,
    pub counts_dirty: bool,
}

impl FontApp {
    pub fn new(cc: &eframe::CreationContext<'_>, screenshot: Option<PathBuf>) -> Self {
        let cfg = load_config();
        theme::install_fonts(&cc.egui_ctx);
        theme::apply(&cc.egui_ctx, cfg.theme);
        let mut scanner = Scanner::new(cc.egui_ctx.clone());
        scanner.push(ScanJob::Installed);
        let mut scanned_dirs = HashSet::new();
        for c in &cfg.collections {
            scanner.push(ScanJob::Dir { path: c.clone(), recursive: true });
            scanned_dirs.insert(c.clone());
        }
        let mut files: Vec<PathBuf> = cfg.library.clone();
        files.extend(cfg.worklist.iter().cloned());
        for g in cfg.groups.values() {
            files.extend(g.iter().map(|k| k.path.clone()));
        }
        if !files.is_empty() {
            scanner.push(ScanJob::Files(files));
        }
        let renderer = Renderer::new(&cc.egui_ctx);
        let bottom_tab = match cfg.bottom_tab {
            1 => BottomTab::Characters,
            2 => BottomTab::Waterfall,
            3 => BottomTab::Tags,
            4 => BottomTab::Search,
            _ => BottomTab::TextSample,
        };
        let cfg_left_tab = cfg.left_tab;
        let cfg_filters = cfg.filters.clone();
        let start_source = if cfg_left_tab == 4 && !cfg_filters.is_empty() { Source::Filters } else { Source::Installed };
        let right_tab = if cfg.right_tab == 1 { RightTab::Properties } else { RightTab::Tasks };
        Self {
            pal: theme::palette(cfg.theme),
            cfg,
            faces: Vec::new(),
            face_index: HashMap::new(),
            installed: InstalledInfo::default(),
            activated: HashSet::new(),
            scanner,
            scanning_label: "Reading installed fonts…".into(),
            scanned_dirs,
            renderer,
            source: start_source,
            view: Vec::new(),
            view_dirty: true,
            results: Vec::new(),
            selection: BTreeSet::new(),
            focus: None,
            anchor: None,
            scroll_to_focus: false,
            left_tab: match cfg_left_tab {
                1 => LeftTab::Worklist,
                2 => LeftTab::System,
                3 => LeftTab::Folders,
                4 => LeftTab::Filters,
                _ => LeftTab::Fonts,
            },
            bottom_tab,
            right_tab,
            quick_find: String::new(),
            show_installed: true,
            show_activated: true,
            show_not_installed: true,
            folder_children: HashMap::new(),
            folder_open: HashSet::new(),
            folder_back: Vec::new(),
            folder_fwd: Vec::new(),
            folder_path_edit: String::new(),
            cps_cache: None,
            char_block: 0,
            char_selected: None,
            keyword_input: String::new(),
            search: SearchForm { name_field: 0, ..Default::default() },
            dups: DupForm { by_name: true, by_size: true, ..Default::default() },
            find: FindForm { subfolders: true, types: [true; 4], ..Default::default() },
            compare_open: false,
            compare_ids: Vec::new(),
            compare_text: "Ag".into(),
            options_open: false,
            about_open: false,
            kerning_open: false,
            kerning_cache: None,
            problems_open: false,
            problems: Vec::new(),
            modal: Modal::None,
            toast: None,
            family_filter: String::new(),
            screenshot,
            frame_no: 0,
            shot_requested: false,
            auto_selected: false,
            cfg_dirty_at: None,
            count_cache: Default::default(),
            filters: cfg_filters.clone(),
            catalog: None,
            catalog_rx: None,
            catalog_error: None,
            catalog_checked: false,
            traits: Vec::new(),
            filter_counts: None,
            counts_dirty: true,
        }
    }

    // ----------------------------------------------------------------------------------------
    // Messages from the scanner.

    pub fn poll_scanner(&mut self) {
        let mut any = false;
        while let Ok(msg) = self.scanner.rx.try_recv() {
            match msg {
                ScanMsg::Installed(info) => {
                    self.installed = info;
                    any = true;
                }
                ScanMsg::Faces(batch) => {
                    for f in batch {
                        if let Some(&i) = self.face_index.get(&f.key) {
                            self.faces[i] = f;
                            self.traits.truncate(i);
                        } else {
                            self.face_index.insert(f.key.clone(), self.faces.len());
                            self.faces.push(f);
                        }
                    }
                    any = true;
                }
                ScanMsg::DirDone(_) => any = true,
                ScanMsg::Progress(s) => {
                    self.scanning_label = s;
                    self.scanner.busy = true;
                }
                ScanMsg::Idle => {
                    self.scanner.busy = false;
                    self.scanning_label.clear();
                }
            }
        }
        if any {
            self.view_dirty = true;
        }
    }

    // ----------------------------------------------------------------------------------------
    // Status and filters.

    pub fn status(&self, f: &FontFace) -> Status {
        let n = &f.npath;
        if self.installed.files.contains(n) {
            Status::Installed
        } else if self.activated.contains(n) {
            Status::Activated
        } else {
            Status::NotInstalled
        }
    }

    pub fn status_color(&self, s: Status) -> egui::Color32 {
        match s {
            Status::Installed => self.pal.green,
            Status::Activated => self.pal.orange,
            Status::NotInstalled => self.pal.grey,
        }
    }

    pub fn tags_of(&self, f: &FontFace) -> Option<&Tags> {
        self.cfg.tags.get(&f.key.id_string())
    }

    fn in_source(&self, f: &FontFace, src: &Source, sets: &SourceSets) -> bool {
        match src {
            Source::None => false,
            Source::Installed => self.status(f) != Status::NotInstalled,
            Source::Activated => self.activated.contains(&f.npath),
            Source::Collection(p) => f.key.path.starts_with(p),
            Source::Group(_) => sets.group.contains(&f.key),
            Source::Library => sets.library.contains(&f.npath),
            Source::Recent(span) => {
                let in_lib = sets.library.contains(&f.npath)
                    || self.cfg.collections.iter().any(|c| f.key.path.starts_with(c));
                in_lib && f.created.or(f.modified).map(|t| span.contains(t)).unwrap_or(false)
            }
            Source::Worklist => sets.worklist.contains(&f.npath),
            Source::Family(fam) => self.status(f) != Status::NotInstalled && f.family_name() == fam,
            Source::Folder(p) => f.folder == *p,
            Source::Results(_) => sets.results.contains(&f.key),
            Source::Filters => false,
        }
    }

    pub fn source_sets(&self) -> SourceSets {
        let group = match &self.source {
            Source::Group(g) => self.cfg.groups.get(g).map(|v| v.iter().cloned().collect()).unwrap_or_default(),
            _ => HashSet::new(),
        };
        SourceSets {
            group,
            library: self.cfg.library.iter().map(|p| norm(p)).collect(),
            worklist: self.cfg.worklist.iter().map(|p| norm(p)).collect(),
            results: self.results.iter().cloned().collect(),
        }
    }

    pub fn count_source(&self, src: &Source) -> usize {
        let key = format!("{src:?}");
        if let Some(&n) = self.count_cache.borrow().get(&key) {
            return n;
        }
        let n = self.count_source_uncached(src);
        self.count_cache.borrow_mut().insert(key, n);
        n
    }

    fn count_source_uncached(&self, src: &Source) -> usize {
        let mut sets = self.source_sets();
        if let Source::Group(g) = src {
            sets.group = self.cfg.groups.get(g).map(|v| v.iter().cloned().collect()).unwrap_or_default();
        }
        self.faces.iter().filter(|f| self.in_source(f, src, &sets)).count()
    }

    pub fn rebuild_view(&mut self) {
        if !self.view_dirty {
            return;
        }
        self.view_dirty = false;
        self.count_cache.borrow_mut().clear();
        if self.source == Source::Filters {
            self.ensure_traits();
        }
        let sets = self.source_sets();
        let q = self.quick_find.to_lowercase();
        let find = self.find.active.clone();
        let mut v: Vec<usize> = (0..self.faces.len())
            .filter(|&i| {
                let f = &self.faces[i];
                if self.source == Source::Filters {
                    return self.traits.get(i).map(|t| self.filters.matches(t, &self.user_tags(f))).unwrap_or(false);
                }
                if let (Source::Results(tag), Some((roots, words, sub, types))) = (&self.source, &find) {
                    if tag == "find" {
                        return find_match(f, roots, words, *sub, types);
                    }
                }
                self.in_source(f, &self.source, &sets)
            })
            .filter(|&i| {
                let f = &self.faces[i];
                let ok = match self.status(f) {
                    Status::Installed => self.show_installed,
                    Status::Activated => self.show_activated,
                    Status::NotInstalled => self.show_not_installed,
                };
                ok && (q.is_empty()
                    || f.display_name().to_lowercase().contains(&q)
                    || f.file_name.to_lowercase().contains(&q)
                    || f.family.to_lowercase().contains(&q))
            })
            .collect();
        let faces = &self.faces;
        let key = self.cfg.sort;
        v.sort_by(|&a, &b| {
            let (fa, fb) = (&faces[a], &faces[b]);
            let o = match key {
                SortBy::FontName => fa.display_name().to_lowercase().cmp(&fb.display_name().to_lowercase()),
                SortBy::Family => fa
                    .family_name()
                    .to_lowercase()
                    .cmp(&fb.family_name().to_lowercase())
                    .then(fa.weight.cmp(&fb.weight))
                    .then(fa.italic.cmp(&fb.italic)),
                SortBy::FileName => fa.file_name.to_lowercase().cmp(&fb.file_name.to_lowercase()),
                SortBy::FileSize => fa.file_size.cmp(&fb.file_size),
                SortBy::Date => fa.modified.cmp(&fb.modified),
                SortBy::Weight => fa.weight.cmp(&fb.weight),
                SortBy::Type => fa.kind.cmp(fb.kind),
                SortBy::Location => fa.folder.cmp(&fb.folder),
            };
            o.then(a.cmp(&b))
        });
        if self.cfg.sort_desc {
            v.reverse();
        }
        self.view = v;
        let in_view: HashSet<usize> = self.view.iter().copied().collect();
        self.selection.retain(|i| in_view.contains(i));
        if let Some(f) = self.focus {
            if !in_view.contains(&f) {
                self.focus = self.selection.iter().next().copied();
            }
        }
    }

    pub fn set_source(&mut self, src: Source) {
        if let Source::Folder(p) = &src {
            if !self.scanned_dirs.contains(p) {
                self.scanned_dirs.insert(p.clone());
                self.scanner.push(ScanJob::Dir { path: p.clone(), recursive: false });
            }
        }
        if self.source != src {
            self.selection.clear();
            self.focus = None;
            self.auto_selected = false;
        }
        self.source = src;
        self.view_dirty = true;
    }

    pub fn source_title(&self) -> String {
        match &self.source {
            Source::None => "Nothing selected".into(),
            Source::Installed => "Installed Fonts".into(),
            Source::Activated => "Activated Fonts".into(),
            Source::Collection(p) => format!("Collection · {}", p.display()),
            Source::Group(g) => format!("Group · {g}"),
            Source::Library => "Library".into(),
            Source::Recent(s) => format!("Recently Added · {}", s.label()),
            Source::Worklist => "Worklist".into(),
            Source::Family(f) => format!("System Fonts · {f}"),
            Source::Folder(p) => p.display().to_string(),
            Source::Filters => format!("Filtered fonts · {} filters", self.filters.count()),
            Source::Results(t) => match t.as_str() {
                "find" => "Find Fonts results".into(),
                "dups" => "Duplicate fonts".into(),
                _ => "Search results".into(),
            },
        }
    }

    // ----------------------------------------------------------------------------------------
    // Selection.

    pub fn click_select(&mut self, id: usize, modifiers: egui::Modifiers) {
        if modifiers.command {
            if !self.selection.remove(&id) {
                self.selection.insert(id);
            }
            self.anchor = Some(id);
        } else if modifiers.shift {
            let a = self.anchor.unwrap_or(id);
            let pa = self.view.iter().position(|&x| x == a).unwrap_or(0);
            let pb = self.view.iter().position(|&x| x == id).unwrap_or(0);
            let (lo, hi) = if pa <= pb { (pa, pb) } else { (pb, pa) };
            self.selection = self.view[lo..=hi].iter().copied().collect();
        } else {
            self.selection.clear();
            self.selection.insert(id);
            self.anchor = Some(id);
        }
        self.focus = Some(id);
    }

    pub fn selected_ids(&self) -> Vec<usize> {
        if self.selection.is_empty() {
            self.focus.into_iter().collect()
        } else {
            self.view.iter().copied().filter(|i| self.selection.contains(i)).collect()
        }
    }

    pub fn move_focus(&mut self, delta: i64) {
        if self.view.is_empty() {
            return;
        }
        let pos = self.focus.and_then(|f| self.view.iter().position(|&x| x == f)).map(|p| p as i64).unwrap_or(-1);
        let np = (pos + delta).clamp(0, self.view.len() as i64 - 1) as usize;
        let id = self.view[np];
        self.selection.clear();
        self.selection.insert(id);
        self.focus = Some(id);
        self.anchor = Some(id);
        self.scroll_to_focus = true;
    }

    // ----------------------------------------------------------------------------------------
    // Preview text.

    pub fn preview_text(&self, f: &FontFace) -> String {
        if !self.cfg.custom_text.trim().is_empty() && self.cfg.display == DisplayText::Custom {
            return self.cfg.custom_text.clone();
        }
        match self.cfg.display {
            DisplayText::FontName => f.display_name().to_string(),
            DisplayText::Pangram => PANGRAM.into(),
            DisplayText::Alphabet => ALPHABET.into(),
            DisplayText::Digits => DIGITS.into(),
            DisplayText::Custom => {
                if self.cfg.custom_text.trim().is_empty() {
                    f.display_name().to_string()
                } else {
                    self.cfg.custom_text.clone()
                }
            }
        }
    }

    pub fn preview_tint(&self) -> egui::Color32 {
        match self.cfg.preview_color {
            Some(c) => egui::Color32::from_rgb(c[0], c[1], c[2]),
            None => self.pal.text,
        }
    }

    pub fn preview_bg(&self) -> egui::Color32 {
        match self.cfg.preview_bg {
            Some(c) => egui::Color32::from_rgb(c[0], c[1], c[2]),
            None => self.pal.card,
        }
    }

    pub fn font_bytes(&self, path: &Path) -> Option<std::sync::Arc<Vec<u8>>> {
        self.renderer.bytes.lock().ok()?.get(path)
    }

    pub fn codepoints_for(&mut self, id: usize) -> Vec<u32> {
        if let Some((cid, v)) = &self.cps_cache {
            if *cid == id {
                return v.clone();
            }
        }
        let f = &self.faces[id];
        let v = self.font_bytes(&f.key.path).map(|d| fontdb::codepoints(&d, f.key.index)).unwrap_or_default();
        self.cps_cache = Some((id, v.clone()));
        v
    }

    // ----------------------------------------------------------------------------------------
    // Actions.

    pub fn toast(&mut self, ctx: &egui::Context, msg: impl Into<String>) {
        self.toast = Some((msg.into(), ctx.input(|i| i.time) + 4.0));
    }

    pub fn mark_cfg_dirty(&mut self) {
        self.cfg_dirty_at = Some(0.0);
        self.counts_dirty = true;
    }

    /// Find and load the font catalog the first time the Filters tab is used.
    pub fn ensure_catalog(&mut self, ctx: &egui::Context) {
        if self.catalog_checked {
            return;
        }
        self.catalog_checked = true;
        if let Some(p) = crate::catalog::find(self.cfg.catalog_path.as_deref(), &self.cfg.collections) {
            self.load_catalog(ctx, p);
        }
    }

    pub fn load_catalog(&mut self, ctx: &egui::Context, path: PathBuf) {
        self.catalog_error = None;
        self.catalog_rx = Some(crate::catalog::load_async(path, ctx.clone()));
    }

    pub fn poll_catalog(&mut self) {
        let Some(rx) = &self.catalog_rx else { return };
        let Ok(res) = rx.try_recv() else { return };
        self.catalog_rx = None;
        match res {
            Ok(c) => {
                // Make every font in the catalog known so the filters can find it.
                self.scanner.push(ScanJob::Files(c.files()));
                self.cfg.catalog_path = Some(c.path.clone());
                self.catalog = Some(std::sync::Arc::new(c));
                self.traits.clear();
                self.view_dirty = true;
                self.mark_cfg_dirty();
            }
            Err(e) => self.catalog_error = Some(e),
        }
    }

    /// Keep filter values computed for every known face.
    pub fn ensure_traits(&mut self) {
        let start = self.traits.len();
        if start < self.faces.len() {
            let cat = self.catalog.clone();
            for f in &self.faces[start..] {
                self.traits.push(std::sync::Arc::new(crate::filters::compute(f, cat.as_deref())));
            }
            self.counts_dirty = true;
        }
        if self.counts_dirty || self.filter_counts.is_none() {
            self.counts_dirty = false;
            let user: Vec<Vec<String>> = self.faces.iter().map(|f| self.user_tags(f)).collect();
            self.filter_counts = Some(crate::filters::counts(&self.traits, &user));
        }
    }

    pub fn user_tags(&self, f: &FontFace) -> Vec<String> {
        match self.cfg.tags.get(&f.key.id_string()) {
            Some(t) => t.keywords.iter().cloned().chain(t.categories.iter().cloned()).collect(),
            None => Vec::new(),
        }
    }

    pub fn save_config(&self) {
        if self.screenshot.is_some() {
            return;
        }
        let file = platform::config_file();
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let mut cfg = self.cfg.clone();
        cfg.bottom_tab = self.bottom_tab as u8;
        cfg.left_tab = self.left_tab as u8;
        cfg.right_tab = self.right_tab as u8;
        if let Ok(s) = serde_json::to_string_pretty(&cfg) {
            let _ = std::fs::write(file, s);
        }
    }

    pub fn set_theme(&mut self, ctx: &egui::Context, mode: ThemeMode) {
        self.cfg.theme = mode;
        self.pal = theme::palette(mode);
        theme::apply(ctx, mode);
        self.mark_cfg_dirty();
    }

    pub fn install_selected(&mut self, ctx: &egui::Context) {
        let ids = self.selected_ids();
        let (mut ok, mut errs) = (0, Vec::new());
        let mut new_files = Vec::new();
        for id in ids {
            let f = self.faces[id].clone();
            if self.status(&f) == Status::Installed {
                continue;
            }
            match platform::install_for_user(&f) {
                Ok(dest) => {
                    ok += 1;
                    new_files.push(dest);
                }
                Err(e) => errs.push(e),
            }
        }
        if ok > 0 {
            self.scanner.push(ScanJob::Installed);
            self.scanner.push(ScanJob::Files(new_files));
        }
        self.report(ctx, format!("Installed {ok} font(s)"), errs);
    }

    pub fn activate_selected(&mut self, ctx: &egui::Context) {
        let ids = self.selected_ids();
        let (mut ok, mut errs) = (0, Vec::new());
        for id in ids {
            let f = &self.faces[id];
            let n = norm(&f.key.path);
            if self.installed.files.contains(&n) || self.activated.contains(&n) {
                continue;
            }
            match platform::activate_temporarily(&f.key.path) {
                Ok(()) => {
                    self.activated.insert(n);
                    ok += 1;
                }
                Err(e) => errs.push(e),
            }
        }
        self.view_dirty = true;
        self.report(ctx, format!("Activated {ok} font(s) until you sign out"), errs);
    }

    pub fn request_uninstall(&mut self) {
        let ids: Vec<usize> =
            self.selected_ids().into_iter().filter(|&i| self.status(&self.faces[i]) != Status::NotInstalled).collect();
        if ids.is_empty() {
            self.modal = Modal::Message {
                title: "Nothing to remove".into(),
                body: "The selected fonts are neither installed nor activated.".into(),
            };
        } else {
            self.modal = Modal::ConfirmUninstall(ids);
        }
    }

    pub fn uninstall(&mut self, ctx: &egui::Context, ids: Vec<usize>) {
        let (mut ok, mut errs) = (0, Vec::new());
        for id in ids {
            let f = self.faces[id].clone();
            let n = norm(&f.key.path);
            if self.activated.contains(&n) {
                let _ = platform::deactivate(&f.key.path);
                self.activated.remove(&n);
                ok += 1;
                continue;
            }
            match platform::uninstall(&f, &self.installed) {
                Ok(()) => {
                    ok += 1;
                    self.installed.files.remove(&n);
                }
                Err(e) => errs.push(e),
            }
        }
        self.scanner.push(ScanJob::Installed);
        self.view_dirty = true;
        self.report(ctx, format!("Removed or deactivated {ok} font(s)"), errs);
    }

    fn report(&mut self, ctx: &egui::Context, ok_msg: String, errs: Vec<String>) {
        if errs.is_empty() {
            self.toast(ctx, ok_msg);
        } else {
            self.modal = Modal::Message { title: "Some fonts could not be changed".into(), body: format!("{ok_msg}.\n\n{}", errs.join("\n")) };
        }
    }

    pub fn attach_collection(&mut self) {
        if let Some(dir) = rfd::FileDialog::new().set_title("Attach a folder as a collection").pick_folder() {
            if !self.cfg.collections.contains(&dir) {
                self.cfg.collections.push(dir.clone());
                self.scanner.push(ScanJob::Dir { path: dir.clone(), recursive: true });
                self.scanned_dirs.insert(dir.clone());
                self.mark_cfg_dirty();
            }
            self.set_source(Source::Collection(dir));
        }
    }

    pub fn sync_collection(&mut self, dir: &Path) {
        // Re-read the folder: forget its faces so removed files disappear.
        let keep: Vec<FontFace> = self.faces.iter().filter(|f| !f.key.path.starts_with(dir)).cloned().collect();
        self.reset_faces(keep);
        self.scanner.push(ScanJob::Dir { path: dir.to_path_buf(), recursive: true });
    }

    fn reset_faces(&mut self, faces: Vec<FontFace>) {
        self.faces = faces;
        self.face_index = self.faces.iter().enumerate().map(|(i, f)| (f.key.clone(), i)).collect();
        self.selection.clear();
        self.focus = None;
        self.cps_cache = None;
        self.compare_ids.clear();
        self.dups.groups.clear();
        self.traits.clear();
        self.view_dirty = true;
    }

    pub fn add_files_to_worklist(&mut self, files: Vec<PathBuf>) {
        let mut added = Vec::new();
        for f in files {
            if fontdb::is_font_file(&f) && !self.cfg.worklist.contains(&f) {
                self.cfg.worklist.push(f.clone());
                added.push(f);
            }
        }
        if !added.is_empty() {
            self.scanner.push(ScanJob::Files(added));
        }
        self.mark_cfg_dirty();
        self.view_dirty = true;
    }

    pub fn add_selected_to_worklist(&mut self) {
        let files: Vec<PathBuf> = self.selected_ids().iter().map(|&i| self.faces[i].key.path.clone()).collect();
        self.add_files_to_worklist(files);
    }

    pub fn add_selected_to_library(&mut self, ctx: &egui::Context) {
        let mut n = 0;
        for i in self.selected_ids() {
            let p = self.faces[i].key.path.clone();
            if !self.cfg.library.contains(&p) {
                self.cfg.library.push(p.clone());
                self.cfg.recent_added.insert(norm(&p), now_secs());
                n += 1;
            }
        }
        self.mark_cfg_dirty();
        self.view_dirty = true;
        self.toast(ctx, format!("Added {n} font file(s) to the Library"));
    }

    pub fn add_folder_to_library(&mut self) {
        if let Some(dir) = rfd::FileDialog::new().set_title("Add a folder of fonts to the Library").pick_folder() {
            let mut files = Vec::new();
            collect(&dir, &mut files);
            for f in &files {
                if !self.cfg.library.contains(f) {
                    self.cfg.library.push(f.clone());
                }
            }
            self.scanner.push(ScanJob::Files(files));
            self.mark_cfg_dirty();
            self.set_source(Source::Library);
        }
    }

    pub fn create_groups_from_folders(&mut self, ctx: &egui::Context) {
        let Some(dir) = rfd::FileDialog::new().set_title("Create groups from the subfolders of…").pick_folder() else {
            return;
        };
        let mut made = 0;
        let mut all = Vec::new();
        for sub in fontdb::list_subdirs(&dir) {
            let mut files = Vec::new();
            collect(&sub, &mut files);
            if files.is_empty() {
                continue;
            }
            let name = sub.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let keys: Vec<FaceKey> = files
                .iter()
                .flat_map(|f| {
                    let n = ttc_count(f);
                    (0..n).map(move |i| FaceKey { path: f.clone(), index: i })
                })
                .collect();
            self.cfg.groups.insert(name, keys);
            all.extend(files);
            made += 1;
        }
        self.scanner.push(ScanJob::Files(all));
        self.mark_cfg_dirty();
        self.view_dirty = true;
        self.toast(ctx, format!("Created {made} group(s)"));
    }

    pub fn add_selected_to_groups(&mut self, groups: &BTreeSet<String>) {
        let keys: Vec<FaceKey> = self.selected_ids().iter().map(|&i| self.faces[i].key.clone()).collect();
        for g in groups {
            let e = self.cfg.groups.entry(g.clone()).or_default();
            for k in &keys {
                if !e.contains(k) {
                    e.push(k.clone());
                }
            }
        }
        self.mark_cfg_dirty();
        self.view_dirty = true;
    }

    pub fn copy_selected_to_folder(&mut self, ctx: &egui::Context) {
        let ids = self.selected_ids();
        if ids.is_empty() {
            return;
        }
        let Some(dir) = rfd::FileDialog::new().set_title("Copy selected font files to…").pick_folder() else { return };
        let mut files: Vec<PathBuf> = ids.iter().map(|&i| self.faces[i].key.path.clone()).collect();
        files.dedup();
        let mut n = 0;
        for f in files {
            if let Some(name) = f.file_name() {
                if std::fs::copy(&f, dir.join(name)).is_ok() {
                    n += 1;
                }
            }
        }
        self.toast(ctx, format!("Copied {n} file(s) to {}", dir.display()));
    }

    pub fn export_html(&mut self, ctx: &egui::Context) {
        let ids = if self.selected_ids().is_empty() { self.view.clone() } else { self.selected_ids() };
        if ids.is_empty() {
            return;
        }
        let Some(out) = rfd::FileDialog::new().set_file_name("fonts.html").add_filter("HTML page", &["html"]).save_file() else {
            return;
        };
        let mut css = String::new();
        let mut body = String::new();
        for (n, &i) in ids.iter().enumerate() {
            let f = &self.faces[i];
            let url = format!("file:///{}", f.key.path.display().to_string().replace('\\', "/"));
            css.push_str(&format!("@font-face {{ font-family: 'f{n}'; src: url('{url}'); }}\n"));
            body.push_str(&format!(
                "<section><h2>{}</h2><p class=meta>{} · {} · {}</p><p style=\"font-family:'f{n}';font-size:36px\">{}</p><p style=\"font-family:'f{n}';font-size:16px\">{}</p></section>\n",
                esc(f.display_name()), esc(&f.family), esc(f.style_name()), esc(&f.file_name), esc(PANGRAM), esc(ALPHABET)
            ));
        }
        let html = format!(
            "<!doctype html><meta charset=utf-8><title>Fontabulous export</title><style>{css}body{{font-family:Segoe UI,sans-serif;background:#13161c;color:#e4e6ea;margin:40px}}section{{border-bottom:1px solid #232832;padding:12px 0}}h2{{font-size:14px;margin:0;color:#f07a63}}.meta{{color:#91969f;font-size:12px;margin:2px 0}}</style>\n<h1>Fonts</h1>\n{body}"
        );
        match std::fs::write(&out, html) {
            Ok(()) => self.toast(ctx, format!("Exported {} font(s) to {}", ids.len(), out.display())),
            Err(e) => self.modal = Modal::Message { title: "Export failed".into(), body: e.to_string() },
        }
    }

    pub fn export_tags(&mut self, ctx: &egui::Context) {
        let Some(out) = rfd::FileDialog::new().set_file_name("fontabulous-tags.json").add_filter("JSON", &["json"]).save_file() else {
            return;
        };
        let tags: BTreeMap<_, _> = self.cfg.tags.iter().filter(|(_, t)| !t.is_empty()).collect();
        if let Ok(s) = serde_json::to_string_pretty(&tags) {
            let _ = std::fs::write(out, s);
            self.toast(ctx, format!("Exported tags for {} font(s)", tags.len()));
        }
    }

    pub fn import_tags(&mut self, ctx: &egui::Context) {
        let Some(p) = rfd::FileDialog::new().add_filter("JSON", &["json"]).pick_file() else { return };
        match std::fs::read_to_string(&p).ok().and_then(|s| serde_json::from_str::<HashMap<String, Tags>>(&s).ok()) {
            Some(t) => {
                let n = t.len();
                self.cfg.tags.extend(t);
                self.mark_cfg_dirty();
                self.toast(ctx, format!("Imported tags for {n} font(s)"));
            }
            None => self.modal = Modal::Message { title: "Import failed".into(), body: "That file does not contain Fontabulous tags.".into() },
        }
    }

    pub fn save_worklist(&mut self, ctx: &egui::Context) {
        let Some(out) = rfd::FileDialog::new().set_file_name("worklist.txt").add_filter("Worklist", &["txt"]).save_file() else {
            return;
        };
        let s: Vec<String> = self.cfg.worklist.iter().map(|p| p.display().to_string()).collect();
        let _ = std::fs::write(out, s.join("\n"));
        self.toast(ctx, "Worklist saved");
    }

    pub fn open_worklist(&mut self) {
        let Some(p) = rfd::FileDialog::new().add_filter("Worklist", &["txt"]).pick_file() else { return };
        if let Ok(s) = std::fs::read_to_string(p) {
            self.cfg.worklist.clear();
            let files: Vec<PathBuf> = s.lines().filter(|l| !l.trim().is_empty()).map(PathBuf::from).collect();
            self.add_files_to_worklist(files);
            self.set_source(Source::Worklist);
        }
    }

    pub fn run_search(&mut self) {
        let s = &self.search;
        let q = s.name.to_lowercase();
        let kw = s.keyword.to_lowercase();
        let foundry = s.foundry.to_lowercase();
        let caption = s.caption.to_lowercase();
        let mut out = Vec::new();
        for f in &self.faces {
            let tags = self.cfg.tags.get(&f.key.id_string());
            let ok = match s.tab {
                0 => {
                    !q.is_empty()
                        && match s.name_field {
                            1 => f.family.to_lowercase().contains(&q),
                            2 => f.full_name.to_lowercase().contains(&q),
                            3 => f.postscript.to_lowercase().contains(&q),
                            4 => f.file_name.to_lowercase().contains(&q),
                            _ => [&f.family, &f.full_name, &f.postscript, &f.file_name, &f.typo_family]
                                .iter()
                                .any(|x| x.to_lowercase().contains(&q)),
                        }
                }
                1 => !kw.is_empty() && tags.map(|t| t.keywords.iter().any(|k| k.to_lowercase().contains(&kw))).unwrap_or(false),
                2 => {
                    !s.categories.is_empty()
                        && tags.map(|t| s.categories.iter().all(|c| t.categories.contains(c))).unwrap_or(false)
                }
                3 => tags.map(|t| t.rating >= s.min_rating.max(1)).unwrap_or(false),
                4 => {
                    let w_ok = match s.weight {
                        1 => f.weight < 350,
                        2 => (350..550).contains(&f.weight),
                        3 => f.weight >= 550,
                        _ => true,
                    };
                    let i_ok = match s.italic {
                        1 => !f.italic,
                        2 => f.italic,
                        _ => true,
                    };
                    let m_ok = match s.mono {
                        1 => !f.monospaced,
                        2 => f.monospaced,
                        _ => true,
                    };
                    let st = self.status(f);
                    let s_ok = match s.status {
                        1 => st == Status::Installed,
                        2 => st == Status::Activated,
                        3 => st == Status::NotInstalled,
                        _ => true,
                    };
                    w_ok && i_ok && m_ok && s_ok
                }
                5 => {
                    !foundry.is_empty()
                        && [&f.manufacturer, &f.vendor_id, &f.designer].iter().any(|x| x.to_lowercase().contains(&foundry))
                }
                6 => !caption.is_empty() && tags.map(|t| t.caption.to_lowercase().contains(&caption)).unwrap_or(false),
                _ => false,
            };
            if ok {
                out.push(f.key.clone());
            }
        }
        self.results = out;
        self.set_source(Source::Results("search".into()));
        self.view_dirty = true;
    }

    pub fn start_find(&mut self) {
        let mut roots: Vec<PathBuf> = self.find.roots.iter().filter(|(_, on)| *on).map(|(p, _)| p.clone()).collect();
        if self.find.windows_fonts {
            roots.push(fontdb::windows_fonts_dir());
        }
        if roots.is_empty() {
            self.modal = Modal::Message { title: "Find Fonts".into(), body: "Tick at least one place to search in.".into() };
            return;
        }
        for r in &roots {
            if !self.scanned_dirs.contains(r) {
                self.scanned_dirs.insert(r.clone());
                self.scanner.push(ScanJob::Dir { path: r.clone(), recursive: self.find.subfolders });
            }
        }
        let words: Vec<String> = self.find.words.split_whitespace().map(|w| w.to_lowercase()).collect();
        self.find.active = Some((roots, words, self.find.subfolders, self.find.types));
        self.set_source(Source::Results("find".into()));
        self.view_dirty = true;
    }

    pub fn run_duplicates(&mut self) {
        let ids: Vec<usize> = if self.dups.all_known { (0..self.faces.len()).collect() } else { self.view.clone() };
        let mut map: HashMap<String, Vec<usize>> = HashMap::new();
        for i in ids {
            let f = &self.faces[i];
            let mut key = String::new();
            if self.dups.by_name {
                key.push_str(&f.display_name().to_lowercase());
                key.push('|');
            }
            if self.dups.by_size {
                key.push_str(&f.file_size.to_string());
                key.push('|');
            }
            if self.dups.by_content {
                key.push_str(&format!("{:x}|", file_hash(&f.key.path)));
            }
            if key.is_empty() {
                key = f.display_name().to_lowercase();
            }
            map.entry(key).or_default().push(i);
        }
        let mut groups: Vec<Vec<usize>> = map.into_values().filter(|v| {
            let paths: HashSet<&PathBuf> = v.iter().map(|&i| &self.faces[i].key.path).collect();
            v.len() > 1 && paths.len() > 1
        }).collect();
        groups.sort_by_key(|g| self.faces[g[0]].display_name().to_lowercase());
        // Pre-check every copy except the first (the one to keep).
        self.dups.checked = groups.iter().flat_map(|g| g.iter().skip(1).copied()).collect();
        self.dups.groups = groups;
        self.dups.ran = true;
    }

    pub fn detect_problems(&mut self) {
        let mut out = Vec::new();
        for n in &self.installed.files {
            if !Path::new(n).exists() {
                out.push(("Missing file".to_string(), format!("Registered font file not found: {n}")));
            }
        }
        for f in &self.faces {
            if self.status(f) == Status::NotInstalled {
                continue;
            }
            if !f.unicode && !f.symbol {
                out.push(("No Unicode map".into(), format!("{} ({}) has no Unicode character map", f.display_name(), f.file_name)));
            }
            if f.embedding == "Restricted License" {
                out.push(("Restricted embedding".into(), format!("{} cannot be embedded in documents", f.display_name())));
            }
            if f.glyph_count < 5 {
                out.push(("Almost empty".into(), format!("{} has only {} glyphs", f.display_name(), f.glyph_count)));
            }
        }
        // Same full name installed from more than one file.
        let mut by_name: HashMap<String, Vec<&FontFace>> = HashMap::new();
        for f in self.faces.iter().filter(|f| self.status(f) == Status::Installed) {
            by_name.entry(f.display_name().to_lowercase()).or_default().push(f);
        }
        for v in by_name.values() {
            let files: HashSet<&PathBuf> = v.iter().map(|f| &f.key.path).collect();
            if files.len() > 1 {
                let list: Vec<String> = files.iter().map(|p| p.display().to_string()).collect();
                out.push(("Installed twice".into(), format!("{}: {}", v[0].display_name(), list.join(", "))));
            }
        }
        out.sort();
        self.problems = out;
        self.problems_open = true;
    }

    pub fn installed_families(&self) -> Vec<(String, usize, bool)> {
        let mut m: BTreeMap<String, (String, usize, bool)> = BTreeMap::new();
        for f in &self.faces {
            if self.status(f) == Status::NotInstalled {
                continue;
            }
            let fam = f.family_name().to_string();
            let e = m.entry(fam.to_lowercase()).or_insert((fam, 0, false));
            e.1 += 1;
            if f.is_postscript() || f.opentype_layout {
                e.2 = true;
            }
        }
        m.into_values().collect()
    }

    pub fn counts(&self) -> (usize, usize) {
        let perm = self.installed.files.len();
        (perm, self.activated.len())
    }
}

pub struct SourceSets {
    pub group: HashSet<FaceKey>,
    pub library: HashSet<String>,
    pub worklist: HashSet<String>,
    pub results: HashSet<FaceKey>,
}

fn find_match(f: &FontFace, roots: &[PathBuf], words: &[String], sub: bool, types: &[bool; 4]) -> bool {
    let in_root = roots.iter().any(|r| if sub { f.key.path.starts_with(r) } else { f.folder == *r });
    if !in_root {
        return false;
    }
    let ext = f.key.path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let ti = match ext.as_str() {
        "ttf" => 0,
        "otf" => 1,
        "ttc" => 2,
        _ => 3,
    };
    if !types[ti] {
        return false;
    }
    let name = format!("{} {} {}", f.display_name(), f.family, f.file_name).to_lowercase();
    words.iter().all(|w| name.contains(w))
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect(&p, out);
        } else if fontdb::is_font_file(&p) {
            out.push(p);
        }
    }
}

fn ttc_count(p: &Path) -> u32 {
    let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if ext != "ttc" && ext != "otc" {
        return 1;
    }
    std::fs::read(p).ok().and_then(|d| ttf_parser::fonts_in_collection(&d)).unwrap_or(1)
}

fn file_hash(p: &Path) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    if let Ok(d) = std::fs::read(p) {
        d.hash(&mut h);
    }
    h.finish()
}

pub fn now_secs() -> u64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or(Duration::ZERO).as_secs()
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn load_config() -> Config {
    let p = platform::config_file();
    std::fs::read_to_string(p).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

pub fn human_size(n: u64) -> String {
    if n >= 1024 * 1024 {
        format!("{:.1} MB", n as f64 / 1048576.0)
    } else {
        format!("{} KB", n.div_ceil(1024))
    }
}

pub fn fmt_time(t: Option<SystemTime>) -> String {
    let Some(t) = t else { return String::new() };
    let Ok(d) = t.duration_since(SystemTime::UNIX_EPOCH) else { return String::new() };
    let days = (d.as_secs() / 86400) as i64;
    // Civil-from-days (Howard Hinnant).
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let dd = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{dd:02}")
}

impl eframe::App for FontApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.frame_no += 1;
        self.poll_scanner();
        self.poll_catalog();
        self.renderer.begin_frame(&ctx);
        self.rebuild_view();
        if !self.auto_selected && !self.view.is_empty() && self.focus.is_none() {
            let first = self.view[0];
            self.selection.insert(first);
            self.focus = Some(first);
            self.anchor = Some(first);
            self.auto_selected = true;
        }
        self.handle_keys(&ctx);

        crate::ui_panels::title_bar(self, ui);
        crate::ui_panels::context_bar(self, ui);
        crate::ui_panels::status_bar(self, ui);
        if self.cfg.show_left {
            crate::ui_panels::left_panel(self, ui);
        }
        if self.cfg.show_right {
            crate::ui_panels::right_panel(self, ui);
        }
        egui::CentralPanel::default().frame(egui::Frame::new().fill(self.pal.canvas)).show(ui, |ui| {
            if self.cfg.show_bottom {
                crate::ui_center::bottom_panel(self, ui);
            }
            crate::ui_center::main_view(self, ui);
        });
        crate::ui_dialogs::dialogs(self, &ctx);
        crate::ui_panels::resize_edges(&ctx);

        if self.cfg_dirty_at.is_some() {
            self.cfg_dirty_at = None;
            self.save_config();
        }
        self.screenshot_step(&ctx);
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        self.save_config();
    }
}

impl FontApp {
    fn handle_keys(&mut self, ctx: &egui::Context) {
        if ctx.memory(|m| m.focused().is_some()) {
            return;
        }
        let (down, up, all, home, end, pgdn, pgup) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::ArrowDown) || i.key_pressed(egui::Key::ArrowRight),
                i.key_pressed(egui::Key::ArrowUp) || i.key_pressed(egui::Key::ArrowLeft),
                i.modifiers.command && i.key_pressed(egui::Key::A),
                i.key_pressed(egui::Key::Home),
                i.key_pressed(egui::Key::End),
                i.key_pressed(egui::Key::PageDown),
                i.key_pressed(egui::Key::PageUp),
            )
        });
        if down {
            self.move_focus(1);
        }
        if up {
            self.move_focus(-1);
        }
        if pgdn {
            self.move_focus(10);
        }
        if pgup {
            self.move_focus(-10);
        }
        if home {
            self.move_focus(-(self.view.len() as i64));
        }
        if end {
            self.move_focus(self.view.len() as i64);
        }
        if all {
            self.selection = self.view.iter().copied().collect();
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::F)) {
            self.find.open = true;
        }
    }

    fn screenshot_step(&mut self, ctx: &egui::Context) {
        let Some(path) = self.screenshot.clone() else { return };
        ctx.request_repaint();
        let settled = !self.scanner.busy && self.renderer.pending() == 0 && !self.faces.is_empty();
        if !self.shot_requested && self.frame_no >= 120 && (settled || self.frame_no > 900) {
            self.shot_requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        let shot = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(img) = shot {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let [w, h] = img.size;
            let mut buf = Vec::with_capacity(w * h * 4);
            for c in &img.pixels {
                buf.extend_from_slice(&c.to_srgba_unmultiplied());
            }
            if let Some(rgba) = image::RgbaImage::from_raw(w as u32, h as u32, buf) {
                let _ = rgba.save(&path);
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}
