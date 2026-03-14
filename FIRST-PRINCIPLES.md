# First Principles — vidi

## Principle 1: Test Driven Development

**Philosophy**: Systematic TDD — write unit tests immediately after each logical unit of code.

**Implementation implications**:

- Each logical unit (function, object, method) needs ≥1 unit test
- Cover edge cases and validation errors (multiple tests per unit)
- Run tests after atomic changes; amend tests only after first run
- Use LSP to identify calling/called code relationships
- When testing for scenarios involving the filesystem (access rights, missing
  file or folder) ALWAYS use mock tests and NEVER manipulate the filesystem itself

## Principle 2: Leverage Existing Solutions

**Philosophy**: Reuse mature, well-maintained libraries rather than reinventing functionality.

**Implementation implications**:

- Prefer established, actively maintained libraries with strong community support
- Choose mature solutions with proven track record (but not stale/unmaintained)
- Follow standard protocols and interfaces when available
- Ensure compatibility with existing toolchains and ecosystems
- Evaluate library health: recent updates, active issues/PRs, documentation quality
- Align with industry best practices and conventions

## Principle 3: Delegation Over Reimplementation

**Philosophy**: vidi is an orchestration layer, not a renderer. It detects, selects, and
launches the best available tool — it does not reimplement what those tools already do.

**Implementation implications**:

- Never reimplement syntax highlighting, image rendering, PDF display, or any
  capability that an existing ecosystem tool already handles well
- The value vidi adds is: detection + selection + theming + unified interface
- Keep the core binary small; complexity lives in the delegated tools
- Exception: vidi owns the toggle event loop for LaTeX/Typst (no existing tool
  provides rendered↔source switching in the terminal)

## Principle 4: Terminal-First Rendering

**Philosophy**: All output targets the terminal. Graphics rendering uses the best
available protocol for the running terminal, degrades gracefully otherwise.

**Implementation implications**:

- Detect terminal capabilities at startup (Kitty → iTerm2 → Sixel → half-block)
- Use environment variables first (instant), escape-sequence queries second
- Every file category must have a rendering path that works in a plain 256-color
  terminal — graphical protocols are enhancements, not requirements
- No GUI apps, no external windows, no browser rendering

## Principle 5: Coherent Theming

**Philosophy**: vidi applies a single configured theme consistently across every
delegated tool. The user configures once; vidi cascades everywhere.

**Implementation implications**:

- The `ThemeMapper` translates the active `Theme` to per-tool flags before launch
- When called from a host tool (yazi, etc.), `VIDI_THEME` overrides local config
  so vidi's output is visually coherent with the surrounding environment
- Tools that do not support theming accept native defaults — document which these are
- Custom themes are defined as TOML palettes; vidi generates tool-specific formats
  (glamour JSON for glow, bat theme name, zathura color values, etc.)

## Principle 6: Graceful Degradation

**Philosophy**: vidi always produces output. Every file category has a universal
fallback; no invocation should end in a confusing blank screen.

**Implementation implications**:

- Every `FileKind` has at least one fallback that is universally available
  (xxd for binary, cat for text — both POSIX/standard)
- Probe tools lazily and fail to the next candidate silently
- Only surface an error when no candidate at all can be found AND the universal
  fallback itself is unavailable
- Missing optional tools are not errors — they are expected on minimal systems

## Principle 7: Performance

**Philosophy**: vidi must not feel slow. It sits between the user and the content;
any latency it adds is friction.

**Implementation implications**:

- Target: < 50 ms from invocation to first output (excluding tool render time)
- Probe tool availability lazily: only probe candidates for the detected `FileKind`
- Cache probe results in-process (probe once per invocation, not per call)
- For LaTeX/Typst toggle: begin compilation asynchronously, show source (bat) first,
  switch to rendered view when compilation completes

## Principle 8: Minimal Surface Area

**Philosophy**: vidi does one thing — opens a file and shows it. It is not a file
manager, not a shell, not a framework.

**Implementation implications**:

- No filesystem navigation, no copy/move/delete
- No plugin system in v0.1.0
- CLI surface is small: `<FILE>`, `--inline`, `--lines`, `--theme`, `--tool`, `--config`
- The yazi integration is shipped as config files, not baked into the binary
- Resist feature creep: if a new capability requires a new rendering engine,
  it belongs in a delegated tool, not in vidi itself
