# EncodeTalker

A Rust-based TUI wrapper around FFmpeg for managing AV1 video encoding with a persistent daemon architecture.

## ✨ Features

- **Client-Server Architecture**: Daemon handles encoding in the background, TUI is reconnectable
- **Persistent Encoding Jobs**: Jobs continue even if you close the TUI - reconnect anytime
- **Intelligent Queue Management**: Automatic job scheduling with configurable concurrency
- **Batch File Selection**: Select multiple files with Space, Ctrl+A (select all), Ctrl+D (deselect all)
- **Easy Dependency Management**: Simple script to install FFmpeg, SVT-AV1-PSY, and libaom locally (no sudo required!)
- **Real-Time Progress Tracking**: Live FPS, ETA, and progress bars for each encoding job
- **Flexible Encoding Pipeline**: Support for SVT-AV1 and libaom encoders
- **Wide Format Support**: Handles .mp4, .mkv, .avi, .mov, .webm, .m2ts (BDMV) and more
- **Audio Flexibility**: Encode to Opus or copy original audio streams
- **Subtitle Passthrough**: Automatically preserves all subtitle tracks
- **Smart Configuration**: Per-encode settings or use sensible defaults
- **Cross-Session State**: Persistent queue and history across restarts

## 🚀 Quick Start

### 1. Install System Dependencies

See [System Requirements](#-system-requirements) for your platform.

### 2. Install Encoding Dependencies

**Install all dependencies** (recommended):
```bash
./INSTALL_DEPENDENCIES.sh
```

**Or install selectively**:
```bash
./INSTALL_DEPENDENCIES.sh --ffmpeg    # FFmpeg only
./INSTALL_DEPENDENCIES.sh --svt-av1   # SVT-AV1-PSY only
./INSTALL_DEPENDENCIES.sh --aomenc    # libaom only
```

**Time required**:
- **Linux**: ~60 minutes (compilation from source)
- **Windows**: ~3 minutes (download pre-compiled binaries)

### 3. Build EncodeTalker

```bash
cargo build --release
```

### 4. Launch the TUI

```bash
# Launch the TUI (automatically starts daemon)
./target/release/encodetalker-tui
```

## 📦 System Requirements

To build the encoding dependencies (FFmpeg, SVT-AV1, libaom), install:

### Arch Linux / Manjaro
```bash
sudo pacman -S base-devel cmake git nasm
```

### Ubuntu / Debian
```bash
sudo apt install build-essential cmake git nasm
```

### Fedora
```bash
sudo dnf install @development-tools cmake git nasm
```

### Windows
```powershell
# Install Git and CMake via Chocolatey
choco install git cmake

# Or download manually:
# - Git: https://git-scm.com/download/win
# - CMake: https://cmake.org/download/
```

**Note**:
- **Linux**: All encoding dependencies are compiled locally in `~/.local/share/encodetalker/deps/` - **no sudo required!** 🎉
- **Windows**: Pre-compiled binaries are downloaded to `%LOCALAPPDATA%\encodetalker\deps\` - **no compilation needed!** ⚡

### ⏱️ Installation Time

The `INSTALL_DEPENDENCIES.sh` script handles all dependency installation automatically:

**Linux** (compilation from source):
- **FFmpeg**: ~15-20 minutes
- **SVT-AV1-PSY**: ~10-15 minutes
- **libaom**: ~15-20 minutes
- **Total: ~40-55 minutes** (one-time setup)

**Windows** (pre-compiled binaries download):
- **All dependencies**: ~2-3 minutes (network-dependent)

**Selective installation**:
```bash
# Install only what you need
./INSTALL_DEPENDENCIES.sh --ffmpeg --svt-av1  # Skip libaom
```

**Verification**:
```bash
# Check if dependencies are installed
./CHECK_INSTALLED_DEPENDENCIES.sh
```

## 🎯 Usage

### Launching the TUI

```bash
./target/release/encodetalker-tui
```

The TUI will:
1. Check if the daemon is running
2. Auto-start the daemon if needed
3. Connect via IPC (Unix socket on Linux, Named Pipe on Windows)
4. Display the interactive interface

### Basic Navigation

- **Tab**: Switch between views (Files → Queue → Active → History)
- **↑↓** or **k**/**j**: Navigate lists
- **Enter**: Open directory or configure video file
- **q**: Quit (daemon keeps running in background)

### Batch Encoding (NEW!)

In the **File Browser** view, you can now select multiple files at once:

- **Space**: Toggle selection for current file
- **Ctrl+A**: Select all files in current directory
- **Ctrl+D**: Deselect all files
- **a**: Add all selected files to encoding queue

This makes it easy to encode entire directories or specific sets of files with the same settings.

### Typical Workflow

1. **Launch TUI**: `./target/release/encodetalker-tui`
2. **Navigate to your videos**: Use `↑↓` and `Enter` in the Files tab
3. **Select files**:
   - Single file: Press `a` or `Enter` on a video file
   - Multiple files: Use `Space` to select, then `a` to add batch
4. **Configure encoding** (appears as dialog):
   - Choose encoder (SVT-AV1 or libaom)
   - Configure audio (Opus or Copy)
   - Adjust CRF (quality) and Preset (speed)
   - Confirm with `Enter`
5. **Monitor progress**: Switch to Active tab (`Tab`)
6. **Check results**: View completed jobs in History tab

**Pro tip**: You can close the TUI at any time - the daemon keeps encoding. Relaunch the TUI to reconnect and check progress!

### Manual Daemon Launch (Optional)

```bash
./target/release/encodetalker-daemon
```

**Important**: Dependencies must be installed first via `./INSTALL_DEPENDENCIES.sh`. The daemon will exit with a clear error message if dependencies are missing.

The daemon will:
- Verify all dependencies are installed in `~/.local/share/encodetalker/deps/bin/`
- Listen on Unix socket (Linux): `~/.local/share/encodetalker/daemon.sock`
- Listen on Named Pipe (Windows): `\\.\pipe\encodetalker`
- Load saved state (queue, history)

## ⌨️ Keyboard Shortcuts

### Global
| Key | Action |
|-----|--------|
| `Tab` | Next view |
| `Shift+Tab` | Previous view |
| `q` | Quit TUI (daemon continues) |

### File Browser
| Key | Action |
|-----|--------|
| `↑↓` / `k`/`j` | Navigate files |
| `Enter` | Open directory or configure video |
| `a` | Add selected file(s) to queue |
| `Space` | Toggle selection (batch mode) |
| `Ctrl+A` | Select all files |
| `Ctrl+D` | Deselect all files |
| `r` | Refresh directory |

### Queue View
| Key | Action |
|-----|--------|
| `↑↓` / `k`/`j` | Navigate jobs |
| `c` | Cancel selected job |
| `r` | Refresh |

### Active View
| Key | Action |
|-----|--------|
| `↑↓` / `k`/`j` | Navigate active jobs |
| `c` | Cancel selected job |
| `r` | Refresh |

### History View
| Key | Action |
|-----|--------|
| `↑↓` / `k`/`j` | Navigate history |
| `r` | Retry failed job |
| `Shift+C` | Clear all history |
| `d` | Delete selected history entry |
| `Ctrl+A` | Select all entries |
| `Ctrl+D` | Deselect all entries |
| `Delete` | Delete selected entries |

### Dialogs (Encoding Config, Confirmations)
| Key | Action |
|-----|--------|
| `↑↓` | Navigate fields |
| `←→` | Change value |
| `Enter` | Confirm |
| `ESC` | Cancel |

## ⚙️ Configuration

Configuration file: `~/.config/encodetalker/config.toml`

```toml
[daemon]
max_concurrent_jobs = 1  # Number of simultaneous encoding jobs
socket_path = "~/.local/share/encodetalker/daemon.sock"
log_level = "info"       # Logging verbosity: trace, debug, info, warn, error

[encoding]
default_encoder = "svt-av1"        # Default encoder: "svt-av1" or "aom"
default_audio_mode = "opus"        # Audio mode: "opus" or "copy"
default_audio_bitrate = 128        # Opus bitrate in kbps
output_suffix = ".av1"             # Suffix for output files
precise_frame_count = false        # Enable accurate frame counting (slower probe)

[encoder.svt-av1]
preset = 6     # 0-13, higher = faster encoding
crf = 30       # 0-63, lower = better quality
params = ["--keyint", "240", "--tune", "3"]  # Additional encoder parameters

[encoder.aom]
cpu-used = 4   # 0-8, higher = faster encoding
crf = 30       # 0-63, lower = better quality

[ui]
file_extensions = [".mp4", ".mkv", ".avi", ".mov", ".webm", ".m2ts"]
refresh_interval_ms = 500  # UI refresh rate
```

### Configuration Notes

- **CRF values**: Lower = better quality but larger files. Recommended range: 28-35
- **Presets**: Higher presets encode faster but may reduce compression efficiency
- **Audio modes**:
  - `opus`: Transcode audio to Opus (efficient, lossy)
  - `copy`: Copy original audio streams (lossless, keeps original codec)
- **precise_frame_count**: When `true`, probes every frame for accurate count (slower). When `false`, estimates from headers (faster, may be inaccurate for some formats)

### 🗂️ Customizing Paths (Advanced)

By default, EncodeTalker uses standard XDG directories:
- **Data**: `~/.local/share/encodetalker/` (state, logs, socket)
- **Dependencies**: `~/.local/share/encodetalker/deps/` (~500 MB of compiled binaries)
- **Config**: `~/.config/encodetalker/`

You can customize these paths by adding a `[paths]` section to your `config.toml`:

```toml
[paths]
# Move all data to an external drive
data_dir = "/mnt/external/encodetalker"

# OR move only dependencies to SSD (faster compilation)
deps_dir = "/mnt/ssd/encodetalker-deps"

# Custom socket for multi-user setups
socket_path = "/tmp/encodetalker-$USER.sock"
```

**Key features**:
- ✅ **Tilde expansion**: `~` expands to your home directory
- ✅ **Environment variables**: Use `$HOME`, `$USER`, etc.
- ✅ **Partial customization**: Set only what you need, rest uses defaults
- ✅ **Automatic consistency**: Daemon and TUI always use the same config

**Common use cases**:
1. **Move dependencies to SSD**: Set `deps_dir = "/mnt/ssd/encodetalker-deps"` to speed up compilation
2. **Multi-user systems**: Use `socket_path = "/tmp/encodetalker-$USER.sock"` for per-user daemons
3. **External storage**: Move entire data directory to larger disk with `data_dir = "/mnt/storage/encodetalker"`

**Important**: Both the daemon and TUI read from `~/.config/encodetalker/config.toml`, so they're always synchronized. Restart the daemon after changing paths.

## 📁 Files and Directories

EncodeTalker creates the following directories:

### Data Directory: `~/.local/share/encodetalker/`
- **deps/bin/**: Compiled binaries (ffmpeg, ffprobe, SvtAv1EncApp, aomenc)
- **deps/src/**: Downloaded source code (kept for reference)
- **state.json**: Persisted state (queue, active jobs, history)
- **daemon.sock**: Unix socket for IPC communication
- **daemon.log**: Daemon log file

### Config Directory: `~/.config/encodetalker/`
- **config.toml**: User configuration (created from defaults if missing)

### Output Files
Encoded videos are saved next to the original file with the configured suffix (default: `.av1.mkv`).

Example:
```
/path/to/video.mp4  →  /path/to/video.av1.mkv
```

## 🏗️ Architecture Overview

EncodeTalker uses a client-server architecture for resilient encoding:

### Encoding Pipeline

```
input.mp4
    │
    ├─→ ffmpeg (demux + raw video in yuv4mpegpipe format)
    │       │
    │       ↓ stdout pipe
    │   AV1 Encoder (SVT-AV1-PSY or libaom)
    │       │
    │       ↓ output
    │   video.ivf (raw AV1 video)
    │
    └─→ ffmpeg (extract audio)
            │
            ↓
        audio.opus (or copied stream)

Final step:
    ffmpeg (mux video.ivf + audio + subtitles) → output.mkv
```

**Note**: The entire pipeline uses FFmpeg for muxing (not mkvtoolnix).

### Component Communication

- **Daemon**: Background process managing the encoding queue
- **TUI**: Interactive terminal interface (client)
- **IPC Protocol**: Communication via Unix socket with bincode-serialized messages
- **Event Broadcasting**: Real-time progress updates sent to all connected clients
- **State Persistence**: Queue and history saved to JSON every 10 seconds

### Key Design Principles

1. **Resilience**: Daemon runs independently - clients can disconnect/reconnect freely
2. **Persistence**: All state survives daemon restarts
3. **Efficiency**: Piped data flow (no temporary files between encoding steps)
4. **Transparency**: Real-time stats parsed from FFmpeg output

For detailed technical documentation, see [CLAUDE.md](CLAUDE.md).

## 🔧 Current Limitations

EncodeTalker is actively developed. Current limitations:

- **Manual stream selection not implemented**: All audio/subtitle streams are included by default
- **Single encoder instance per job**: No multi-pass encoding yet
- **No remote API**: Daemon only accessible via local IPC
- **Limited platform support**: Linux and Windows supported, macOS not tested (PRs welcome!)
- **Limited audio options**: Only Opus encoding or copy (no other codecs)
- **No video filters**: Cropping, resizing, denoising not yet implemented
- **No preset system**: Cannot save/load encoding configurations

See [GitHub Issues](https://github.com/yourusername/EncodeTalker/issues) for planned features and known bugs.

## 🤝 Contributing

Contributions are welcome! To get started:

1. Read [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and guidelines
2. Check [CLAUDE.md](CLAUDE.md) for architecture and coding conventions
3. Look for issues labeled `good-first-issue` or `help-wanted`
4. Fork the repo, make your changes, and submit a PR

### Development Quick Start

```bash
# Clone repository
git clone https://github.com/yourusername/EncodeTalker.git
cd EncodeTalker

# Build and test
cargo build
cargo test --all

# Format and lint (required before committing)
cargo fmt --all
cargo clippy --all-targets --all-features

# Run with debug logs
RUST_LOG=debug ./target/release/encodetalker-daemon
```

## 📄 License

Licensed under either of:

- **MIT License** ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
- **Apache License, Version 2.0** ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)

at your option.

## 🙏 Acknowledgments

EncodeTalker builds upon excellent open-source projects:

- **[FFmpeg](https://ffmpeg.org/)**: Universal media framework
- **[SVT-AV1-PSY](https://github.com/gianni-rosato/svt-av1-psy)**: Optimized AV1 encoder
- **[libaom](https://aomedia.googlesource.com/aom/)**: Reference AV1 encoder
- **[Ratatui](https://ratatui.rs/)**: Terminal UI framework
- **[Tokio](https://tokio.rs/)**: Async runtime for Rust

---

**Made with ❤️ by the EncodeTalker Team**

*Questions? Issues? Feature requests? Open an issue on GitHub!*
