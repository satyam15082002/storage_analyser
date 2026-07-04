use std::path::Path;

use anyhow::Result;
use serde::Serialize;

use crate::model::{FsArena, NodeId};

#[derive(Serialize)]
struct Row {
    path: String,
    size: u64,
    allocated_size: u64,
    is_dir: bool,
}

fn collect_rows(arena: &FsArena, id: NodeId, out: &mut Vec<Row>) {
    let node = &arena.nodes[id];
    out.push(Row {
        path: arena.path_of(id).to_string_lossy().to_string(),
        size: node.size,
        allocated_size: node.allocated_size,
        is_dir: node.is_dir,
    });
    for &child in &node.children {
        collect_rows(arena, child, out);
    }
}

pub fn export_csv(arena: &FsArena, root: NodeId, dest: &Path) -> Result<()> {
    let mut rows = Vec::new();
    collect_rows(arena, root, &mut rows);
    let mut writer = csv::Writer::from_path(dest)?;
    for row in rows {
        writer.serialize(row)?;
    }
    writer.flush()?;
    Ok(())
}

pub fn export_json(arena: &FsArena, root: NodeId, dest: &Path) -> Result<()> {
    let mut rows = Vec::new();
    collect_rows(arena, root, &mut rows);
    let file = std::fs::File::create(dest)?;
    serde_json::to_writer_pretty(file, &rows)?;
    Ok(())
}
