use crossterm::event::{KeyCode, KeyEvent};

use super::app::{App, Mode};

/// Handles one key event against `app`, mutating it in place (navigation, sorting,
/// filtering) and performing the few actions with real side effects (export, recycle-bin
/// delete) directly, since this is a small single-user TUI and a fully pure event/action
/// split would just be indirection without benefit here.
pub fn handle_key(app: &mut App, key: KeyEvent) {
    match app.mode {
        Mode::Filtering => handle_filtering(app, key),
        Mode::ConfirmDelete => handle_confirm_delete(app, key),
        Mode::Info(_) => handle_info(app, key),
        Mode::Browsing => handle_browsing(app, key),
    }
}

fn handle_browsing(app: &mut App, key: KeyEvent) {
    app.status = None;
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
        KeyCode::Up | KeyCode::Char('k') => app.move_selection(-1),
        KeyCode::Down | KeyCode::Char('j') => app.move_selection(1),
        KeyCode::PageUp => app.move_selection(-10),
        KeyCode::PageDown => app.move_selection(10),
        KeyCode::Right | KeyCode::Enter | KeyCode::Char('l') => app.descend(),
        KeyCode::Left | KeyCode::Backspace | KeyCode::Char('h') => {
            if app.is_at_root() {
                app.back_to_drive_picker();
            } else {
                app.ascend();
            }
        }
        KeyCode::Char('b') => app.back_to_drive_picker(),
        KeyCode::Char('s') => {
            app.sort = app.sort.next();
            app.selected = 0;
        }
        KeyCode::Char('/') => {
            app.filter.clear();
            app.mode = Mode::Filtering;
        }
        KeyCode::Char('e') => export_current(app),
        KeyCode::Char('d') => {
            if app.selected_node().is_some() {
                app.mode = Mode::ConfirmDelete;
            }
        }
        KeyCode::Char('i') => {
            if let Some(id) = app.selected_node() {
                app.mode = Mode::Info(id);
            }
        }
        KeyCode::Char('v') => app.view_width = app.view_width.toggled(),
        KeyCode::Char('o') => open_selected(app),
        _ => {}
    }
}

fn handle_info(app: &mut App, _key: KeyEvent) {
    // Any key dismisses the info popup.
    app.mode = Mode::Browsing;
}

fn handle_filtering(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.filter.clear();
            app.selected = 0;
            app.mode = Mode::Browsing;
        }
        KeyCode::Enter => {
            app.selected = 0;
            app.mode = Mode::Browsing;
        }
        KeyCode::Backspace => {
            app.filter.pop();
            app.selected = 0;
        }
        KeyCode::Char(c) => {
            app.filter.push(c);
            app.selected = 0;
        }
        _ => {}
    }
}

fn handle_confirm_delete(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            if let Some(id) = app.selected_node() {
                let path = app.arena.path_of(id);
                match crate::recycle::send_to_recycle_bin(&path) {
                    Ok(()) => {
                        app.status = Some(format!("Sent to Recycle Bin: {}", path.display()));
                        app.arena.nodes[app.current].children.retain(|&c| c != id);
                        app.move_selection(0);
                    }
                    Err(e) => app.status = Some(format!("Delete failed: {e}")),
                }
            }
            app.mode = Mode::Browsing;
        }
        _ => app.mode = Mode::Browsing,
    }
}

fn open_selected(app: &mut App) {
    let (path, is_dir) = match app.selected_node() {
        Some(id) => (app.arena.path_of(id), app.arena.nodes[id].is_dir),
        None => (app.arena.path_of(app.current), true), // empty folder: open the folder itself
    };
    match crate::open::open_in_explorer(&path, is_dir) {
        Ok(()) => app.status = Some(format!("Opened in Explorer: {}", path.display())),
        Err(e) => app.status = Some(format!("Open failed: {e}")),
    }
}

fn export_current(app: &mut App) {
    let dest = std::path::PathBuf::from("storage-analyzer-export.csv");
    match crate::export::export_csv(&app.arena, app.current, &dest) {
        Ok(()) => app.status = Some(format!("Exported to {}", dest.display())),
        Err(e) => app.status = Some(format!("Export failed: {e}")),
    }
}
