//! Reads the person's font-library catalog (`_Catalog/catalog.json`), which records
//! Adobe Fonts-style filter values for every family, and maps each font file to it.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;

use crate::fontdb::norm;

#[derive(Debug, Default, Clone, serde::Deserialize)]
#[serde(default)]
pub struct CatFile {
    pub path: String,
}

#[derive(Debug, Default, Clone, serde::Deserialize)]
#[serde(default)]
pub struct CatFamily {
    pub foundry: String,
    pub family: String,
    pub adobe_foundry: Option<String>,
    pub designer: Option<String>,
    pub classification: Vec<String>,
    pub tags: Vec<String>,
    pub recommendations: Vec<String>,
    pub weight: Vec<String>,
    pub width: Vec<String>,
    pub x_height: Option<String>,
    pub contrast: Option<String>,
    pub caps_only: bool,
    pub number_style: Vec<String>,
    pub variable: bool,
    pub writing_systems: Vec<String>,
    pub languages: Vec<String>,
    pub files: Vec<CatFile>,
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(default)]
struct CatalogJson {
    root: String,
    families: Vec<CatFamily>,
    languages: BTreeMap<String, String>,
}

pub struct Catalog {
    pub path: PathBuf,
    pub root: PathBuf,
    pub families: Vec<CatFamily>,
    /// Lower-cased absolute file path -> family index.
    pub by_path: HashMap<String, usize>,
    pub languages: BTreeMap<String, String>,
}

impl Catalog {
    pub fn files(&self) -> Vec<PathBuf> {
        self.families.iter().flat_map(|f| f.files.iter().map(|x| self.root.join(&x.path))).collect()
    }
}

pub fn load(path: &Path) -> Result<Catalog, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("Could not read {}: {e}", path.display()))?;
    let json: CatalogJson = serde_json::from_str(&text).map_err(|e| format!("{} is not a font catalog: {e}", path.display()))?;
    let root = if json.root.is_empty() {
        // Default: the catalog sits in <root>\_Catalog\catalog.json
        path.parent().and_then(|p| p.parent()).map(|p| p.to_path_buf()).unwrap_or_default()
    } else {
        PathBuf::from(&json.root)
    };
    let mut by_path = HashMap::new();
    for (i, f) in json.families.iter().enumerate() {
        for file in &f.files {
            by_path.insert(norm(&root.join(&file.path)), i);
        }
    }
    Ok(Catalog { path: path.to_path_buf(), root, families: json.families, by_path, languages: json.languages })
}

pub fn load_async(path: PathBuf, ctx: egui::Context) -> Receiver<Result<Catalog, String>> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(load(&path));
        ctx.request_repaint();
    });
    rx
}

/// Look for a catalog: the saved path, `_Catalog\catalog.json` in an attached
/// collection, or in a `Fonts` folder at the root of any drive.
pub fn find(saved: Option<&Path>, collections: &[PathBuf]) -> Option<PathBuf> {
    if let Some(p) = saved {
        if p.is_file() {
            return Some(p.to_path_buf());
        }
    }
    for c in collections {
        let p = c.join("_Catalog").join("catalog.json");
        if p.is_file() {
            return Some(p);
        }
    }
    for d in crate::fontdb::drives() {
        let p = d.join("Fonts").join("_Catalog").join("catalog.json");
        if p.is_file() {
            return Some(p);
        }
    }
    None
}
