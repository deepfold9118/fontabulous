# Fontabulous

A font manager for Windows: browse, preview, compare, organise, install and
uninstall fonts. Written in Rust with [egui](https://github.com/emilk/egui),
with [Lucide](https://lucide.dev) icons and a light and a dark theme in the
style of the ArtCraft apps
([VectorCraft](https://github.com/deepfold9118/vectorcraft),
[PhotoCraft](https://github.com/deepfold9118/photocraft)).

Its companion font editor is
[Fontabulous Studio](https://github.com/deepfold9118/fontabulous-studio).

## Download

Get `Fontabulous-Setup-<version>.exe` from the
[latest release](https://github.com/deepfold9118/fontabulous/releases/latest).
It installs for the current user (no administrator rights needed), adds a
Start menu entry and a desktop shortcut, and appears in Windows' Installed
apps, where it can be uninstalled. The release also has the bare
`fontabulous.exe`, which runs without installing.

## What it does

- **Browse** fonts in folders, the installed Windows fonts, and a Worklist
  of loose font files (which can be saved and reopened). Show them as a
  list, tiles or details, sorted as you like.
- **Preview** with a text sample, a characters table and a waterfall, and
  read each font's properties (names, version, designer, copyright and
  more).
- **Install**, **activate temporarily** (usable until you sign out, without
  installing) and **uninstall or deactivate** fonts.
- **Organise** with groups (also created from a folder structure), a
  library, ratings and tags, including one-click Usage and Mood tags. Tags
  can be exported and imported.
- **Filters**: classification, mood and usage tags, recommendations,
  typographic properties (weight, width, x-height, contrast), language
  support and foundry, with a count on every chip. Values come from a
  font-library catalog (`_Catalog\catalog.json`) when one is attached, and
  are measured from the font files otherwise.
- **Tools**: Find Fonts, Search for Duplicates, a side-by-side Compare,
  Show Kerning Pairs, Detect Font Problems, export to an HTML page, and copy
  font files to a folder.

Settings, groups and tags are kept in `%APPDATA%\Fontabulous\config.json`.
Set `FONTABULOUS_CONFIG` to another file to keep them elsewhere (for
portable use).

## Building

Needs Rust and the Visual Studio C++ build tools.

```powershell
cargo build --release
```

`build-installer.ps1` builds the program and the installer (`installer\`,
a small egui program that embeds the release build) with the build folders
stripped from them, and writes both to `dist\` with SHA-256 files.

## Licence

Apache-2.0. See [LICENSE](LICENSE).

The interface uses the [Inter](https://rsms.me/inter/) typeface (SIL Open
Font License 1.1, [assets/OFL-Inter.txt](assets/OFL-Inter.txt)) and the
[Lucide](https://lucide.dev) icon font (ISC,
[assets/LICENSE-lucide.txt](assets/LICENSE-lucide.txt)).
