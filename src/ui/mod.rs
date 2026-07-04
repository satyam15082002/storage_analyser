mod app;
mod drive_picker;
mod keys;
mod theme;
mod tree_view;

use std::io::Stdout;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen, SetTitle};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Padding, Paragraph};
use ratatui::Terminal;

use crate::scan::{self, Engine, ScanEvent};
use app::App;

pub const APP_TITLE: &str = "Storage Analyzer";
pub const APP_BYLINE: &str = "by Satyam Tamrakar";

pub(crate) type Term = Terminal<CrosstermBackend<Stdout>>;

/// Caps content width the way a responsive website does with a max-width container: on
/// wide terminals the UI sits centered with side margins instead of stretching bars and
/// text edge-to-edge; on narrow terminals (below the cap) it just uses the full width, no
/// margins wasted. 140 columns comfortably fits every screen here without feeling stretched
/// on ultrawide monitors, while a normal 80-120 column terminal is unaffected.
const MAX_CONTENT_WIDTH: u16 = 140;

pub(crate) fn content_area(area: Rect) -> Rect {
    if area.width <= MAX_CONTENT_WIDTH {
        return area;
    }
    let margin = (area.width - MAX_CONTENT_WIDTH) / 2;
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(margin),
            Constraint::Length(MAX_CONTENT_WIDTH),
            Constraint::Length(margin),
        ])
        .split(area)[1]
}

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
    execute!(stdout, EnterAlternateScreen, SetTitle(format!("{APP_TITLE} — {APP_BYLINE}")))?;
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
        let mut frame: u64 = 0;
        let outcome = loop {
            terminal.draw(|f| draw_progress(f, progress, frame))?;
            frame += 1;

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
    use ratatui::style::Modifier;
    use ratatui::text::{Line, Span};

    f.render_widget(Block::default().style(Style::default().bg(theme::BG)), f.area());
    let area = content_area(f.area());
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(40), Constraint::Length(9), Constraint::Percentage(40)])
        .split(area);

    let body = Style::default().fg(theme::TEXT);
    let lines = vec![
        Line::from(vec![
            Span::styled(format!("'{}'", path.display()), Style::default().fg(theme::TEXT).add_modifier(Modifier::BOLD)),
            Span::styled(" is a whole NTFS drive, but this process isn't elevated,", body),
        ]),
        Line::styled("so the fast MFT scan engine can't run — it would fall back to a", body),
        Line::styled("much slower full directory walk.", body),
        Line::from(""),
        Line::styled(
            "Relaunch elevated (UAC prompt) for a fast scan instead? (y/N)",
            Style::default().fg(theme::TEXT).add_modifier(Modifier::BOLD),
        ),
    ];
    let p = Paragraph::new(lines).alignment(Alignment::Center).block(
        Block::default()
            .style(Style::default().bg(theme::BG))
            .borders(Borders::ALL)
            .border_type(theme::PANEL_BORDER)
            .border_style(Style::default().fg(theme::ACCENT))
            .title(" Elevation recommended ")
            .title_style(Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD))
            .title_alignment(Alignment::Center)
            .padding(Padding::uniform(1)),
    );
    f.render_widget(p, layout[1]);
}

fn draw_progress(f: &mut ratatui::Frame, count: u64, frame: u64) {
    use ratatui::style::Modifier;
    use ratatui::text::{Line, Span};

    f.render_widget(Block::default().style(Style::default().bg(theme::BG)), f.area());
    let area = content_area(f.area());
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(45), Constraint::Length(8), Constraint::Percentage(45)])
        .split(area);

    let lines = vec![
        Line::from(vec![
            Span::styled("Scanning… ", Style::default().fg(theme::TEXT)),
            Span::styled(format!("{count}"), Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD)),
            Span::styled(" entries processed", Style::default().fg(theme::TEXT)),
        ]),
        Line::from(indeterminate_bar(30, frame)),
        Line::from(""),
        Line::styled("(press q to cancel)", Style::default().fg(theme::TEXT)),
    ];
    let p = Paragraph::new(lines).alignment(Alignment::Center).block(
        Block::default()
            .style(Style::default().bg(theme::BG))
            .borders(Borders::ALL)
            .border_type(theme::PANEL_BORDER)
            .border_style(Style::default().fg(theme::ACCENT))
            .title(format!(" {APP_TITLE} "))
            .title_style(Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD))
            .title_alignment(Alignment::Center)
            .padding(Padding::uniform(1)),
    );
    f.render_widget(p, layout[1]);
}

/// An indeterminate progress bar (no known total to measure against, since the walker
/// engine doesn't know the file count up front): a gradient "ruler" — green fading through
/// amber into red across its width — with a bright head sweeping back and forth over it,
/// same idea as a classic KITT/Cylon scanner. Purely decorative (it doesn't mean "50% done"),
/// but it reads as "still actively working" far better than a static line of text does.
/// Deliberately doesn't use `Modifier::DIM` for the trailing (non-head) cells — on some
/// terminal color profiles DIM renders as effectively invisible rather than a subtle fade,
/// so the "dimmer" look here comes entirely from the gradient's own darker colors.
fn indeterminate_bar(width: usize, frame: u64) -> Vec<ratatui::text::Span<'static>> {
    use ratatui::style::Modifier;
    use ratatui::text::Span;

    if width < 2 {
        return Vec::new();
    }
    let period = (width as u64 - 1) * 2;
    let pos_in_period = frame % period.max(1);
    let head = if pos_in_period < width as u64 { pos_in_period } else { period - pos_in_period };

    (0..width)
        .map(|i| {
            let pos = i as f64 / (width - 1) as f64;
            let color = theme::gradient(pos);
            if i as u64 == head {
                Span::styled("█", Style::default().fg(theme::TEXT).bg(color).add_modifier(Modifier::BOLD))
            } else {
                Span::styled("█", Style::default().fg(color))
            }
        })
        .collect()
}
