mod app;
mod keys;
mod tree_view;

use std::io::Stdout;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Terminal;

use crate::scan::{self, Engine, ScanEvent};
use app::App;

type Term = Terminal<CrosstermBackend<Stdout>>;

pub fn run(path: PathBuf, engine: Engine) -> Result<()> {
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

fn run_inner(terminal: &mut Term, path: PathBuf, engine: Engine) -> Result<()> {
    let (tx, rx) = mpsc::channel::<ScanEvent>();
    scan::spawn(path, engine, tx);

    let mut progress: u64 = 0;
    let outcome = loop {
        terminal.draw(|f| draw_progress(f, progress))?;

        if event::poll(Duration::from_millis(80))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press
                    && matches!(key.code, crossterm::event::KeyCode::Char('q') | crossterm::event::KeyCode::Esc)
                {
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

    Ok(())
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
