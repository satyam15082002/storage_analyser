use humansize::{format_size, DECIMAL};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Padding, Paragraph, Wrap};
use ratatui::Frame;

use super::app::{App, Mode, ViewWidth};
use super::theme;
use crate::model::NodeId;

/// Fixed-width columns in the contents list: a leading type icon, a right-aligned size
/// field, and " 100.0% ". The name column and the bar get whatever's left of the terminal
/// width, so wide terminals aren't left with dead space on the right.
const ICON_COL: usize = 3;
const PCT_COL: usize = 9;
const SIZE_COL: usize = 12;
const MIN_NAME_COL: usize = 10;
const MIN_BAR_WIDTH: usize = 8;
const MAX_BAR_WIDTH: usize = 40;

pub fn draw(f: &mut Frame, app: &App) {
    // Paint the full frame with our own background so the look is consistent regardless of
    // the terminal profile's default colors, then lay content out inside it.
    f.render_widget(Block::default().style(Style::default().bg(theme::BG)), f.area());

    let area = match app.view_width {
        ViewWidth::Compact => super::content_area(f.area()),
        ViewWidth::Wide => f.area(),
    };
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

/// Regular-weight text: the one text color, no bold. Use for anything that isn't the single
/// most important thing on its line.
fn plain() -> Style {
    Style::default().fg(theme::TEXT)
}

/// Bold text: same color as `plain()`, just heavier — this is how importance is signaled
/// here, not by shading through a gray scale (see `theme` module docs).
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

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let node = &app.arena.nodes[app.current];
    let path = app.breadcrumb();
    let sep = Span::styled(" │ ", plain());
    let title = Line::from(vec![
        Span::styled(format!("{}", path.display()), bold()),
        sep.clone(),
        Span::styled(format_size(node.size, DECIMAL), Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD)),
        Span::styled(" total", plain()),
        sep.clone(),
        Span::styled(format!("{}", node.file_count), plain()),
        Span::styled(" items", plain()),
        sep,
        Span::styled("engine ", plain()),
        Span::styled(app.engine_used, bold()),
    ]);
    let header = Paragraph::new(title)
        .alignment(Alignment::Center)
        .block(themed_block(format!(" {}  ·  {} ", super::APP_TITLE, super::APP_BYLINE)));
    f.render_widget(header, area);
}

fn draw_list(f: &mut Frame, app: &App, area: Rect) {
    let kids = app.visible_children();
    let parent_size = app.arena.nodes[app.current].size.max(1);

    // Inner content width: 1 col border + 1 col padding on each side.
    let inner_width = area.width.saturating_sub(4) as usize;
    let bar_width = ((inner_width as f64 * 0.28) as usize).clamp(MIN_BAR_WIDTH, MAX_BAR_WIDTH);
    let name_width = inner_width
        .saturating_sub(ICON_COL + bar_width + PCT_COL + SIZE_COL)
        .max(MIN_NAME_COL);

    let items: Vec<ListItem> = kids
        .iter()
        .map(|&id| {
            let n = &app.arena.nodes[id];
            let frac = (n.size as f64 / parent_size as f64).clamp(0.0, 1.0);
            let filled = (frac * bar_width as f64).round() as usize;

            let icon = if n.is_reparse_point {
                theme::ICON_REPARSE
            } else if n.is_dir {
                theme::ICON_DIR
            } else {
                theme::ICON_FILE
            };

            let mut spans = vec![
                Span::raw(format!("{icon} ")),
                Span::styled(format!("{:<width$}", truncate(&n.name, name_width), width = name_width), plain()),
            ];
            spans.extend(theme::gradient_bar(filled, bar_width));
            spans.push(Span::styled(format!(" {:>6.1}% ", frac * 100.0), plain()));
            spans.push(Span::styled(format!("{:>width$}", format_size(n.size, DECIMAL), width = SIZE_COL), bold()));
            // A blank spacer line under each row so bars/text don't visually collide between
            // adjacent items — a little vertical breathing room per entry.
            ListItem::new(vec![Line::from(spans), Line::from("")])
        })
        .collect();

    let mut state = ListState::default();
    if !kids.is_empty() {
        state.select(Some(app.selected));
    }

    let list = List::new(items)
        .style(Style::default().bg(theme::BG))
        .block(themed_block(" contents (Enter: open, Backspace: up) ".to_string()))
        // No `.fg(...)` here: `List` patches this style onto already-rendered cells, and any
        // fg set here would stomp the bar's own size-color, per-span, on the selected row.
        .highlight_style(Style::default().bg(theme::BG_SELECTED).add_modifier(Modifier::BOLD));

    f.render_stateful_widget(list, area, &mut state);
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let text = match app.mode {
        Mode::Filtering => format!("Filter: {}_", app.filter),
        Mode::ConfirmDelete => "Delete selected item to Recycle Bin? (y/n)".to_string(),
        Mode::Info(_) => "press any key to close".to_string(),
        Mode::Browsing => app.status.clone().unwrap_or_else(|| {
            format!(
                "↑/↓ move  →/Enter open  ←/Backspace up  b: drives  i: info  o: explorer  s: sort ({})  v: view ({})  /: filter  e: export  d: delete  q: quit",
                app.sort.label(),
                app.view_width.label(),
            )
        }),
    };
    let style = if app.status.is_some() && matches!(app.mode, Mode::Browsing) {
        Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD)
    } else {
        plain()
    };
    let footer = Paragraph::new(text).style(style).alignment(Alignment::Center);
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

    let label = plain();
    let value = bold();
    let mut lines = vec![
        Line::from(vec![Span::styled("Name      ", label), Span::styled(n.name.clone(), value)]),
        Line::from(vec![Span::styled("Path      ", label), Span::styled(path.display().to_string(), value)]),
        Line::from(vec![Span::styled("Type      ", label), Span::styled(kind, value)]),
        Line::from(vec![
            Span::styled("Size      ", label),
            Span::styled(format_size(n.size, DECIMAL), value),
            Span::styled(format!("  ({:.1}% of parent)", frac), label),
        ]),
        Line::from(vec![Span::styled("On disk   ", label), Span::styled(format_size(n.allocated_size, DECIMAL), value)]),
    ];
    if n.is_dir {
        lines.push(Line::from(vec![Span::styled("Items     ", label), Span::styled(format!("{}", n.file_count), value)]));
        lines.push(Line::from(vec![
            Span::styled("Children  ", label),
            Span::styled(format!("{}", n.children.len()), value),
        ]));
    }

    let popup = centered_rect(70, 50, area);
    f.render_widget(Clear, popup);
    let block = themed_block(" Details (any key to close) ".to_string()).padding(Padding::uniform(1));
    let text = Paragraph::new(lines).block(block).wrap(Wrap { trim: false });
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

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max.saturating_sub(1)).collect();
        t.push('…');
        t
    }
}
