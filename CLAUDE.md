# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Storage Analyzer — a Windows-only Rust CLI/TUI that shows what's consuming disk space, using the same NTFS-MFT-direct-read technique as WizTree for near-instant whole-drive scans, falling back to a portable recursive walker everywhere else.

## Build & run

This machine has no MSVC linker installed, so the project builds with the **GNU host toolchain**, not the default `stable-x86_64-pc-windows-msvc`. Always build with:

```
cargo +stable-x86_64-pc-windows-gnu build --release
```

`mingw64\bin` (a WinLibs GCC install under `%LocalAppData%\Microsoft\WinGet\Packages\BrechtSanders.WinLibs.POSIX.UCRT_...\mingw64\bin`) must be on `PATH` for the linker to be found — prepend it in PowerShell before building:

```powershell
$env:Path = "C:\Users\HP\AppData\Local\Microsoft\WinGet\Packages\BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\mingw64\bin;$env:Path"
```

`.cargo/config.toml` sets `target-feature=+crt-static` so the resulting `.exe` is fully static (no MinGW runtime or CRT DLL dependencies) — verify with `objdump -p target/release/storage-analyzer.exe | grep "DLL Name"` after any dependency changes; only core Windows system DLLs and UCRT API-set forwarders should appear.

There is no test suite. Verify changes by running the exe directly (`.\target\release\storage-analyzer.exe [PATH] [--engine auto|mft|walk] [--top N] [--export file.csv]`) and, for TUI changes, launching it and driving it interactively — a background `Start-Process` + `Stop-Process` after a couple seconds only proves it doesn't crash on startup, not that the UI behaves correctly.

Kill stray processes before rebuilding if `cargo build` fails with "Access is denied" removing the `.exe` — a previous run is still holding the file open (`Get-Process storage-analyzer | Stop-Process -Force`, may need to retry once).

## Architecture

### Two scan engines, one output shape

Both engines populate the same arena-based `FsArena` (`src/model.rs`): a `Vec<FsNode>` addressed by `NodeId` (a plain index), not `Rc<RefCell<_>>`. Nodes are always created after their parent, so a node's index is always greater than its parent's — `FsArena::finalize()` exploits this with a single reverse pass to aggregate directory sizes/file counts bottom-up in one cache-friendly sweep, no recursion.

- **`src/mft/`** — the fast path. Reads the NTFS `$MFT` directly: parses the boot sector, locates `$MFT`'s own (possibly fragmented) data runs by reading its base record (record 0), then streams every MFT record in large sequential chunks (`mod.rs`), parsing attributes (`record.rs`) and data runs (`data_runs.rs`) via a sector-aligned raw volume reader (`reader.rs`). Requires an elevated process (volume handles are admin-only) and `SeBackupPrivilege` (`src/privileges.rs`). Only valid for a whole-drive root scan on NTFS.
- **`src/walker.rs`** — the portable fallback. `rayon`-parallelized recursive directory walk. Used for subfolder scans, non-NTFS volumes, or non-elevated processes.
- **`src/scan.rs`** — chooses between them (`Engine::Auto` picks MFT only when `is_drive_root() && is_ntfs() && is_elevated()`, else walker) and runs the chosen one on a background thread (`scan::spawn`), streaming `ScanEvent::Progress`/`Done` back over an mpsc channel. Progress is tracked via a shared `AtomicU64` bumped directly by the (possibly multi-threaded) engine; a separate ticker thread polls it on a fixed cadence, decoupling UI redraw rate from actual scan throughput.

Known correctness trade-off in the MFT engine: it skips `$ATTRIBUTE_LIST`-based attribute overflow (very fragmented files whose `$DATA` header lives in an extension MFT record rather than the base record) — sizes for such files come from whichever `$DATA` instance is found first, which is usually but not always accurate. A previously-fixed bug in the same area: sparse ("hole") runs in `$MFT`'s own data-run list must still advance the record-number counter by the hole's record-equivalent length (`mft/mod.rs`), or every subsequent record's parent/child linkage desyncs.

### TUI structure (`src/ui/`)

- **`app.rs`** — `App` holds the scanned `FsArena`, current navigation position (`NodeId`), sort mode, filter text, and UI mode (`Browsing`/`Filtering`/`ConfirmDelete`/`Info(NodeId)`). Pure state + mutation methods; no rendering.
- **`mod.rs`** — owns the terminal (raw mode / alternate screen), the top-level `run_inner` loop, and the non-interactive screens (drive-elevation prompt, scan-progress screen with an animated indeterminate gradient bar since the walker engine has no known total to show real percentage against).
- **`tree_view.rs`** / **`drive_picker.rs`** — the two main screens; both follow the same row convention (name/letter left, gradient usage bar + % + size right, single line per row, blank spacer line between rows).
- **`theme.rs`** — the single source of colors/icons. Text hierarchy is communicated by `Modifier::BOLD` vs. regular weight on one text color, **not** by a gray scale — `Modifier::DIM` is deliberately never used anywhere in the UI (renders as illegible/near-invisible on several terminal color profiles). The usage-bar gradient (`gradient()`/`gradient_bar()`) is a continuous green→amber→red interpolation, not a 3-band step function, colored per-cell by position along the bar (a "heat-map ruler").
- **`keys.rs`** — all key handling, dispatched by `App::mode`.

Selection highlighting in both list screens uses `List::highlight_style` with **only `.bg(...)` set, never `.fg(...)`** — `ratatui::widgets::List` patches `highlight_style` onto already-rendered cells (`Style::patch`, where any `Some` field always overwrites), so setting `fg` there would stomp the gradient bar's own per-cell color on the selected row. This is documented inline at each `highlight_style(...)` call site; don't "simplify" it by adding `.fg()` back.

### Session-level scan cache (`src/ui/mod.rs`)

`run_inner` keeps a `HashMap<PathBuf, CachedScan>` across drive-picker round-trips (not persisted to disk — cleared on process exit). Re-selecting an already-scanned path from the picker loads the cached `FsArena` instantly *and* kicks off a background re-scan on a fresh channel; the browsing loop drains that channel each tick and, on completion, swaps in the fresh arena and remaps the current browsing position into it via `FsArena::find_path()` (walks the new tree by matching path components, since `NodeId`s are just indices and aren't valid across two different arenas). Pressing `r` forces an immediate full rescan instead of waiting for the background one.

### Windows-only, GNU toolchain

Every Windows API call goes through the `windows` crate (feature-gated in `Cargo.toml`; add new `Win32_*` features there as needed, not ad-hoc `winapi` calls). There is no `#[cfg(not(windows))]` anywhere — the project intentionally targets Windows only, so don't add cross-platform fallback branches.
