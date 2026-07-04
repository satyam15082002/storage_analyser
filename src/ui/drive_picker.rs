use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use humansize::{format_size, DECIMAL};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

use crate::drives::{list_drives, DriveInfo};

use super::Term;

/// Interactive drive picker shown when the user didn't pass a path on the command line.
/// Returns `None` if the user cancelled (q/Esc) without picking anything.
pub fn pick(terminal: &mut Term) -> Result<Option<DriveInfo>> {
    let drives = list_drives();
    if drives.is_empty() {
        anyhow::bail!("no drives found to scan");
    }

    let mut selected = 0usize;
    loop {
        terminal.draw(|f| draw(f, &drives, selected))?;

        if event::poll(Duration::from_millis(150))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(None),
                    KeyCode::Up | KeyCode::Char('k') => {
                        selected = selected.saturating_sub(1);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        selected = (selected + 1).min(drives.len() - 1);
                    }
                    KeyCode::Enter => {
                        return Ok(drives.into_iter().nth(selected));
                    }
                    _ => {}
                }
            }
        }
    }
}

const MIN_BAR_WIDTH: usize = 8;
const MAX_BAR_WIDTH: usize = 50;
const LABEL_COL: usize = 22;

fn draw(f: &mut ratatui::Frame, drives: &[DriveInfo], selected: usize) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(3), Constraint::Length(1)])
        .split(area);

    let header = Paragraph::new(" Select a drive to analyse ")
        .block(Block::default().borders(Borders::ALL).title(" Storage Analyser "));
    f.render_widget(header, chunks[0]);

    let inner_width = chunks[1].width.saturating_sub(4) as usize;
    let letter_col = 8; // "C:\  "
    let trailing_col = 46; // "  12.3 GB used, 45.6 GB free of 100.0 GB"
    let space_for_bar = inner_width.saturating_sub(letter_col + LABEL_COL + trailing_col);
    let bar_width = space_for_bar.clamp(MIN_BAR_WIDTH, MAX_BAR_WIDTH);

    let items: Vec<ListItem> = drives
        .iter()
        .map(|d| {
            let frac = d.used_fraction().clamp(0.0, 1.0);
            let filled = (frac * bar_width as f64).round() as usize;
            let bar: String = "█".repeat(filled) + &"░".repeat(bar_width - filled);
            let color = if frac > 0.9 {
                Color::Red
            } else if frac > 0.7 {
                Color::Yellow
            } else {
                Color::Green
            };

            let line = Line::from(vec![
                Span::styled(
                    format!("{}:\\  {:<width$}", d.letter, d.label, width = LABEL_COL),
                    Style::default().fg(Color::White),
                ),
                Span::styled(bar, Style::default().fg(color)),
                Span::raw(format!(
                    "  {} used, {} free of {}",
                    format_size(d.used_bytes(), DECIMAL),
                    format_size(d.free_bytes, DECIMAL),
                    format_size(d.total_bytes, DECIMAL)
                )),
            ]);
            ListItem::new(line)
        })
        .collect();

    let mut state = ListState::default();
    state.select(Some(selected));

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" drives "))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    f.render_stateful_widget(list, chunks[1], &mut state);

    let footer = Paragraph::new("↑/↓ select  Enter: analyse  q: quit");
    f.render_widget(footer, chunks[2]);
}
