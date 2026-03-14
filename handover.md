# Vidi — Project Handover

## What is vidi

`vidi` is a universal terminal file viewer CLI tool. Its core purpose is to detect
the type of any file passed to it, identify the best available installed utility on
the system, and present the file in a consistent scrollable/browsable interface in
the terminal — including graphical terminals (Kitty, iTerm2, WezTerm, etc.).

The name comes from Latin *vidi* ("I saw"), as in *veni, vidi, vici*.

## Key design decisions (confirmed with user)

- **Not a file manager** — no filesystem navigation, no copy/move/delete. Opens a
  specific file and renders it. This differentiates it from yazi, ranger, lf, nnn.
- **Delegation model** — vidi is a thin orchestration layer. It detects file type
  and dispatches to the best installed tool (bat, glow, chafa, ffplay, zathura,
  etc.). It does not reimplement rendering.
- **Two output modes** (required from day one):
  - Default: full-screen TUI (standalone use, yazi opener)
  - `--inline` / `--lines N`: constrained output (yazi previewer, scripting)
- **Yazi integration** is a planned but secondary deliverable:
  - As a previewer plugin (`vidi.yazi`) via `--inline` mode
  - As a full-screen opener via `block = true` in yazi.toml
  - Integration ships as optional config, not baked into core
- **Language**: Rust (confirmed). Fits the ecosystem (bat, viu, yazi are all Rust),
  crates.io publishing, and CLAUDE.md standards for docs.rs documentation.

## What exists so far

- GitHub repo created: https://github.com/ChrisGVE/vidi (public, empty)
- Local folder: /Users/chris/dev/tools/vidi
- No code, no PRD, no task-master initialization yet

## What needs to be done next (in order)

1. ~~**Decide language**~~ — **Rust confirmed**
2. **Create PRD** — use template at `/Users/chris/.claude/PRD.txt`, store in
   `.taskmaster/docs/`, follow naming convention `YYYYMMDD-HHMM_vidi_0.1.0_PRD_initialization.txt`
3. **Initialize task-master** — `task-master init`, then `task-master parse-prd`
4. **Register project with workspace-qdrant**
5. **Create FIRST-PRINCIPLES.md**
6. **Begin implementation** per task-master tasks

## Viewer delegation candidates (reference)

| File category | Candidate tools |
|---|---|
| Source code / text | `bat`, `highlight`, `source-highlight`, `cat` |
| Markdown | `glow`, `mdcat`, `bat` |
| Images | `kitty icat`, `viu`, `chafa`, `timg` |
| Video | `ffplay`, `mpv` (inline thumbnails via chafa/ffmpeg) |
| PDF | `zathura`, `mupdf`, `pdftotext` + bat |
| Archives | `atool`, `bsdtar -tv` |
| Hex / binary | `xv`, `hexyl`, `xxd` |
| CSV / tabular | `tidy-viewer`, `visidata` |
| JSON / YAML / TOML | `jq`, `bat` |
| Directories | not in scope (yazi handles this) |

## Comparable tools (for differentiation)

- **yazi** — file manager with preview pane; vidi targets the viewer slot yazi
  delegates to, not yazi's own space
- **rifle** (ranger's opener) — dispatches to tools but does not provide a unified
  browsing experience; no inline mode
- **xdg-open** — launches GUI apps, no terminal rendering
- **viu** — images only
- **bat** — text/code only
- None of the above combine: type detection + best-tool dispatch + unified scroll
  interface + inline mode

## GitHub repo

https://github.com/ChrisGVE/vidi
