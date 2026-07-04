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
                    // Wraps around at both ends instead of clamping and getting stuck.
                    KeyCode::Up | KeyCode::Char('k') => {
                        selected = (selected as i64 - 1).rem_euclid(drives.len() as i64) as usize;
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        selected = (selected + 1) % drives.len();
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

// Same column convention as the contents list (tree_view): name left, bar + % + size right,
// single line per row — one consistent visual language across both screens.
const LETTER_COL: usize = 6; // "C:\  "
const LABEL_COL: usize = 20;
const PCT_COL: usize = 8;
const SIZE_COL: usize = 40; // "267.38 GB / 358.20 GB (90.82 GB free)"
const MIN_BAR_WIDTH: usize = 8;
const MAX_BAR_WIDTH: usize = 40;

/// Regular-weight text: the one text color, no bold.
fn plain() -> Style {
    Style::default().fg(theme::TEXT)
}

/// Bold text: same color as `plain()`, just heavier — importance is signaled by weight, not
/// by shading through a gray scale (see `theme` module docs).
fn bold() -> Style {
    Style::default().fg(theme::TEXT).add_modifier(Modifier::BOLD)
}

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
        .style(plain())
        .alignment(Alignment::Center)
        .block(themed_block(format!(" {}  ·  {} ", super::APP_TITLE, super::APP_BYLINE)));
    f.render_widget(header, chunks[0]);

    let inner_width = chunks[1].width.saturating_sub(4) as usize;
    let bar_width = inner_width
        .saturating_sub(LETTER_COL + LABEL_COL + PCT_COL + SIZE_COL)
        .clamp(MIN_BAR_WIDTH, MAX_BAR_WIDTH);

    let items: Vec<ListItem> = drives
        .iter()
        .map(|d| {
            let frac = d.used_fraction().clamp(0.0, 1.0);
            let filled = (frac * bar_width as f64).round() as usize;

            let mut spans = vec![
                Span::styled(format!("{}:\\  ", d.letter), Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{:<width$}", truncate(&d.label, LABEL_COL - 1), width = LABEL_COL), plain()),
            ];
            spans.extend(theme::gradient_bar(filled, bar_width));
            spans.push(Span::styled(format!(" {:>5.1}% ", frac * 100.0), plain()));
            spans.push(Span::styled(
                format!(
                    "{:>width$}",
                    format!(
                        "{} / {} ({} free)",
                        format_size(d.used_bytes(), DECIMAL),
                        format_size(d.total_bytes, DECIMAL),
                        format_size(d.free_bytes, DECIMAL)
                    ),
                    width = SIZE_COL
                ),
                bold(),
            ));
            // A blank spacer line under each row so bars/text don't visually collide between
            // adjacent drives — a little vertical breathing room per entry.
            ListItem::new(vec![Line::from(spans), Line::from("")])
        })
        .collect();

    let mut state = ListState::default();
    state.select(Some(selected));

    let list = List::new(items)
        .style(Style::default().bg(theme::BG))
        .block(themed_block(" drives ".to_string()))
        // No `.fg(...)` here: `List` patches this style onto already-rendered cells, and any
        // fg set here would stomp the bar's own size-color, per-span, on the selected row.
        .highlight_style(Style::default().bg(theme::BG_SELECTED));
    f.render_stateful_widget(list, chunks[1], &mut state);

    let footer = Paragraph::new("↑/↓ select  Enter: analyse  q: quit")
        .style(plain())
        .alignment(Alignment::Center);
    f.render_widget(footer, chunks[2]);
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max.saturating_sub(1)).collect();
        t.push('…');
        t
    }
}
