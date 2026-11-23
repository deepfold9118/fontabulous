//! Off-thread text rasterisation for font previews. Every preview is drawn with the
//! font file itself (ab_glyph) into a white coverage mask that the UI tints, so
//! previews follow the theme colours without re-rendering.

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use egui::{Color32, ColorImage, TextureHandle, TextureOptions, Vec2};
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};

use crate::fontdb::FaceKey;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct Job {
    pub face: FaceKey,
    pub text: String,
    /// Em size in physical pixels.
    pub px: u32,
    /// Wrap width in physical pixels (0 = single line, clipped to max_w).
    pub wrap: u32,
    /// Maximum width in physical pixels for single lines.
    pub max_w: u32,
    /// Centre the glyph (character cells).
    pub center: bool,
}

impl Job {
    pub fn key(&self) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.hash(&mut h);
        h.finish()
    }
}

struct Done {
    key: u64,
    image: Option<ColorImage>,
}

// ---------------------------------------------------------------------------
// Shared font-file cache.

#[derive(Default)]
pub struct BytesCache {
    map: HashMap<PathBuf, Arc<Vec<u8>>>,
    order: VecDeque<PathBuf>,
    total: usize,
}

impl BytesCache {
    pub fn get(&mut self, p: &Path) -> Option<Arc<Vec<u8>>> {
        if let Some(b) = self.map.get(p) {
            return Some(b.clone());
        }
        let data = Arc::new(std::fs::read(p).ok()?);
        self.total += data.len();
        self.map.insert(p.to_path_buf(), data.clone());
        self.order.push_back(p.to_path_buf());
        while (self.total > 384 * 1024 * 1024 || self.order.len() > 160) && self.order.len() > 1 {
            if let Some(old) = self.order.pop_front() {
                if let Some(d) = self.map.remove(&old) {
                    self.total -= d.len();
                }
            }
        }
        Some(data)
    }
}

pub type SharedBytes = Arc<Mutex<BytesCache>>;

// ---------------------------------------------------------------------------

struct Entry {
    tex: Option<TextureHandle>,
    size: Vec2,
    pending: bool,
    failed: bool,
    last_used: u64,
}

pub struct Renderer {
    stack: Arc<(Mutex<Vec<(u64, Job)>>, Condvar)>,
    rx: Receiver<Done>,
    entries: HashMap<u64, Entry>,
    frame: u64,
    pub bytes: SharedBytes,
}

impl Renderer {
    pub fn new(ctx: &egui::Context) -> Self {
        let stack: Arc<(Mutex<Vec<(u64, Job)>>, Condvar)> = Arc::new((Mutex::new(Vec::new()), Condvar::new()));
        let (tx, rx) = channel::<Done>();
        let bytes: SharedBytes = Arc::new(Mutex::new(BytesCache::default()));
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(2, 6);
        for i in 0..threads {
            let st = stack.clone();
            let tx: Sender<Done> = tx.clone();
            let b = bytes.clone();
            let c = ctx.clone();
            std::thread::Builder::new()
                .name(format!("raster-{i}"))
                .spawn(move || worker(st, tx, b, c))
                .ok();
        }
        Self { stack, rx, entries: HashMap::new(), frame: 0, bytes }
    }

    /// Call once per frame before drawing.
    pub fn begin_frame(&mut self, ctx: &egui::Context) {
        self.frame += 1;
        while let Ok(d) = self.rx.try_recv() {
            if let Some(e) = self.entries.get_mut(&d.key) {
                e.pending = false;
                match d.image {
                    Some(img) => {
                        let ppp = ctx.pixels_per_point();
                        e.size = Vec2::new(img.size[0] as f32 / ppp, img.size[1] as f32 / ppp);
                        e.tex = Some(ctx.load_texture(format!("pv{}", d.key), img, TextureOptions::LINEAR));
                    }
                    None => e.failed = true,
                }
            }
        }
        // Evict textures that have not been used for a while.
        if self.entries.len() > 1800 {
            let cutoff = self.frame.saturating_sub(120);
            self.entries.retain(|_, e| e.pending || e.last_used >= cutoff);
        }
    }

    pub fn pending(&self) -> usize {
        self.entries.values().filter(|e| e.pending).count()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.stack.0.lock().unwrap().clear();
    }

    /// Returns the texture (if ready) and its size in points.
    pub fn get(&mut self, job: Job) -> Preview {
        let key = job.key();
        let frame = self.frame;
        if let Some(e) = self.entries.get_mut(&key) {
            e.last_used = frame;
            if let Some(t) = &e.tex {
                return Preview::Ready(t.id(), e.size);
            }
            if e.failed {
                return Preview::Failed;
            }
            return Preview::Pending;
        }
        self.entries.insert(key, Entry { tex: None, size: Vec2::ZERO, pending: true, failed: false, last_used: frame });
        let (m, cv) = &*self.stack;
        let mut s = m.lock().unwrap();
        s.push((key, job));
        // Keep the newest requests; old ones get re-requested if still visible.
        if s.len() > 600 {
            let dropped: Vec<(u64, Job)> = s.drain(0..200).collect();
            drop(s);
            for (k, _) in dropped {
                self.entries.remove(&k);
            }
        } else {
            drop(s);
        }
        cv.notify_one();
        Preview::Pending
    }
}

#[derive(Clone, Copy)]
pub enum Preview {
    Ready(egui::TextureId, Vec2),
    Pending,
    Failed,
}

fn worker(stack: Arc<(Mutex<Vec<(u64, Job)>>, Condvar)>, tx: Sender<Done>, bytes: SharedBytes, ctx: egui::Context) {
    loop {
        let (key, job) = {
            let (m, cv) = &*stack;
            let mut g = m.lock().unwrap();
            loop {
                if let Some(j) = g.pop() {
                    break j;
                }
                g = cv.wait(g).unwrap();
            }
        };
        let data = bytes.lock().unwrap().get(&job.face.path);
        let image = data.and_then(|d| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| rasterize(&d, &job))).ok().flatten()
        });
        let _ = tx.send(Done { key, image });
        ctx.request_repaint();
    }
}

fn glyph_for(font: &FontRef, c: char) -> ab_glyph::GlyphId {
    let g = font.glyph_id(c);
    if g.0 == 0 {
        // Symbol fonts map their glyphs into U+F0xx.
        if let Some(alt) = char::from_u32(0xF000 + (c as u32 & 0xFF)) {
            let g2 = font.glyph_id(alt);
            if g2.0 != 0 {
                return g2;
            }
        }
    }
    g
}

/// Text to show when the font has hardly any of the requested characters.
fn fallback_text(font: &FontRef, text: &str) -> Option<String> {
    let chars: Vec<char> = text.chars().filter(|c| !c.is_whitespace()).collect();
    if chars.is_empty() {
        return None;
    }
    let missing = chars.iter().filter(|&&c| glyph_for(font, c).0 == 0).count();
    if missing * 2 <= chars.len() {
        return None;
    }
    let mut out = String::new();
    let mut n = 0;
    for (_, c) in font.codepoint_ids() {
        if !c.is_control() && !c.is_whitespace() {
            out.push(c);
            n += 1;
            if n >= 40 {
                break;
            }
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

pub fn rasterize(data: &[u8], job: &Job) -> Option<ColorImage> {
    let font = FontRef::try_from_slice_and_index(data, job.face.index).ok()?;
    let upem = font.units_per_em().unwrap_or(1000.0).max(1.0);
    let height = font.height_unscaled().max(1.0);
    let scale = PxScale::from(job.px as f32 * height / upem);
    let sf = font.as_scaled(scale);
    let text = if job.center { job.text.clone() } else { fallback_text(&font, &job.text).unwrap_or_else(|| job.text.clone()) };
    let ascent = sf.ascent();
    let descent = sf.descent();
    let line_h = (ascent - descent + sf.line_gap()).max(1.0);

    // Layout into lines.
    let mut lines: Vec<Vec<(ab_glyph::GlyphId, f32)>> = vec![Vec::new()];
    let wrap = job.wrap as f32;
    let mut x = 0.0f32;
    let mut prev: Option<ab_glyph::GlyphId> = None;
    let words: Vec<&str> = text.split_inclusive(|c: char| c == ' ' || c == '\n').collect();
    for word in words {
        // Measure word.
        let mut w = 0.0;
        let mut p = prev;
        for c in word.chars() {
            if c == '\n' {
                continue;
            }
            let g = glyph_for(&font, c);
            if let Some(pg) = p {
                w += sf.kern(pg, g);
            }
            w += sf.h_advance(g);
            p = Some(g);
        }
        if wrap > 0.0 && x > 0.0 && x + w > wrap {
            lines.push(Vec::new());
            x = 0.0;
            prev = None;
        }
        for c in word.chars() {
            if c == '\n' {
                lines.push(Vec::new());
                x = 0.0;
                prev = None;
                continue;
            }
            let g = glyph_for(&font, c);
            if let Some(pg) = prev {
                x += sf.kern(pg, g);
            }
            lines.last_mut().unwrap().push((g, x));
            x += sf.h_advance(g);
            prev = Some(g);
        }
        if job.wrap == 0 && job.max_w > 0 && x > job.max_w as f32 * 1.2 {
            break;
        }
    }

    let content_w = lines
        .iter()
        .filter_map(|l| l.last().map(|(g, x)| x + sf.h_advance(*g)))
        .fold(0.0f32, f32::max)
        .ceil();
    let (img_w, img_h) = if job.center {
        (job.max_w.max(1) as usize, job.max_w.max(1) as usize)
    } else {
        let mut w = content_w.max(1.0) as usize + 2;
        if job.max_w > 0 {
            w = w.min(job.max_w as usize);
        }
        if job.wrap > 0 {
            w = w.min(job.wrap as usize + 2);
        }
        (w, ((lines.len() as f32 - 1.0) * line_h + ascent - descent).ceil().max(1.0) as usize + 2)
    };
    if img_w * img_h > 16_000_000 {
        return None;
    }
    let mut cov = vec![0u8; img_w * img_h];
    let (ox, oy) = if job.center {
        let gw = content_w;
        ((img_w as f32 - gw) / 2.0, (img_h as f32 - (ascent - descent)) / 2.0 + ascent)
    } else {
        (1.0, ascent + 1.0)
    };
    for (li, line) in lines.iter().enumerate() {
        let base_y = oy + li as f32 * line_h;
        for (g, gx) in line {
            let glyph = g.with_scale_and_position(scale, ab_glyph::point(ox + gx, base_y));
            if let Some(og) = font.outline_glyph(glyph) {
                let b = og.px_bounds();
                og.draw(|px, py, c| {
                    let xx = b.min.x as i32 + px as i32;
                    let yy = b.min.y as i32 + py as i32;
                    if xx >= 0 && yy >= 0 && (xx as usize) < img_w && (yy as usize) < img_h {
                        let i = yy as usize * img_w + xx as usize;
                        let v = (c.clamp(0.0, 1.0) * 255.0) as u8;
                        cov[i] = cov[i].saturating_add(v);
                    }
                });
            }
        }
    }
    let pixels: Vec<Color32> = cov.into_iter().map(|a| Color32::from_white_alpha(a)).collect();
    Some(ColorImage::new([img_w, img_h], pixels))
}
