mod adapters;
mod app;
mod domain;
mod ports;
mod theme;
mod ui;

use anyhow::{Context, Result};
use crossterm::{
    cursor::{Hide, Show},
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::fs::File;
use std::io::{self, IsTerminal, Write};
use std::time::Duration;

use adapters::{comments::JsonCommentStore, git::Git2Repository, highlight::SyntectHighlighter};
use app::{App, Screen};
use ports::Highlighter;

// ponytail: when stdout is not a TTY (ttyd/WebSocket), open /dev/tty R/W and
// dup2 it onto stdin so crossterm's ioctl calls land on the real PTY.
// Ceiling: requires /dev/tty to exist; fails cleanly if it doesn't.
enum Tty {
    Stdout,
    DevTty(File),
}

impl Write for Tty {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Tty::Stdout => io::stdout().write(buf),
            Tty::DevTty(f) => f.write(buf),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        match self {
            Tty::Stdout => io::stdout().flush(),
            Tty::DevTty(f) => f.flush(),
        }
    }
}

#[derive(Clone, Copy)]
enum UiMode {
    Alternate,
    Inline,
}

impl UiMode {
    fn from_args() -> Self {
        if std::env::args().any(|a| a == "--inline") {
            Self::Inline
        } else {
            Self::Alternate
        }
    }

    fn enter(self, tty: &mut Tty) -> io::Result<()> {
        match self {
            Self::Inline => execute!(tty, Hide),
            Self::Alternate => execute!(tty, EnterAlternateScreen, EnableMouseCapture),
        }
    }

    fn leave(self, tty: &mut Tty) {
        let _ = disable_raw_mode();
        match self {
            Self::Inline => {
                let _ = execute!(tty, Show);
            }
            Self::Alternate => {
                let _ = execute!(tty, LeaveAlternateScreen, DisableMouseCapture, Show);
            }
        }
    }
}

struct TerminalGuard {
    tty: Tty,
    mode: UiMode,
}

fn open_tty() -> Result<Tty> {
    if io::stdout().is_terminal() {
        return Ok(Tty::Stdout);
    }
    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;
        let dev = File::options()
            .read(true)
            .write(true)
            .open("/dev/tty")
            .context("/dev/tty not available — is this a real terminal?")?;
        let ret = unsafe { libc::dup2(dev.as_raw_fd(), libc::STDIN_FILENO) };
        if ret < 0 {
            return Err(io::Error::last_os_error()).context("dup2 /dev/tty -> stdin");
        }
        Ok(Tty::DevTty(dev))
    }
    #[cfg(not(unix))]
    Ok(Tty::Stdout)
}

impl TerminalGuard {
    fn new() -> Result<Self> {
        let mode = UiMode::from_args();
        let mut tty = open_tty()?;
        enable_raw_mode().context("enable_raw_mode")?;
        if let Err(e) = mode.enter(&mut tty) {
            mode.leave(&mut tty);
            return Err(e).context("terminal setup");
        }
        Ok(Self { tty, mode })
    }

    fn backend_tty(&self) -> io::Result<Tty> {
        match &self.tty {
            Tty::Stdout => Ok(Tty::Stdout),
            Tty::DevTty(f) => f.try_clone().map(Tty::DevTty),
        }
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        self.mode.leave(&mut self.tty);
    }
}

fn main() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let git_repo = Git2Repository::open(&cwd)?;
    let comment_store = JsonCommentStore::new(&cwd)?;
    let highlighter = SyntectHighlighter::new();
    let mut app = App::new(Box::new(git_repo), comment_store, cwd.join(".rustiq"))?;

    let guard = TerminalGuard::new()?;
    let backend_tty = guard.backend_tty().context("clone /dev/tty for backend")?;
    let mut terminal = Terminal::new(CrosstermBackend::new(backend_tty))?;
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

        let Event::Key(key) = event::read()? else {
            continue;
        };

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
        KeyCode::Down | KeyCode::Char('j') if app.baseline_cursor + 1 < app.baselines.len() => {
            app.baseline_cursor += 1
        }
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
        (_, KeyCode::Up) | (_, KeyCode::Char('k'))
            if key.modifiers.contains(KeyModifiers::SHIFT) =>
        {
            app.file_up();
        }
        (_, KeyCode::Down) | (_, KeyCode::Char('j'))
            if key.modifiers.contains(KeyModifiers::SHIFT) =>
        {
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
        KeyCode::Backspace => {
            app.comment_input_text.pop();
        }
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
