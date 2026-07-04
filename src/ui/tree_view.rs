use humansize::{format_size, DECIMAL};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use super::app::{App, Mode};

const BAR_WIDTH: usize = 24;

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(3), Constraint::Length(2)])
        .split(area);

    draw_header(f, app, chunks[0]);
    draw_list(f, app, chunks[1]);
    draw_footer(f, app, chunks[2]);
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

    let items: Vec<ListItem> = kids
        .iter()
        .map(|&id| {
            let n = &app.arena.nodes[id];
            let frac = (n.size as f64 / parent_size as f64).clamp(0.0, 1.0);
            let filled = (frac * BAR_WIDTH as f64).round() as usize;
            let bar: String = "█".repeat(filled) + &"░".repeat(BAR_WIDTH - filled);

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
                Span::styled(format!("{:<40}", truncate(&name, 40)), Style::default().fg(Color::White)),
                Span::styled(bar, Style::default().fg(color)),
                Span::raw(format!(" {:>6.1}% ", frac * 100.0)),
                Span::styled(format!("{:>10}", format_size(n.size, DECIMAL)), Style::default().fg(Color::Gray)),
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
        Mode::ConfirmDelete => {
            "Delete selected item to Recycle Bin? (y/n)".to_string()
        }
        Mode::Browsing => app.status.clone().unwrap_or_else(|| {
            format!(
                "↑/↓ move  →/Enter open  ←/Backspace up  s: sort ({})  /: filter  e: export CSV  d: delete  q: quit",
                app.sort.label()
            )
        }),
    };
    let footer = Paragraph::new(text);
    f.render_widget(footer, area);
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
