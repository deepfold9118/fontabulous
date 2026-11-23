//! Font discovery and metadata: scans folders in a background thread and parses
//! each face's naming, style and metric information with ttf-parser.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::time::SystemTime;

pub const FONT_EXTS: &[&str] = &["ttf", "otf", "ttc", "otc"];

pub fn is_font_file(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| FONT_EXTS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct FaceKey {
    pub path: PathBuf,
    pub index: u32,
}

impl FaceKey {
    pub fn id_string(&self) -> String {
        format!("{}#{}", self.path.display(), self.index)
    }
}

#[derive(Clone, Debug)]
pub struct FontFace {
    pub key: FaceKey,
    /// Lower-cased path used for fast lookups.
    pub npath: String,
    pub file_name: String,
    pub folder: PathBuf,
    pub file_size: u64,
    pub modified: Option<SystemTime>,
    pub created: Option<SystemTime>,
    pub kind: &'static str,
    pub family: String,
    pub subfamily: String,
    pub full_name: String,
    pub postscript: String,
    pub typo_family: String,
    pub typo_subfamily: String,
    pub unique_id: String,
    pub version: String,
    pub copyright: String,
    pub trademark: String,
    pub manufacturer: String,
    pub designer: String,
    pub description: String,
    pub license: String,
    pub vendor_url: String,
    pub vendor_id: String,
    pub weight: u16,
    pub width: u16,
    pub italic: bool,
    pub bold: bool,
    pub monospaced: bool,
    pub kerning: bool,
    pub unicode: bool,
    pub symbol: bool,
    pub opentype_layout: bool,
    pub variable: bool,
    pub embedding: &'static str,
    pub glyph_count: u16,
    pub units_per_em: u16,
    pub ascender: i16,
    pub descender: i16,
    pub line_gap: i16,
    pub x_height: Option<i16>,
    pub cap_height: Option<i16>,
    pub family_class: i16,
    pub panose: [u8; 10],
    pub tables: Vec<String>,
    /// Writing systems covered (bit i = coverage::SCRIPTS[i]).
    pub scripts: u32,
    /// Languages covered (bit i = coverage::LANGS[i]).
    pub langs: u64,
    /// No separate lowercase letters (all caps / titling).
    pub caps_only: bool,
    /// x-height divided by cap height, when the font records both.
    pub xh_ratio: Option<f32>,
}

impl FontFace {
    pub fn display_name(&self) -> &str {
        if !self.full_name.is_empty() {
            &self.full_name
        } else if !self.family.is_empty() {
            &self.family
        } else {
            &self.file_name
        }
    }
    pub fn family_name(&self) -> &str {
        if !self.typo_family.is_empty() {
            &self.typo_family
        } else if !self.family.is_empty() {
            &self.family
        } else {
            &self.file_name
        }
    }
    pub fn style_name(&self) -> &str {
        if !self.typo_subfamily.is_empty() {
            &self.typo_subfamily
        } else {
            &self.subfamily
        }
    }
    pub fn is_postscript(&self) -> bool {
        self.tables.iter().any(|t| t == "CFF " || t == "CFF2")
    }
}

pub fn weight_name(w: u16) -> &'static str {
    match w {
        0..=149 => "Thin",
        150..=249 => "Extra Light",
        250..=349 => "Light",
        350..=449 => "Regular",
        450..=549 => "Medium",
        550..=649 => "Semi Bold",
        650..=749 => "Bold",
        750..=849 => "Extra Bold",
        _ => "Black",
    }
}

/// Short badge shown on tiles, e.g. "B" for bold, "L" for light.
pub fn weight_badge(w: u16, italic: bool) -> String {
    let b = match w {
        0..=149 => "T",
        150..=249 => "XL",
        250..=349 => "L",
        350..=449 => "",
        450..=549 => "M",
        550..=649 => "SB",
        650..=749 => "B",
        750..=849 => "XB",
        _ => "BL",
    };
    let mut s = b.to_string();
    if italic {
        s.push('I');
    }
    s
}

pub fn width_name(w: u16) -> &'static str {
    match w {
        1 => "Ultra Condensed",
        2 => "Extra Condensed",
        3 => "Condensed",
        4 => "Semi Condensed",
        5 => "Medium (Normal)",
        6 => "Semi Expanded",
        7 => "Expanded",
        8 => "Extra Expanded",
        9 => "Ultra Expanded",
        _ => "Medium (Normal)",
    }
}

pub fn family_class_name(c: i16) -> &'static str {
    match (c >> 8) & 0xff {
        0 => "No Classification",
        1 => "Oldstyle Serifs",
        2 => "Transitional Serifs",
        3 => "Modern Serifs",
        4 => "Clarendon Serifs",
        5 => "Slab Serifs",
        7 => "Freeform Serifs",
        8 => "Sans Serif",
        9 => "Ornamentals",
        10 => "Scripts",
        12 => "Symbolic",
        _ => "Reserved",
    }
}

pub fn panose_family_name(p: u8) -> &'static str {
    match p {
        0 => "Any",
        1 => "No Fit",
        2 => "Latin Text",
        3 => "Latin Hand Written",
        4 => "Latin Decorative",
        5 => "Latin Symbol",
        _ => "Unknown",
    }
}

pub fn panose_serif_name(p: u8) -> &'static str {
    match p {
        0 => "Any",
        1 => "No Fit",
        2 => "Cove",
        3 => "Obtuse Cove",
        4 => "Square Cove",
        5 => "Obtuse Square Cove",
        6 => "Square",
        7 => "Thin",
        8 => "Oval",
        9 => "Exaggerated",
        10 => "Triangle",
        11 => "Normal Sans",
        12 => "Obtuse Sans",
        13 => "Perpendicular Sans",
        14 => "Flared",
        15 => "Rounded",
        _ => "Unknown",
    }
}

pub fn panose_proportion_name(p: u8) -> &'static str {
    match p {
        0 => "Any",
        1 => "No Fit",
        2 => "Old Style",
        3 => "Modern",
        4 => "Even Width",
        5 => "Extended",
        6 => "Condensed",
        7 => "Very Extended",
        8 => "Very Condensed",
        9 => "Monospaced",
        _ => "Unknown",
    }
}

fn name_of(face: &ttf_parser::Face, id: u16) -> String {
    let names = face.names();
    let mut fallback = None;
    for n in names {
        if n.name_id != id {
            continue;
        }
        if n.is_unicode() {
            if let Some(s) = n.to_string() {
                // Prefer English (Windows 0x409) names.
                if n.platform_id == ttf_parser::PlatformId::Windows && n.language_id == 0x409 {
                    return s.trim().to_string();
                }
                if fallback.is_none() {
                    fallback = Some(s);
                }
            }
        } else if fallback.is_none() && n.platform_id == ttf_parser::PlatformId::Macintosh {
            // Mac Roman: treat as Latin-1 which is close enough for names.
            let s: String = n.name.iter().map(|&b| b as char).collect();
            fallback = Some(s);
        }
    }
    fallback.unwrap_or_default().trim().to_string()
}

fn embedding_name(face: &ttf_parser::Face) -> &'static str {
    use ttf_parser::Permissions;
    match face.permissions() {
        Some(Permissions::Installable) => "Installable",
        Some(Permissions::Restricted) => "Restricted License",
        Some(Permissions::PreviewAndPrint) => "Preview & Print",
        Some(Permissions::Editable) => "Editable",
        None => "Unknown",
    }
}

pub fn parse_face(data: &[u8], path: &Path, index: u32, meta: &std::fs::Metadata, kind: &'static str) -> Option<FontFace> {
    let face = ttf_parser::Face::parse(data, index).ok()?;
    let raw = face.raw_face();
    let mut tables: Vec<String> = Vec::new();
    // Table directory: read tags from the raw face data via known table list.
    for tag in KNOWN_TABLES {
        let t = ttf_parser::Tag::from_bytes(&tag_bytes(tag));
        if raw.table(t).is_some() {
            tables.push(tag.to_string());
        }
    }
    let os2 = raw.table(ttf_parser::Tag::from_bytes(b"OS/2"));
    let (vendor_id, family_class, panose) = match os2 {
        Some(d) if d.len() >= 62 => {
            let v: String = d[58..62].iter().map(|&b| b as char).collect();
            let fc = i16::from_be_bytes([d[30], d[31]]);
            let mut p = [0u8; 10];
            p.copy_from_slice(&d[32..42]);
            (v.trim().to_string(), fc, p)
        }
        _ => (String::new(), 0, [0u8; 10]),
    };
    let mut unicode = false;
    let mut symbol = false;
    if let Some(cmap) = face.tables().cmap {
        for st in cmap.subtables {
            if st.is_unicode() {
                unicode = true;
            }
            if st.platform_id == ttf_parser::PlatformId::Windows && st.encoding_id == 0 {
                symbol = true;
            }
        }
    }
    let file_name = path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let has = |t: &str| tables.iter().any(|x| x == t);
    let kerning = has("kern") || has("GPOS");
    let opentype_layout = has("GSUB") || has("GPOS");
    let variable = has("fvar");
    let (scripts, langs, caps_only) = crate::coverage::measure(&face);
    let xh_ratio = match (face.x_height(), face.capital_height()) {
        (Some(x), Some(c)) if x > 0 && c > 0 => Some(x as f32 / c as f32),
        _ => None,
    };
    let kind: &'static str = if kind == "collection" {
        if has("CFF ") { "OpenType Collection" } else { "TrueType Collection" }
    } else if has("CFF ") || has("CFF2") {
        "OpenType/PS"
    } else if opentype_layout || has("DSIG") {
        "OpenType/TT"
    } else {
        "TrueType"
    };
    Some(FontFace {
        key: FaceKey { path: path.to_path_buf(), index },
        npath: norm(path),
        file_name,
        folder: path.parent().map(|p| p.to_path_buf()).unwrap_or_default(),
        file_size: meta.len(),
        modified: meta.modified().ok(),
        created: meta.created().ok(),
        kind,
        family: name_of(&face, 1),
        subfamily: name_of(&face, 2),
        unique_id: name_of(&face, 3),
        full_name: name_of(&face, 4),
        version: name_of(&face, 5),
        postscript: name_of(&face, 6),
        copyright: name_of(&face, 0),
        trademark: name_of(&face, 7),
        manufacturer: name_of(&face, 8),
        designer: name_of(&face, 9),
        description: name_of(&face, 10),
        vendor_url: name_of(&face, 11),
        license: name_of(&face, 13),
        typo_family: name_of(&face, 16),
        typo_subfamily: name_of(&face, 17),
        vendor_id,
        weight: face.weight().to_number(),
        width: face.width().to_number(),
        italic: face.is_italic() || face.is_oblique(),
        bold: face.is_bold(),
        monospaced: face.is_monospaced(),
        kerning,
        unicode,
        symbol,
        opentype_layout,
        variable,
        embedding: embedding_name(&face),
        glyph_count: face.number_of_glyphs(),
        units_per_em: face.units_per_em(),
        ascender: face.ascender(),
        descender: face.descender(),
        line_gap: face.line_gap(),
        x_height: face.x_height(),
        cap_height: face.capital_height(),
        family_class,
        panose,
        tables,
        scripts,
        langs,
        caps_only,
        xh_ratio,
    })
}

fn tag_bytes(s: &str) -> [u8; 4] {
    let mut b = [b' '; 4];
    for (i, c) in s.bytes().take(4).enumerate() {
        b[i] = c;
    }
    b
}

const KNOWN_TABLES: &[&str] = &[
    "BASE", "CBDT", "CBLC", "CFF ", "CFF2", "COLR", "CPAL", "DSIG", "EBDT", "EBLC", "GDEF", "GPOS", "GSUB",
    "HVAR", "JSTF", "MATH", "MVAR", "OS/2", "STAT", "SVG ", "VDMX", "VORG", "VVAR", "avar", "cmap", "cvt ",
    "fpgm", "fvar", "gasp", "glyf", "gvar", "hdmx", "head", "hhea", "hmtx", "kern", "loca", "maxp", "meta",
    "name", "post", "prep", "sbix", "vhea", "vmtx",
];

/// Parse every face in one file.
pub fn scan_file(path: &Path) -> Vec<FontFace> {
    let mut out = Vec::new();
    let Ok(meta) = std::fs::metadata(path) else { return out };
    let Ok(file) = std::fs::File::open(path) else { return out };
    // SAFETY: the file is only read; if it changes underneath us parsing may fail, which is handled.
    let Ok(map) = (unsafe { memmap2::Mmap::map(&file) }) else { return out };
    let data: &[u8] = &map;
    let count = ttf_parser::fonts_in_collection(data);
    match count {
        Some(n) => {
            for i in 0..n {
                if let Some(f) = parse_face(data, path, i, &meta, "collection") {
                    out.push(f);
                }
            }
        }
        None => {
            if let Some(f) = parse_face(data, path, 0, &meta, "single") {
                out.push(f);
            }
        }
    }
    out
}

/// Unicode code points mapped by the face (for the characters table).
pub fn codepoints(data: &[u8], index: u32) -> Vec<u32> {
    let mut set: Vec<u32> = Vec::new();
    if let Ok(face) = ttf_parser::Face::parse(data, index) {
        if let Some(cmap) = face.tables().cmap {
            let mut seen = HashSet::new();
            for st in cmap.subtables {
                let sym = st.platform_id == ttf_parser::PlatformId::Windows && st.encoding_id == 0;
                if !(st.is_unicode() || sym) {
                    continue;
                }
                st.codepoints(|c| {
                    if st.glyph_index(c).map(|g| g.0 != 0).unwrap_or(false) && seen.insert(c) {
                        set.push(c);
                    }
                });
            }
        }
    }
    set.sort_unstable();
    set
}

/// Kerning pairs from the legacy 'kern' table (format 0).
pub fn kerning_pairs(data: &[u8], index: u32, limit: usize) -> Vec<(char, char, i16)> {
    let mut out = Vec::new();
    let Ok(face) = ttf_parser::Face::parse(data, index) else { return out };
    let Some(kern) = face.tables().kern else { return out };
    // Map glyph ids back to chars for the Latin range we display.
    let mut rev: HashMap<u16, char> = HashMap::new();
    for c in (0x20u32..0x250).filter_map(char::from_u32) {
        if let Some(g) = face.glyph_index(c) {
            rev.entry(g.0).or_insert(c);
        }
    }
    let chars: Vec<(char, ttf_parser::GlyphId)> =
        (0x21u32..0x17f).filter_map(char::from_u32).filter_map(|c| face.glyph_index(c).map(|g| (c, g))).collect();
    for st in kern.subtables {
        if !st.horizontal || st.variable {
            continue;
        }
        for (a, ga) in &chars {
            for (b, gb) in &chars {
                if let Some(v) = st.glyphs_kerning(*ga, *gb) {
                    if v != 0 {
                        out.push((*a, *b, v));
                        if out.len() >= limit {
                            return out;
                        }
                    }
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Installed-font discovery (Windows registry).

#[derive(Default, Clone)]
pub struct InstalledInfo {
    /// Lower-cased absolute paths of permanently installed font files.
    pub files: HashSet<String>,
    /// Lower-cased paths of fonts installed per-user (removable without admin).
    pub user_files: HashSet<String>,
    /// Registry value names by lower-cased path (needed for uninstall).
    pub value_names: HashMap<String, String>,
    /// The same files with their original spelling, for scanning and display.
    pub paths: Vec<PathBuf>,
}

pub fn windows_fonts_dir() -> PathBuf {
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
    PathBuf::from(windir).join("Fonts")
}

pub fn user_fonts_dir() -> PathBuf {
    let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
    PathBuf::from(local).join("Microsoft").join("Windows").join("Fonts")
}

pub fn norm(p: &Path) -> String {
    p.to_string_lossy().to_lowercase().replace('/', "\\")
}

#[cfg(windows)]
pub fn read_installed() -> InstalledInfo {
    use winreg::enums::*;
    use winreg::RegKey;
    let mut info = InstalledInfo::default();
    let sys_dir = windows_fonts_dir();
    let key_path = "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Fonts";
    for (hive, is_user) in [(HKEY_LOCAL_MACHINE, false), (HKEY_CURRENT_USER, true)] {
        let Ok(k) = RegKey::predef(hive).open_subkey(key_path) else { continue };
        for (name, _) in k.enum_values().flatten() {
            let Ok(v): Result<String, _> = k.get_value(&name) else { continue };
            let p = PathBuf::from(&v);
            let full = if p.is_absolute() { p } else { sys_dir.join(p) };
            if !is_font_file(&full) {
                continue;
            }
            let n = norm(&full);
            if !info.files.contains(&n) {
                info.paths.push(full.clone());
            }
            if is_user {
                info.user_files.insert(n.clone());
            }
            info.value_names.insert(n.clone(), name.clone());
            info.files.insert(n);
        }
    }
    // Every font sitting in the Windows Fonts folder counts as installed.
    if let Ok(rd) = std::fs::read_dir(&sys_dir) {
        for e in rd.flatten() {
            let p = e.path();
            if is_font_file(&p) && info.files.insert(norm(&p)) {
                info.paths.push(p);
            }
        }
    }
    info
}

#[cfg(not(windows))]
pub fn read_installed() -> InstalledInfo {
    InstalledInfo::default()
}

// ---------------------------------------------------------------------------
// Background scanner.

#[derive(Clone, Debug)]
pub enum ScanJob {
    Installed,
    Dir { path: PathBuf, recursive: bool },
    Files(Vec<PathBuf>),
}

pub enum ScanMsg {
    Installed(InstalledInfo),
    Faces(Vec<FontFace>),
    DirDone(PathBuf),
    Progress(String),
    Idle,
}

pub struct Scanner {
    queue: Arc<(Mutex<VecDeque<ScanJob>>, Condvar)>,
    pub rx: Receiver<ScanMsg>,
    pub busy: bool,
}

impl Scanner {
    pub fn new(ctx: egui::Context) -> Self {
        let queue: Arc<(Mutex<VecDeque<ScanJob>>, Condvar)> = Arc::new((Mutex::new(VecDeque::new()), Condvar::new()));
        let (tx, rx) = channel();
        let q = queue.clone();
        std::thread::Builder::new()
            .name("font-scanner".into())
            .spawn(move || scanner_thread(q, tx, ctx))
            .ok();
        Self { queue, rx, busy: false }
    }

    pub fn push(&mut self, job: ScanJob) {
        let (m, cv) = &*self.queue;
        m.lock().unwrap().push_back(job);
        cv.notify_one();
        self.busy = true;
    }
}

fn scanner_thread(q: Arc<(Mutex<VecDeque<ScanJob>>, Condvar)>, tx: Sender<ScanMsg>, ctx: egui::Context) {
    let mut done_files: HashSet<PathBuf> = HashSet::new();
    loop {
        let job = {
            let (m, cv) = &*q;
            let mut g = m.lock().unwrap();
            loop {
                if let Some(j) = g.pop_front() {
                    break j;
                }
                let _ = tx.send(ScanMsg::Idle);
                ctx.request_repaint();
                g = cv.wait(g).unwrap();
            }
        };
        match job {
            ScanJob::Installed => {
                let info = read_installed();
                let mut files: Vec<PathBuf> = info.paths.clone();
                files.sort();
                files.dedup_by(|a, b| norm(a) == norm(b));
                let _ = tx.send(ScanMsg::Installed(info));
                scan_list(&files, &mut done_files, &tx, &ctx);
            }
            ScanJob::Dir { path, recursive } => {
                let _ = tx.send(ScanMsg::Progress(format!("Scanning {}", path.display())));
                let mut files = Vec::new();
                collect_files(&path, recursive, &mut files, 0);
                files.sort();
                scan_list(&files, &mut done_files, &tx, &ctx);
                let _ = tx.send(ScanMsg::DirDone(path));
            }
            ScanJob::Files(files) => scan_list(&files, &mut done_files, &tx, &ctx),
        }
        ctx.request_repaint();
    }
}

fn scan_list(files: &[PathBuf], done: &mut HashSet<PathBuf>, tx: &Sender<ScanMsg>, ctx: &egui::Context) {
    let mut batch = Vec::new();
    for (i, f) in files.iter().enumerate() {
        if !done.insert(f.clone()) {
            continue;
        }
        batch.extend(scan_file(f));
        if batch.len() >= 64 {
            let _ = tx.send(ScanMsg::Faces(std::mem::take(&mut batch)));
            let _ = tx.send(ScanMsg::Progress(format!("Reading fonts… {}/{}", i + 1, files.len())));
            ctx.request_repaint();
        }
    }
    if !batch.is_empty() {
        let _ = tx.send(ScanMsg::Faces(batch));
    }
}

fn collect_files(dir: &Path, recursive: bool, out: &mut Vec<PathBuf>, depth: usize) {
    if depth > 24 || out.len() > 200_000 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_dir() {
            if recursive {
                collect_files(&p, true, out, depth + 1);
            }
        } else if is_font_file(&p) {
            out.push(p);
        }
    }
}

/// Count of font files directly inside a folder (used by the folder browser).
pub fn list_subdirs(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                .map(|e| e.path())
                .filter(|p| {
                    !p.file_name().map(|n| n.to_string_lossy().starts_with('$')).unwrap_or(false)
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort_by_key(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default());
    v
}

pub fn drives() -> Vec<PathBuf> {
    let mut v = Vec::new();
    #[cfg(windows)]
    for c in b'A'..=b'Z' {
        let p = PathBuf::from(format!("{}:\\", c as char));
        if p.exists() {
            v.push(p);
        }
    }
    #[cfg(not(windows))]
    v.push(PathBuf::from("/"));
    v
}
