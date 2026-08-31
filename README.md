# rustiq

Interactive TUI git diff viewer with syntax highlighting, inline comments, and theme support.

## Install

```bash
cargo build --release
# optionally:
cp target/release/rustiq ~/.local/bin/
```

## Usage

Run inside any git repository:

```bash
rustiq
rustiq --inline   # PTY hosts that cannot read the alternate screen
```

On launch, choose a baseline (commit or working tree). Then browse changed files and their diffs.

## DeepSeek Harness

Plugin: [`dsh-tool-rustiq/`](dsh-tool-rustiq/). Merge [`examples/cordis.patch.yml`](dsh-tool-rustiq/examples/cordis.patch.yml) into `~/.dsh/profiles/web/cordis.patch.yml`, set the absolute plugin path, put `rustiq` on PATH, restart `dsh web`. Tools: `rustiq_open` / `rustiq_send` / `rustiq_read` / `rustiq_close`.

## Keybindings

### Baseline picker

| Key | Action |
|-----|--------|
| `↑/↓` or `j/k` | Navigate commits |
| `Enter` | Select baseline |
| `q` / `Esc` | Quit |

### Main view

| Key | Action |
|-----|--------|
| `↑/↓` or `j/k` | Navigate diff lines (cursor for comments) |
| `Tab` / `Shift+Tab` | Next / previous file |
| `Shift+↑/↓` or `Shift+j/k` | Navigate file list |
| `PageUp/PageDown` | Scroll diff |
| `s` | Toggle split ↔ stacked view |
| `T` | Cycle theme |
| `r` | Refresh diff |
| `c` | Add comment on current line |
| `e` | Edit comment on current line |
| `d` | Delete comment on current line |
| `C` | Copy all comments to clipboard |
| `V` | Open comment export (writes `.rustiq/export.txt`; live comments also in `.rustiq/comments.txt`) |
| `q` / `Esc` | Quit |

### Comment input

| Key | Action |
|-----|--------|
| `Enter` | Save comment |
| `Esc` | Cancel |
| `Backspace` | Delete character |

## Themes

| Name | Description |
|------|-------------|
| `default-dark` | Dark background, saturated red/green diff lines |
| `github-light` | White background, pastel pink/green diff lines |

Press `T` to cycle themes at runtime.

## Comments

Each rustiq process is a **new review session**. Comments live in memory for that session and are dual-written on every save:

| File | Role |
|------|------|
| `.rustiq/comments-<session-id>.txt` | Archive for this session (kept after quit) |
| `.rustiq/comments.txt` | Always the **active** session (overwritten on restart / each save) |

Add `.rustiq/` to `.gitignore` (already ignored in this repo).

- Restarting rustiq starts a fresh session (UI does not restore prior comments).
- Prior sessions remain as `comments-<uuid>.txt` archives.
- If the ±10 lines of context around a commented line change (code edited or fixed), the comment is marked **stale** (`[S]` in gutter).
- Comment text is shown inline directly under the commented code line (with multi-line wrapping).
- Press `C` to copy all comments to clipboard in diff format with ±10 lines of context.
- Press `V` also writes `.rustiq/export.txt` (same export format as `comments.txt`).

## Architecture

Hexagonal (ports & adapters):

```text
domain/     — pure data types (DiffFile, Hunk, Comment, …)
ports/      — GitRepository + Highlighter + CommentStore traits
adapters/   — git2, syntect, JSON file implementations
app.rs      — state machine
ui/         — ratatui rendering
main.rs     — wiring + event loop
```
