use std::path::PathBuf;

use crate::model::{FsArena, NodeId};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    SizeDesc,
    NameAsc,
    CountDesc,
}

impl SortMode {
    pub fn label(self) -> &'static str {
        match self {
            SortMode::SizeDesc => "size",
            SortMode::NameAsc => "name",
            SortMode::CountDesc => "count",
        }
    }

    pub fn next(self) -> SortMode {
        match self {
            SortMode::SizeDesc => SortMode::NameAsc,
            SortMode::NameAsc => SortMode::CountDesc,
            SortMode::CountDesc => SortMode::SizeDesc,
        }
    }
}

pub enum Mode {
    Browsing,
    Filtering,
    ConfirmDelete,
    Info(NodeId),
}

pub struct App {
    pub arena: FsArena,
    pub engine_used: &'static str,
    pub current: NodeId,
    pub selected: usize,
    pub sort: SortMode,
    pub filter: String,
    pub mode: Mode,
    pub status: Option<String>,
    pub should_quit: bool,
    /// Set when the user asks to go back to the drive picker instead of quitting outright.
    pub want_drive_picker: bool,
}

impl App {
    pub fn new(arena: FsArena, engine_used: &'static str) -> Self {
        let current = arena.root;
        App {
            arena,
            engine_used,
            current,
            selected: 0,
            sort: SortMode::SizeDesc,
            filter: String::new(),
            mode: Mode::Browsing,
            status: None,
            should_quit: false,
            want_drive_picker: false,
        }
    }

    pub fn is_at_root(&self) -> bool {
        self.arena.nodes[self.current].parent == crate::model::NO_PARENT
    }

    pub fn back_to_drive_picker(&mut self) {
        self.want_drive_picker = true;
        self.should_quit = true;
    }

    /// Children of the current node, sorted per `self.sort` and filtered by `self.filter`
    /// (case-insensitive substring match against the entry name).
    pub fn visible_children(&self) -> Vec<NodeId> {
        let mut kids = self.arena.nodes[self.current].children.clone();

        if !self.filter.is_empty() {
            let needle = self.filter.to_lowercase();
            kids.retain(|&id| self.arena.nodes[id].name.to_lowercase().contains(&needle));
        }

        match self.sort {
            SortMode::SizeDesc => kids.sort_by(|a, b| self.arena.nodes[*b].size.cmp(&self.arena.nodes[*a].size)),
            SortMode::NameAsc => kids.sort_by(|a, b| self.arena.nodes[*a].name.cmp(&self.arena.nodes[*b].name)),
            SortMode::CountDesc => {
                kids.sort_by(|a, b| self.arena.nodes[*b].file_count.cmp(&self.arena.nodes[*a].file_count))
            }
        }
        kids
    }

    pub fn breadcrumb(&self) -> PathBuf {
        self.arena.path_of(self.current)
    }

    pub fn descend(&mut self) {
        let kids = self.visible_children();
        let Some(&target) = kids.get(self.selected) else { return };
        if self.arena.nodes[target].is_dir && !self.arena.nodes[target].children.is_empty() {
            self.current = target;
            self.selected = 0;
            self.filter.clear();
        }
    }

    pub fn ascend(&mut self) {
        let node = &self.arena.nodes[self.current];
        if node.parent == crate::model::NO_PARENT {
            return;
        }
        let prev_child = self.current;
        self.current = node.parent;
        self.filter.clear();
        // Restore selection to where we came from, so ascending then descending is stable.
        let siblings = self.visible_children();
        self.selected = siblings.iter().position(|&id| id == prev_child).unwrap_or(0);
    }

    pub fn move_selection(&mut self, delta: i64) {
        let len = self.visible_children().len();
        if len == 0 {
            self.selected = 0;
            return;
        }
        let new = self.selected as i64 + delta;
        self.selected = new.clamp(0, len as i64 - 1) as usize;
    }

    pub fn selected_node(&self) -> Option<NodeId> {
        self.visible_children().get(self.selected).copied()
    }
}
