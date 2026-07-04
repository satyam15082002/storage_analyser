mod app;
mod drive_picker;
mod keys;
mod tree_view;

use std::io::Stdout;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Terminal;

use crate::scan::{self, Engine, ScanEvent};
use app::App;

pub(crate) type Term = Terminal<CrosstermBackend<Stdout>>;

/// Runs the interactive TUI. If `path` is `None`, shows a drive picker first (the user
/// didn't specify what to analyse on the command line).
pub fn run(path: Option<PathBuf>, engine: Engine) -> Result<()> {
    let mut terminal = init_terminal()?;
    let result = run_inner(&mut terminal, path, engine);
    restore_terminal(&mut terminal)?;
    result
}

fn init_terminal() -> Result<Term> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    Ok(Terminal::new(CrosstermBackend::new(stdout))?)
}

fn restore_terminal(terminal: &mut Term) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn run_inner(terminal: &mut Term, path: Option<PathBuf>, engine: Engine) -> Result<()> {
    // `next_path` is the CLI-supplied path the first time through; every subsequent loop
    // (the user pressed 'b'/Backspace-at-root to go back) always re-shows the picker.
    let mut next_path = path;

    loop {
        let target = match next_path.take() {
            Some(p) => p,
            None => match drive_picker::pick(terminal)? {
                Some(drive) => drive.path(),
                None => return Ok(()), // user cancelled the picker — quit entirely
            },
        };

        if !offer_elevation_if_needed(terminal, &target, engine)? {
            return Ok(()); // relaunching elevated; this process is done
        }

        let (tx, rx) = mpsc::channel::<ScanEvent>();
        scan::spawn(target, engine, tx);

        let mut progress: u64 = 0;
        let outcome = loop {
            terminal.draw(|f| draw_progress(f, progress))?;

            if event::poll(Duration::from_millis(80))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press && matches!(key.code, KeyCode::Char('q') | KeyCode::Esc) {
                        return Ok(());
                    }
                }
            }

            match rx.try_recv() {
                Ok(ScanEvent::Progress(n)) => progress = n,
                Ok(ScanEvent::Done(result)) => break result,
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    return Err(anyhow::anyhow!("scan thread ended unexpectedly"))
                }
            }
        };

        let outcome = outcome?;
        let mut app = App::new(outcome.arena, outcome.engine_used);

        while !app.should_quit {
            terminal.draw(|f| tree_view::draw(f, &app))?;

            if event::poll(Duration::from_millis(200))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        keys::handle_key(&mut app, key);
                    }
                }
            }
        }

        if !app.want_drive_picker {
            return Ok(());
        }
        // else: loop back around and show the drive picker again
    }
}

/// If `path` is a whole NTFS drive and this process isn't elevated, the fast MFT engine
/// can't run (volume handles are admin-only) and the scan would silently fall back to the
/// much slower walker. Offers to relaunch elevated instead. Returns `false` if a relaunch
/// was kicked off (caller should stop — this process is done), `true` to keep going here.
fn offer_elevation_if_needed(terminal: &mut Term, path: &std::path::Path, engine: Engine) -> Result<bool> {
    if engine == Engine::Walk || crate::privileges::is_elevated() {
        return Ok(true);
    }
    if !scan::is_drive_root(path) || !scan::is_ntfs(path) {
        return Ok(true);
    }

    loop {
        terminal.draw(|f| draw_elevation_prompt(f, path))?;

        if event::poll(Duration::from_millis(150))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('y') | KeyCode::Char('Y') => {
                        return match crate::privileges::relaunch_elevated() {
                            Ok(()) => Ok(false),
                            Err(_) => Ok(true), // couldn't relaunch (e.g. UAC declined) — continue here
                        };
                    }
                    KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc | KeyCode::Enter => return Ok(true),
                    _ => {}
                }
            }
        }
    }
}

fn draw_elevation_prompt(f: &mut ratatui::Frame, path: &std::path::Path) {
    let area = f.area();
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(40), Constraint::Length(6), Constraint::Percentage(40)])
        .split(area);

    let text = format!(
        "'{}' is a whole NTFS drive, but this process isn't elevated,\nso the fast MFT scan engine can't run — it would fall back to a\nmuch slower full directory walk.\n\nRelaunch elevated (UAC prompt) for a fast scan instead? (y/N)",
        path.display()
    );
    let p = Paragraph::new(text)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).title(" Elevation recommended "));
    f.render_widget(p, layout[1]);
}

fn draw_progress(f: &mut ratatui::Frame, count: u64) {
    let area = f.area();
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(45), Constraint::Length(3), Constraint::Percentage(45)])
        .split(area);

    let text = format!("Scanning… {} entries processed\n\n(press q to cancel)", count);
    let p = Paragraph::new(text)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).title(" Storage Analyser "));
    f.render_widget(p, layout[1]);
}
