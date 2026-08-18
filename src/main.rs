mod adapters;
mod app;
mod domain;
mod ports;
mod theme;
mod ui;

use std::io;
use std::time::Duration;
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use adapters::{comments::JsonCommentStore, git::Git2Repository, highlight::SyntectHighlighter};
use app::{App, Screen};

fn main() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let git_repo = Git2Repository::open(&cwd)?;
    let comment_store = JsonCommentStore::new(&cwd)?;
    let highlighter = SyntectHighlighter::new();

    let mut app = App::new(Box::new(git_repo), comment_store)?;

    // terminal setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_loop(&mut terminal, &mut app, &highlighter);

    // restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    hl: &SyntectHighlighter,
) -> Result<()> {
    loop {
        terminal.draw(|f| ui::render(f, app, hl))?;

        if !event::poll(Duration::from_millis(16))? {
            continue;
        }

        let Event::Key(key) = event::read()? else { continue };

        // clear transient status message on any key
        if app.status_message.is_some() && !matches!(key.code, KeyCode::Char('C')) {
            app.status_message = None;
        }

        match app.screen {
            Screen::BaselinePicker => handle_baseline(app, key)?,
            Screen::Main => handle_main(app, key)?,
            Screen::CommentInput => handle_comment_input(app, key),
        }
    }
}

fn handle_baseline(app: &mut App, key: event::KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => std::process::exit(0),
        KeyCode::Up | KeyCode::Char('k') => {
            if app.baseline_cursor > 0 { app.baseline_cursor -= 1; }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.baseline_cursor + 1 < app.baselines.len() { app.baseline_cursor += 1; }
        }
        KeyCode::Enter => app.select_baseline()?,
        _ => {}
    }
    Ok(())
}

fn handle_main(app: &mut App, key: event::KeyEvent) -> Result<()> {
    match (key.modifiers, key.code) {
        (_, KeyCode::Char('q')) | (_, KeyCode::Esc) => std::process::exit(0),
        (_, KeyCode::Char('v')) | (_, KeyCode::Char('V')) => {
            app.view_mode = app.view_mode.toggle();
        }
        (_, KeyCode::Char('t')) | (_, KeyCode::Char('T')) => {
            app.theme = app.theme.cycle();
        }
        (_, KeyCode::Char('r')) => {
            app.select_baseline()?;
        }
        // file navigation
        (_, KeyCode::Left) | (_, KeyCode::Char('h')) => {}
        (_, KeyCode::Up) | (_, KeyCode::Char('k')) if key.modifiers.contains(KeyModifiers::SHIFT) => {
            app.file_up();
        }
        (_, KeyCode::Down) | (_, KeyCode::Char('j')) if key.modifiers.contains(KeyModifiers::SHIFT) => {
            app.file_down();
        }
        // diff line navigation (plain j/k)
        (_, KeyCode::Up) | (_, KeyCode::Char('k')) => app.diff_line_up(),
        (_, KeyCode::Down) | (_, KeyCode::Char('j')) => app.diff_line_down(),
        // file navigation with Tab
        (_, KeyCode::Tab) => app.file_down(),
        (_, KeyCode::BackTab) => app.file_up(),
        // scroll page
        (_, KeyCode::PageUp) => app.diff_scroll = app.diff_scroll.saturating_sub(10),
        (_, KeyCode::PageDown) => app.diff_scroll = app.diff_scroll.saturating_add(10),
        // comments
        (_, KeyCode::Char('c')) => app.open_comment_input(),
        (_, KeyCode::Char('e')) => app.open_comment_input(),
        (_, KeyCode::Char('d')) => app.delete_comment_on_current_line(),
        (_, KeyCode::Char('C')) => app.export_comments_to_clipboard(),
        _ => {}
    }
    Ok(())
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
