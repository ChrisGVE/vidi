# Changelog

All notable changes to this project will be documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] — 2026-03-17

### Added

**HTML as a first-class file kind**
- `FileKind::Html` added for `.html`, `.htm`, `.xhtml` — previously treated as
  plain text; now routed through its own tool chain and internal renderer
- External tool chain: w3m → lynx → bat (html syntax) → cat

**Internal renderer tier**
- New `renderer` module providing `internal_render()` and `has_internal_renderer()`
- `FileKind::Html` and `FileKind::Ebook` (epub) have built-in renderers; no
  external tool required for basic rendering
- HTML renderer: scraper-based DOM walker producing ANSI-styled text (bold
  headings, indented code/blockquotes, bullet list items, `<br>` newlines)
- EPUB renderer: iterates the spine via `rbook` and feeds each chapter through
  the HTML renderer with an embedded image resolver; chapter separator emitted
  between chapters
- Image rendering: `chafa --format=symbols` for in-terminal images; falls back
  to `[image: filename]` placeholder when chafa is absent
- Fullscreen internal output: written to a temp file, paged via `less -R`
- Inline internal output: passed through existing `truncate_ansi_safe` logic

**Per-kind viewer preferences (`[viewer]` config table)**
- New `[viewer]` table in `~/.config/vidi/config.toml`
- Keys match `FileKind::config_key()` values; valid keys: `text`, `markdown`,
  `image`, `video`, `audio`, `pdf`, `ebook`, `html`, `office`, `spreadsheet`,
  `csv`, `json`, `yaml`, `toml`, `latex`, `typst`, `archive`, `binary`
- `"internal"` selects the internal renderer for the kind
- Any other value is a soft tool preference (falls back to registry if not
  installed)
- Resolution order: `--tool` flag → `viewer[kind]` config → Default (internal
  if available, then registry)

### Changed

- `.html`, `.htm`, `.xhtml` extension detection moved from `FileKind::Text` to
  `FileKind::Html`
- Viewer dispatch in `main.rs` extended to handle `ViewerChoice` variants and
  internal renderer paths

### Dependencies

- `rbook = "0.7"` — EPUB parsing
- `scraper = "0.22"` — HTML5 DOM parsing

## [0.1.0] — 2026-03-14

Initial release.

### Added

**Core detection**
- File-type detection via extension map (170+ extensions across 17 categories),
  magic-byte sniffing (`infer` crate), and UTF-8 content fallback
- 17 file categories: Text, Markdown, Image, Video, Audio, PDF, Ebook,
  OfficeDocs, Spreadsheet, CSV, LaTeX, Typst, JSON, YAML, TOML, Archive, Binary

**Terminal graphics**
- Automatic graphics protocol detection: Kitty → WezTerm/Ghostty → iTerm2 →
  Sixel → Unicode half-block 24-bit → Unicode half-block 256-colour
- Detection via environment variables (instant, no roundtrip)

**Viewer delegation**
- Priority-ordered tool registry for all 17 file categories
- In-process probe cache: each binary is checked against PATH at most once
- Universal fallbacks: `cat` for text, `xxd` for binary (always available)
- Supported tools include: bat, glow, mdcat, chafa, viu, timg, mpv, ffprobe,
  zathura, mutool, pdftotext, epy, pandoc, doxx, visidata, sc-im, csvlens,
  tidy-viewer, miller, jless, jq, yq, taplo, ouch, bsdtar, hexyl, xxd,
  tectonic, typst, and more

**Theming**
- 12 built-in named themes: catppuccin-mocha (default), catppuccin-latte,
  catppuccin-frappe, catppuccin-macchiato, tokyonight, gruvbox-dark,
  gruvbox-light, nord, dracula, solarized-dark, solarized-light, one-dark
- Custom themes defined as TOML colour palettes in `config.toml`
- Theme cascaded to every delegated tool via tool-specific flag mapping
- `VIDI_THEME` environment variable for host-tool integration (yazi, etc.)
- Theme resolution order: `VIDI_THEME` → `--theme` flag → config → default

**Output modes**
- Full-screen mode (default): replaces the vidi process via `exec()` with the
  selected tool, or spawns and waits where exec is not appropriate
- Inline mode (`--inline --lines N`): captures tool stdout and truncates to N
  lines with ANSI-safe boundary handling (no corrupted escape sequences)
- Stdin support: `vidi -` reads from stdin into a temporary file

**LaTeX and Typst toggle mode**
- Full-screen toggle between rendered view and source view
- LaTeX: compiled via `tectonic` (Rust-based, auto-downloads packages)
- Typst: compiled via `typst compile`
- Page rendered to PNG via `mutool draw`, displayed via `chafa`
- Compilation runs asynchronously; source view shown immediately
- Keys: `r` rendered, `s` source, `q`/`Esc`/`Ctrl-C` quit
- Graceful fallback to source-only when compiler or mutool is absent

**Audio and video**
- Full-screen: ffprobe metadata table (container, duration, bitrate, codec,
  resolution/sample rate) + optional first-frame preview for video
- Full-screen: `[p] play  [q] quit` prompt; playback via mpv
  (`--vo=kitty` on Kitty terminals for in-terminal video)
- Inline: metadata table only (no playback prompt)

**Configuration**
- XDG-compliant config at `~/.config/vidi/config.toml`
- Configurable: theme, custom theme palettes, per-category tool overrides,
  extra binary search paths
- Missing config file returns defaults silently (not an error)

**CLI**
- `vidi <file>` — full-screen view
- `vidi --inline [--lines N] <file>` — constrained stdout output
- `vidi --theme NAME <file>` — override theme
- `vidi --tool NAME <file>` — force a specific tool
- `vidi --config PATH <file>` — use alternate config file
- `vidi -` — read from stdin

**Yazi integration** (`contrib/yazi/`)
- `vidi.yazi/init.lua` — previewer plugin using `vidi --inline`
- Installation guide and opener configuration snippet
- Keymap snippet binding `V` to full-screen vidi

**CI**
- GitHub Actions: build + test + clippy + fmt on push/PR (ubuntu + macos)
- Release workflow: cross-compiled binaries for x86_64/aarch64 Linux and macOS,
  attached to GitHub Releases on version tags

[0.1.0]: https://github.com/ChrisGVE/vidi/releases/tag/v0.1.0
