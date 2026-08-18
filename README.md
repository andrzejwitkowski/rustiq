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
```

On launch, choose a baseline (commit or working tree). Then browse changed files and their diffs.

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
| `V` | Toggle split ↔ stacked view |
| `T` | Cycle theme |
| `r` | Refresh diff |
| `c` | Add comment on current line |
| `e` | Edit comment on current line |
| `d` | Delete comment on current line |
| `C` | Copy all comments to clipboard |
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

Comments are stored in `.rustiq/comments.json` inside the repository (add to `.gitignore`).

- Comments survive restart.
- If the ±10 lines of context around a commented line change (code edited or fixed), the comment is marked **stale** (`[S]` in gutter).
- Press `C` to copy all comments to clipboard in diff format with ±10 lines of context.

## Architecture

Hexagonal (ports & adapters):

```
domain/     — pure data types (DiffFile, Hunk, Comment, …)
ports/      — GitRepository + Highlighter + CommentStore traits
adapters/   — git2, syntect, JSON file implementations
app.rs      — state machine
ui/         — ratatui rendering
main.rs     — wiring + event loop
```
