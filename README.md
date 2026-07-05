# Storage Analyzer

_by Satyam Tamrakar_

A high-performance Windows storage analyzer: find out what's taking up space on your disks, fast. Scans an entire NTFS drive in seconds by reading the Master File Table directly (the same technique tools like WizTree use), with a portable recursive-walk fallback for subfolders, non-NTFS volumes, or non-elevated sessions.

[Watch the demo](media/demo.mov)

## Features

- **Two scan engines, chosen automatically**: a direct NTFS `$MFT` reader for whole-drive scans (elevated + NTFS required), and a parallel recursive walker for everything else.
- **Interactive TUI**: drive picker → size-sorted, gradient-colored usage bars for every folder/file, drill-down navigation, live search/filter, sort by size/name/count.
- **Session cache with silent background refresh**: revisit a drive/folder you've already scanned and it loads instantly, while a fresh scan runs quietly behind the scenes and swaps in once it's ready.
- **Headless mode**: `--top N` / `--export csv|json` for scripting, with a text-based drive picker.
- **Open in Explorer** and **send to Recycle Bin** directly from the TUI.
- Statically linked — no runtime DLLs to ship. Runs standalone as a single `.exe`, or install it properly with the bundled installer (Start Menu shortcut, uninstaller, optional PATH entry).

## Building

Requires a Rust toolchain targeting `x86_64-pc-windows-gnu` (this project builds with the GNU host toolchain, not MSVC, since it doesn't depend on Visual Studio Build Tools):

```powershell
cargo +stable-x86_64-pc-windows-gnu build --release
```

The resulting binary is at `target\release\storage-analyzer.exe` — it can be run directly, no installation required.

## Installing (Windows installer)

To build a proper `Setup.exe` (Start Menu shortcut, optional Desktop shortcut, optional PATH entry, uninstaller registered in "Add or Remove Programs"):

1. Build the release binary (see above).
2. Install [Inno Setup](https://jrsoftware.org/isinfo.php) if you don't have it (`winget install JRSoftware.InnoSetup`).
3. Compile the installer script:
   ```powershell
   & "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe" installer\storage-analyzer.iss
   ```
4. The installer is written to `dist\StorageAnalyzerSetup.exe`. Run it to install.

The installer script is at `installer\storage-analyzer.iss` — edit `MyAppVersion` there when bumping the version in `Cargo.toml`.

## Usage

```
storage-analyzer.exe [PATH] [OPTIONS]
```

Run with no arguments to open the interactive drive picker. Pass a path to scan it directly (a full drive root like `D:\` uses the fast MFT engine when possible; any other path uses the walker).

| Option | Description |
|---|---|
| `--engine <auto\|mft\|walk>` | Force a scan engine instead of auto-selecting (default `auto`) |
| `--top <N>` | Print the top N largest entries and exit (no TUI) |
| `--export <file.csv\|file.json>` | Export the full scan and exit (no TUI) |

### Keybindings (TUI)

| Key | Action |
|---|---|
| `↑`/`↓`, `j`/`k` | Move selection |
| `→`/`Enter`/`l` | Open selected folder |
| `←`/`Backspace`/`h` | Go up (or back to the drive picker, at a drive root) |
| `b` | Back to drive picker |
| `r` | Force a fresh re-scan of the current root |
| `s` | Cycle sort mode (size / name / count) |
| `v` | Toggle compact/wide view width |
| `/` | Filter by name |
| `i` | Show details popup for the selected entry |
| `o` | Open selected entry in Windows Explorer |
| `d` | Send selected entry to the Recycle Bin (reversible) |
| `e` | Export the current view to CSV |
| `q`/`Esc` | Quit |

## Notes

- The fast MFT engine needs the process to be **elevated** (volume handles are admin-only) and the target volume to be **NTFS**. The app does **not** require admin to start — subfolder scans, non-NTFS volumes, and the walker engine all work with zero prompts. Only when you target a whole NTFS drive without elevation does it offer a one-time "relaunch elevated (UAC)?" prompt for that run; declining just falls back to the slower walker instead.
- Windows-only. There is no plan to support other platforms.
