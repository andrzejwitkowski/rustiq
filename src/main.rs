mod adapters;
mod app;
mod domain;
mod ports;
mod theme;
mod ui;

use std::fs::File;
use std::io::{self, IsTerminal, Write};
use std::time::Duration;
use anyhow::{Context, Result};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use adapters::{comments::JsonCommentStore, git::Git2Repository, highlight::SyntectHighlighter};
use app::{App, Screen};
use ports::Highlighter;

// ponytail: /dev/tty fallback only; no DummyBackend or text-mode path needed —
// ttyd exposes a real PTY via /dev/tty even when stdout is a WebSocket pipe.
enum Tty {
    Stdout,
    DevTty(File),
}

impl Write for Tty {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self { Tty::Stdout => io::stdout().write(buf), Tty::DevTty(f) => f.write(buf) }
    }
    fn flush(&mut self) -> io::Result<()> {
        match self { Tty::Stdout => io::stdout().flush(), Tty::DevTty(f) => f.flush() }
    }
}

struct TerminalGuard {
    use_dev_tty: bool,
}

impl TerminalGuard {
    fn new(use_dev_tty: bool) -> Result<Self> {
        enable_raw_mode().context("enable_raw_mode")?;
        if use_dev_tty {
            let mut tty = File::options().write(true).open("/dev/tty")?;
            execute!(tty, EnterAlternateScreen, EnableMouseCapture)?;
        } else {
            execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
        }
        Ok(Self { use_dev_tty })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        if self.use_dev_tty {
            if let Ok(mut tty) = File::options().write(true).open("/dev/tty") {
                let _ = execute!(tty, LeaveAlternateScreen, DisableMouseCapture, crossterm::cursor::Show);
            }
        } else {
            let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture, crossterm::cursor::Show);
        }
    }
}

fn main() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let git_repo = Git2Repository::open(&cwd)?;
    let comment_store = JsonCommentStore::new(&cwd)?;
    let highlighter = SyntectHighlighter::new();
    let mut app = App::new(Box::new(git_repo), comment_store, cwd.join(".rustiq"))?;

    let use_dev_tty = !io::stdout().is_terminal();
    let _guard = TerminalGuard::new(use_dev_tty)?;

    let tty: Tty = if use_dev_tty {
        Tty::DevTty(File::options().write(true).open("/dev/tty").context("/dev/tty")?)
    } else {
        Tty::Stdout
    };
    let mut terminal = Terminal::new(CrosstermBackend::new(tty))?;
    run_loop(&mut terminal, &mut app, &highlighter)
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<Tty>>,
    app: &mut App,
    hl: &dyn Highlighter,
) -> Result<()> {
    loop {
        terminal.draw(|f| ui::render(f, app, hl))?;

        if !event::poll(Duration::from_millis(16))? {
            continue;
        }

        let Event::Key(key) = event::read()? else { continue };

        if app.status_message.is_some() && !matches!(key.code, KeyCode::Char('C')) {
            app.status_message = None;
        }

        let should_quit = match app.screen {
            Screen::BaselinePicker => handle_baseline(app, key)?,
            Screen::Main => handle_main(app, key)?,
            Screen::CommentInput => {
                handle_comment_input(app, key);
                false
            }
            Screen::CommentExport => {
                handle_comment_export(app, key);
                false
            }
        };
        if should_quit {
            return Ok(());
        }
    }
}

fn handle_baseline(app: &mut App, key: event::KeyEvent) -> Result<bool> {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => return Ok(true),
        KeyCode::Up | KeyCode::Char('k') if app.baseline_cursor > 0 => app.baseline_cursor -= 1,
        KeyCode::Down | KeyCode::Char('j') if app.baseline_cursor + 1 < app.baselines.len() => app.baseline_cursor += 1,
        KeyCode::Enter => app.select_baseline()?,
        _ => {}
    }
    Ok(false)
}

fn handle_main(app: &mut App, key: event::KeyEvent) -> Result<bool> {
    match (key.modifiers, key.code) {
        (_, KeyCode::Char('q')) | (_, KeyCode::Esc) => return Ok(true),
        (_, KeyCode::Char('s')) | (_, KeyCode::Char('S')) => {
            app.view_mode = app.view_mode.toggle();
        }
        (_, KeyCode::Char('t')) | (_, KeyCode::Char('T')) => {
            app.theme = app.theme.cycle();
        }
        (_, KeyCode::Char('r')) => {
            app.select_baseline()?;
        }
        (_, KeyCode::Left) | (_, KeyCode::Char('h')) => {}
        (_, KeyCode::Up) | (_, KeyCode::Char('k')) if key.modifiers.contains(KeyModifiers::SHIFT) => {
            app.file_up();
        }
        (_, KeyCode::Down) | (_, KeyCode::Char('j')) if key.modifiers.contains(KeyModifiers::SHIFT) => {
            app.file_down();
        }
        (_, KeyCode::Up) | (_, KeyCode::Char('k')) => app.diff_line_up(),
        (_, KeyCode::Down) | (_, KeyCode::Char('j')) => app.diff_line_down(),
        (_, KeyCode::Tab) => app.file_down(),
        (_, KeyCode::BackTab) => app.file_up(),
        (_, KeyCode::PageUp) => app.diff_scroll = app.diff_scroll.saturating_sub(10),
        (_, KeyCode::PageDown) => app.diff_scroll = app.diff_scroll.saturating_add(10),
        (_, KeyCode::Char('c')) => app.open_comment_input(),
        (_, KeyCode::Char('e')) => app.open_comment_input(),
        (_, KeyCode::Char('d')) => app.delete_comment_on_current_line(),
        (_, KeyCode::Char('C')) => app.export_comments_to_clipboard(),
        (_, KeyCode::Char('v')) | (_, KeyCode::Char('V')) => app.open_comment_export(),
        _ => {}
    }
    Ok(false)
}

fn handle_comment_input(app: &mut App, key: event::KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.screen = app::Screen::Main;
            app.comment_input_text.clear();
            app.comment_editing_id = None;
        }
        KeyCode::Enter => app.save_comment(),
        KeyCode::Backspace => { app.comment_input_text.pop(); }
        KeyCode::Char(c) => app.comment_input_text.push(c),
        _ => {}
    }
}

fn handle_comment_export(app: &mut App, key: event::KeyEvent) {
    let max = app.comment_export_line_count.saturating_sub(1);
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => app.screen = app::Screen::Main,
        KeyCode::Up | KeyCode::Char('k') => {
            app.comment_export_scroll = app.comment_export_scroll.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.comment_export_scroll = app.comment_export_scroll.saturating_add(1).min(max);
        }
        KeyCode::PageUp => {
            app.comment_export_scroll = app.comment_export_scroll.saturating_sub(10);
        }
        KeyCode::PageDown => {
            app.comment_export_scroll = app.comment_export_scroll.saturating_add(10).min(max);
        }
        _ => {}
    }
}
