# vidi.yazi

A [yazi](https://github.com/sxyazi/yazi) plugin that uses [vidi](https://github.com/ChrisGVE/vidi) as a universal file previewer and opener.

- **Previewer**: renders any file type inline in the yazi preview pane via `vidi --inline`.
- **Opener**: launches vidi full-screen when you open a file from yazi.

## Requirements

- yazi ≥ 0.4
- vidi installed and on your `$PATH`

## Installation

### 1. Install vidi

Once published to crates.io:

```sh
cargo install vidi
```

Until then, build from source:

```sh
git clone https://github.com/ChrisGVE/vidi
cd vidi
cargo install --path .
```

### 2. Install the plugin

Copy the `vidi.yazi` directory to yazi's plugin folder:

```sh
# macOS / Linux
cp -r vidi.yazi ~/.config/yazi/plugins/

# Or, if you cloned the vidi repo:
cp -r contrib/yazi/vidi.yazi ~/.config/yazi/plugins/
```

### 3. Configure the previewer

Yazi already handles images, plain text, source code, PDF first-page thumbnails, and video thumbnails natively. vidi adds value for the formats yazi does not cover:

| Format | vidi adds |
|---|---|
| Ebooks (epub, mobi, djvu) | Full content rendering via `epy` or `pandoc` |
| Office documents (docx, odt, pptx) | Text extraction via `doxx` or `pandoc` |
| Spreadsheets (xlsx, ods, numbers) | Tabular preview via `visidata` or `sc-im` |
| CSV / tabular | Interactive view via `csvlens` or `tidy-viewer` |
| Audio files | Metadata table via `ffprobe` |
| LaTeX / Typst | Source + compiled PDF toggle |
| Archives | Contents listing via `ouch` |

The recommended configuration activates vidi only for these formats:

```toml
[plugin]
prepend_previewers = [
  { mime = "application/epub+zip",    run = "vidi" },
  { mime = "application/x-mobipocket-ebook", run = "vidi" },
  { mime = "image/vnd.djvu",          run = "vidi" },
  { mime = "application/vnd.openxmlformats-officedocument.*", run = "vidi" },
  { mime = "application/vnd.oasis.opendocument.*", run = "vidi" },
  { mime = "application/vnd.ms-excel*", run = "vidi" },
  { mime = "application/vnd.openxmlformats-officedocument.spreadsheetml*", run = "vidi" },
  { name = "*.numbers",               run = "vidi" },
  { name = "*.csv",                   run = "vidi" },
  { name = "*.tsv",                   run = "vidi" },
  { mime = "audio/*",                 run = "vidi" },
  { name = "*.tex",                   run = "vidi" },
  { name = "*.typ",                   run = "vidi" },
  { mime = "application/zip",         run = "vidi" },
  { mime = "application/gzip",        run = "vidi" },
  { mime = "application/x-tar",       run = "vidi" },
  { mime = "application/x-7z-compressed", run = "vidi" },
  { mime = "application/x-xz",        run = "vidi" },
  { mime = "application/zstd",        run = "vidi" },
]
```

If you prefer vidi to handle everything and fall back to yazi's built-in previewers for anything it does not cover, use the catch-all rule instead:

```toml
[plugin]
prepend_previewers = [
  { name = "*", run = "vidi" },
]
```

### 4. Configure the opener (optional)

To open files with vidi in full-screen mode when pressing Enter in yazi,
add to `~/.config/yazi/yazi.toml`:

```toml
[opener]
vidi = [
  { run = 'vidi "$@"', block = true, for = "unix" },
]

[open]
prepend_rules = [
  { name = "*", use = "vidi" },
]
```

`block = true` keeps yazi suspended while vidi runs, restoring the yazi UI
cleanly when you quit vidi.

## Theme detection

The plugin attempts to map the active yazi Catppuccin flavor to the matching
vidi theme.  For other themes, set `VIDI_THEME` in your shell environment:

```sh
export VIDI_THEME=catppuccin-mocha
```

Supported values mirror vidi's built-in theme names (e.g. `catppuccin-latte`,
`catppuccin-frappe`, `catppuccin-macchiato`, `catppuccin-mocha`).
