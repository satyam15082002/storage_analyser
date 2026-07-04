use humansize::{format_size, DECIMAL};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use super::app::{App, Mode};
use crate::model::NodeId;

/// Fixed-width columns in the contents list: " 100.0% " and a right-aligned size field.
/// The name column and the bar get whatever's left of the terminal width, so wide
/// terminals aren't left with dead space on the right.
const PCT_COL: usize = 9;
const SIZE_COL: usize = 12;
const MIN_NAME_COL: usize = 10;
const MIN_BAR_WIDTH: usize = 8;
const MAX_BAR_WIDTH: usize = 40;

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(3), Constraint::Length(2)])
        .split(area);

    draw_header(f, app, chunks[0]);
    draw_list(f, app, chunks[1]);
    draw_footer(f, app, chunks[2]);

    if let Mode::Info(id) = app.mode {
        draw_info_popup(f, app, id, area);
    }
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let node = &app.arena.nodes[app.current];
    let path = app.breadcrumb();
    let title = format!(
        " {}  —  {} total  —  {} items  —  engine: {} ",
        path.display(),
        format_size(node.size, DECIMAL),
        node.file_count,
        app.engine_used,
    );
    let header = Paragraph::new(title).block(Block::default().borders(Borders::ALL).title(" Storage Analyser "));
    f.render_widget(header, area);
}

fn draw_list(f: &mut Frame, app: &App, area: Rect) {
    let kids = app.visible_children();
    let parent_size = app.arena.nodes[app.current].size.max(1);

    // Inner content width (List renders inside its own border, ~2 cols each side).
    let inner_width = area.width.saturating_sub(4) as usize;
    let bar_width = ((inner_width as f64 * 0.28) as usize).clamp(MIN_BAR_WIDTH, MAX_BAR_WIDTH);
    let name_width = inner_width
        .saturating_sub(bar_width + PCT_COL + SIZE_COL)
        .max(MIN_NAME_COL);

    let items: Vec<ListItem> = kids
        .iter()
        .map(|&id| {
            let n = &app.arena.nodes[id];
            let frac = (n.size as f64 / parent_size as f64).clamp(0.0, 1.0);
            let filled = (frac * bar_width as f64).round() as usize;
            let bar: String = "█".repeat(filled) + &"░".repeat(bar_width - filled);

            let marker = if n.is_reparse_point {
                " ↪"
            } else if n.is_dir {
                "/"
            } else {
                " "
            };
            let name = format!("{}{}", n.name, marker);

            let color = size_color(frac);
            let line = Line::from(vec![
                Span::styled(
                    format!("{:<width$}", truncate(&name, name_width), width = name_width),
                    Style::default().fg(Color::White),
                ),
                Span::styled(bar, Style::default().fg(color)),
                Span::raw(format!(" {:>6.1}% ", frac * 100.0)),
                Span::styled(
                    format!("{:>width$}", format_size(n.size, DECIMAL), width = SIZE_COL),
                    Style::default().fg(Color::Gray),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let mut state = ListState::default();
    if !kids.is_empty() {
        state.select(Some(app.selected));
    }

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" contents (Enter: open, Backspace: up) "))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    f.render_stateful_widget(list, area, &mut state);
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let text = match app.mode {
        Mode::Filtering => format!("Filter: {}_", app.filter),
        Mode::ConfirmDelete => "Delete selected item to Recycle Bin? (y/n)".to_string(),
        Mode::Info(_) => "press any key to close".to_string(),
        Mode::Browsing => app.status.clone().unwrap_or_else(|| {
            format!(
                "↑/↓ move  →/Enter open  ←/Backspace up  b: drives  i: info  s: sort ({})  /: filter  e: export  d: delete  q: quit",
                app.sort.label()
            )
        }),
    };
    let footer = Paragraph::new(text);
    f.render_widget(footer, area);
}

fn draw_info_popup(f: &mut Frame, app: &App, id: NodeId, area: Rect) {
    let n = &app.arena.nodes[id];
    let path = app.arena.path_of(id);
    let parent_size = app.arena.nodes[app.current].size.max(1);
    let frac = (n.size as f64 / parent_size as f64) * 100.0;

    let kind = if n.is_reparse_point {
        "Reparse point (symlink/junction) — not scanned further"
    } else if n.is_dir {
        "Directory"
    } else {
        "File"
    };

    let mut lines = vec![
        format!("Name:      {}", n.name),
        format!("Path:      {}", path.display()),
        format!("Type:      {kind}"),
        format!("Size:      {} ({:.1}% of parent)", format_size(n.size, DECIMAL), frac),
        format!("On disk:   {}", format_size(n.allocated_size, DECIMAL)),
    ];
    if n.is_dir {
        lines.push(format!("Items:     {}", n.file_count));
        lines.push(format!("Children:  {}", n.children.len()));
    }

    let popup = centered_rect(70, 50, area);
    f.render_widget(Clear, popup);
    let block = Block::default().borders(Borders::ALL).title(" Details (any key to close) ");
    let text = Paragraph::new(lines.join("\n")).block(block).wrap(Wrap { trim: false });
    f.render_widget(text, popup);
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

fn size_color(frac: f64) -> Color {
    if frac > 0.5 {
        Color::Red
    } else if frac > 0.2 {
        Color::Yellow
    } else {
        Color::Green
    }
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
