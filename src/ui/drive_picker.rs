use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use humansize::{format_size, DECIMAL};
use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Padding, Paragraph};

use crate::drives::{list_drives, DriveInfo};

use super::theme;
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
const ICON_COL: usize = 3;
const LETTER_COL: usize = 6; // "C:\  "
const TRAILING_COL: usize = 46; // "  12.3 GB used, 45.6 GB free of 100.0 GB"

fn themed_block(title: String) -> Block<'static> {
    Block::default()
        .style(Style::default().bg(theme::BG))
        .borders(Borders::ALL)
        .border_type(theme::PANEL_BORDER)
        .border_style(Style::default().fg(theme::BORDER))
        .title(title)
        .title_style(Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD))
        .title_alignment(Alignment::Center)
        .padding(Padding::horizontal(1))
}

fn draw(f: &mut ratatui::Frame, drives: &[DriveInfo], selected: usize) {
    f.render_widget(Block::default().style(Style::default().bg(theme::BG)), f.area());

    let area = super::content_area(f.area());
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(3), Constraint::Length(1)])
        .split(area);

    let header = Paragraph::new(" Select a drive to analyse ")
        .style(Style::default().fg(theme::TEXT).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(themed_block(format!(" {}  ·  {} ", super::APP_TITLE, super::APP_BYLINE)));
    f.render_widget(header, chunks[0]);

    let inner_width = chunks[1].width.saturating_sub(4) as usize;
    let space_for_bar = inner_width.saturating_sub(ICON_COL + LETTER_COL + LABEL_COL + TRAILING_COL);
    let bar_width = space_for_bar.clamp(MIN_BAR_WIDTH, MAX_BAR_WIDTH);

    let items: Vec<ListItem> = drives
        .iter()
        .map(|d| {
            let frac = d.used_fraction().clamp(0.0, 1.0);
            let filled = (frac * bar_width as f64).round() as usize;
            let bar: String = "█".repeat(filled) + &"░".repeat(bar_width - filled);
            let color = theme::size_color(frac);

            let line = Line::from(vec![
                Span::raw(format!("{} ", theme::ICON_DRIVE)),
                Span::styled(
                    format!("{}:\\  {:<width$}", d.letter, d.label, width = LABEL_COL),
                    Style::default().fg(theme::TEXT),
                ),
                Span::styled(bar, Style::default().fg(color)),
                Span::styled(
                    format!(
                        "  {} used, {} free of {}",
                        format_size(d.used_bytes(), DECIMAL),
                        format_size(d.free_bytes, DECIMAL),
                        format_size(d.total_bytes, DECIMAL)
                    ),
                    Style::default().fg(theme::SUBTEXT),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let mut state = ListState::default();
    state.select(Some(selected));

    let list = List::new(items)
        .style(Style::default().bg(theme::BG))
        .block(themed_block(" drives ".to_string()))
        // No `.fg(...)` here: `List` patches this style onto already-rendered cells, and any
        // fg set here would stomp the bar's own size-color, per-span, on the selected row.
        .highlight_style(Style::default().bg(theme::BG_SELECTED).add_modifier(Modifier::BOLD));
    f.render_stateful_widget(list, chunks[1], &mut state);

    let footer = Paragraph::new("↑/↓ select  Enter: analyse  q: quit")
        .style(Style::default().fg(theme::SUBTEXT))
        .alignment(Alignment::Center);
    f.render_widget(footer, chunks[2]);
}
