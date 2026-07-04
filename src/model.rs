use std::path::{Path, PathBuf};

/// Index into `FsArena::nodes`. `usize::MAX` is used as a sentinel for "no parent".
pub type NodeId = usize;
pub const NO_PARENT: NodeId = usize::MAX;

#[derive(Debug, Clone)]
pub struct FsNode {
    pub name: String,
    /// Own size for files; aggregated (recursive) size for directories once `finalize` has run.
    pub size: u64,
    /// Own allocated-on-disk size for files; aggregated for directories once finalized.
    pub allocated_size: u64,
    pub is_dir: bool,
    /// True for symlinks/junctions/mount points: recorded but never descended into.
    pub is_reparse_point: bool,
    pub parent: NodeId,
    pub children: Vec<NodeId>,
    /// Aggregated count of files contained (directories only, after finalize).
    pub file_count: u64,
}

impl FsNode {
    fn leaf(name: String, is_dir: bool, is_reparse_point: bool, parent: NodeId) -> Self {
        FsNode {
            name,
            size: 0,
            allocated_size: 0,
            is_dir,
            is_reparse_point,
            parent,
            children: Vec::new(),
            file_count: 0,
        }
    }
}

/// Arena-based filesystem tree. Using indices instead of `Rc<RefCell<_>>` keeps the
/// bottom-up size aggregation a single cache-friendly pass over a flat `Vec`.
#[derive(Debug, Default)]
pub struct FsArena {
    pub nodes: Vec<FsNode>,
    pub root: NodeId,
}

impl FsArena {
    pub fn new(root_name: String) -> Self {
        let root_node = FsNode::leaf(root_name, true, false, NO_PARENT);
        FsArena {
            nodes: vec![root_node],
            root: 0,
        }
    }

    pub fn add_child(
        &mut self,
        parent: NodeId,
        name: String,
        is_dir: bool,
        is_reparse_point: bool,
    ) -> NodeId {
        let id = self.nodes.len();
        self.nodes.push(FsNode::leaf(name, is_dir, is_reparse_point, parent));
        self.nodes[parent].children.push(id);
        id
    }

    pub fn set_file_size(&mut self, id: NodeId, size: u64, allocated_size: u64) {
        let node = &mut self.nodes[id];
        node.size = size;
        node.allocated_size = allocated_size;
    }

    /// Reconstructs the full path of a node by walking parent links.
    pub fn path_of(&self, id: NodeId) -> PathBuf {
        let mut parts = Vec::new();
        let mut cur = id;
        loop {
            let node = &self.nodes[cur];
            parts.push(node.name.as_str());
            if node.parent == NO_PARENT {
                break;
            }
            cur = node.parent;
        }
        parts.reverse();
        let mut path = PathBuf::new();
        for part in parts {
            path.push(part);
        }
        path
    }

    /// Finds the node at `target` by descending from the root, matching one path component
    /// (by name) at a time — used to carry the current browsing position over into a fresh
    /// `FsArena` after a background re-scan replaces this one, since `NodeId`s from the old
    /// arena aren't valid indices into the new one. Falls back to the caller mapping to the
    /// root if a component along the way no longer exists (e.g. a folder got deleted).
    pub fn find_path(&self, target: &Path) -> Option<NodeId> {
        let root_path = self.path_of(self.root);
        let rel = target.strip_prefix(&root_path).ok()?;

        let mut current = self.root;
        for component in rel.components() {
            let std::path::Component::Normal(os_str) = component else { continue };
            let name = os_str.to_string_lossy();
            current = *self.nodes[current].children.iter().find(|&&id| self.nodes[id].name == name)?;
        }
        Some(current)
    }

    /// Aggregates directory sizes/file counts bottom-up. Must be called once after the
    /// tree is fully populated (nodes are always created after their parent, so a single
    /// reverse pass over the arena is a valid post-order traversal).
    pub fn finalize(&mut self) {
        for id in (0..self.nodes.len()).rev() {
            let (is_dir, parent, size, allocated_size, is_file) = {
                let node = &self.nodes[id];
                (node.is_dir, node.parent, node.size, node.allocated_size, !node.is_dir)
            };
            if parent == NO_PARENT {
                continue;
            }
            let parent_node = &mut self.nodes[parent];
            parent_node.size += size;
            parent_node.allocated_size += allocated_size;
            if is_file {
                parent_node.file_count += 1;
            } else if is_dir {
                // Directory file_count already includes its own descendants because
                // children are processed before parents in this reverse pass.
                let child_count = self.nodes[id].file_count;
                self.nodes[parent].file_count += child_count;
            }
        }
    }
}
