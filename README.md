# vidi

Opens any file in the terminal. Detects the type, picks the best installed viewer, and renders it — full-screen or inline.

[![CI](https://github.com/ChrisGVE/vidi/actions/workflows/ci.yml/badge.svg)](https://github.com/ChrisGVE/vidi/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/vidi)](https://crates.io/crates/vidi)
[![docs.rs](https://img.shields.io/docsrs/vidi)](https://docs.rs/vidi)

## Installation

```sh
cargo install vidi
```

Rust 1.82 or later is required.

## Usage

```sh
vidi <file>              # full-screen view (replaces the shell)
vidi --inline <file>     # write output to stdout, constrained to terminal height
vidi --inline --lines 30 <file>   # constrained to 30 lines
vidi --theme catppuccin-latte <file>
vidi --tool bat <file>   # force a specific viewer
vidi -                   # read from stdin
```

## Supported formats

| Category | Example extensions | Preferred tool | Fallback |
|---|---|---|---|
| Source code / text | `.rs` `.py` `.js` `.sh` `.txt` | bat | highlight, cat |
| Markdown | `.md` `.markdown` | glow | mdcat, bat |
| Images | `.jpg` `.png` `.gif` `.webp` `.svg` | chafa | viu, timg |
| Video | `.mp4` `.mkv` `.mov` | timg | mpv |
| Audio | `.mp3` `.flac` `.ogg` `.wav` | ffprobe (metadata) | — |
| PDF | `.pdf` | zathura | mutool, pdftotext |
| Ebooks | `.epub` `.mobi` `.djvu` | epy | pandoc |
| Office documents | `.docx` `.odt` `.pptx` | doxx | pandoc |
| Spreadsheets | `.xlsx` `.ods` `.numbers` | visidata | sc-im |
| CSV / tabular | `.csv` `.tsv` | csvlens | tidy-viewer, miller |
| JSON | `.json` `.jsonl` | jless | jq, bat |
| YAML | `.yaml` `.yml` | yq | bat |
| TOML | `.toml` | taplo | bat |
| LaTeX | `.tex` `.sty` `.cls` | tectonic + bat (toggle) | bat |
| Typst | `.typ` | typst + bat (toggle) | bat |
| Archives | `.tar.gz` `.zip` `.7z` | ouch | bsdtar |
| Binary / hex | any unrecognised binary | hexyl | xxd |

None of the preferred tools are required. vidi probes what is installed and selects the best available option. `cat` and `xxd` are the universal fallbacks for text and binary respectively.

## Theming

The default theme is `catppuccin-mocha`. vidi translates the active theme into tool-specific flags before launching each viewer, so bat, glow, and other tools all reflect the same palette.

**Built-in themes**

- `catppuccin-mocha` (default, dark)
- `catppuccin-latte` (light)
- `catppuccin-frappe` (dark)
- `catppuccin-macchiato` (dark)

**Selecting a theme**

In priority order, highest first:

1. `VIDI_THEME` environment variable
2. `--theme <name>` CLI flag
3. `theme` key in `~/.config/vidi/config.toml`
4. Built-in default (`catppuccin-mocha`)

The `VIDI_THEME` variable is intended for host tools (e.g. yazi) that want vidi's output to match their own active theme.

**Custom themes**

Define custom themes in `config.toml` as TOML tables with `name`, `bg`, `fg`, `cursor`, a 16-entry `ansi` array, and a 4-entry `accents` array. Each color is an RGB table `{ r = 0, g = 0, b = 0 }`.

## Configuration

Config file location: `~/.config/vidi/config.toml` (respects `XDG_CONFIG_HOME`).

All keys are optional. Missing file or missing keys fall back to defaults.

```toml
# Active theme name. Built-in: catppuccin-mocha, catppuccin-latte,
# catppuccin-frappe, catppuccin-macchiato.
theme = "catppuccin-mocha"

# Per-tool argument overrides. The key is the tool name as listed in
# the supported formats table above.
[tool_overrides]
bat = ["--paging=always", "--style=numbers,changes"]

# Additional directories to search for viewer binaries beyond PATH.
extra_search_paths = ["/opt/homebrew/bin"]

# Custom theme definitions (optional).
# [[custom_themes]]
# name = "my-theme"
# bg   = { r = 30,  g = 30,  b = 46  }
# fg   = { r = 205, g = 214, b = 244 }
# cursor = { r = 245, g = 224, b = 220 }
# ansi = [
#   { r = 69,  g = 71,  b = 90  },   # 0  black
#   { r = 243, g = 139, b = 168 },   # 1  red
#   { r = 166, g = 227, b = 161 },   # 2  green
#   { r = 249, g = 226, b = 175 },   # 3  yellow
#   { r = 137, g = 180, b = 250 },   # 4  blue
#   { r = 245, g = 194, b = 231 },   # 5  magenta
#   { r = 148, g = 226, b = 213 },   # 6  cyan
#   { r = 186, g = 194, b = 222 },   # 7  white
#   { r = 88,  g = 91,  b = 112 },   # 8  bright black
#   { r = 243, g = 139, b = 168 },   # 9  bright red
#   { r = 166, g = 227, b = 161 },   # 10 bright green
#   { r = 249, g = 226, b = 175 },   # 11 bright yellow
#   { r = 137, g = 180, b = 250 },   # 12 bright blue
#   { r = 245, g = 194, b = 231 },   # 13 bright magenta
#   { r = 148, g = 226, b = 213 },   # 14 bright cyan
#   { r = 205, g = 214, b = 244 },   # 15 bright white
# ]
# accents = [
#   { r = 203, g = 166, b = 247 },   # mauve
#   { r = 250, g = 179, b = 135 },   # peach
#   { r = 137, g = 220, b = 235 },   # sky
#   { r = 116, g = 199, b = 236 },   # sapphire
# ]
```

## Yazi integration

vidi can serve as both a **previewer** and a **full-screen opener** inside [yazi](https://github.com/sxyazi/yazi).

Configuration files will be provided in `contrib/yazi/` in a future release. In the meantime:

- **Previewer**: call `vidi --inline --lines $YAZI_PREVIEW_HEIGHT "$1"` from a yazi `prepend_previewers` rule.
- **Opener**: call `vidi "$1"` with `block = true` in a yazi `open` rule.
- Set `VIDI_THEME` to match yazi's active theme so colors are consistent.

## LaTeX and Typst

For `.tex` and `.typ` files, vidi opens a toggle view:

- The initial view shows the source with syntax highlighting via `bat`.
- Press `r` to compile and display the rendered PDF (requires `tectonic` for LaTeX or `typst compile` for Typst, plus `mutool` for PDF-to-image conversion).
- Press `s` to return to the source view.
- Press `q` to quit.

Compilation runs asynchronously; the source view is shown immediately while rendering is in progress.

## Requirements

No tool is strictly required. vidi degrades gracefully:

- **Text and source code**: `cat` is the universal fallback (POSIX standard).
- **Binary files**: `xxd` is the universal fallback (ships with vim).

For richer output, install any combination of the tools listed in the supported formats table. Homebrew users on macOS can install the full set with:

```sh
brew install bat glow chafa viu timg ffmpeg mpv zathura mupdf-tools \
     poppler pandoc visidata csvlens miller jless jq yq taplo ouch hexyl
```

## License

MIT
