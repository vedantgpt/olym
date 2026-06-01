# VISTERM

Visual Terminal Tool built for **Code Olympics 2026**.

A Rust terminal application that transforms images, video, and classic gameplay into rich ASCII/Braille visual experiences — all running inside your terminal.

---

## Demo

<video src="https://raw.githubusercontent.com/vedantgpt/olym/main/demo.mp4" controls width="100%"></video>

---

## Challenge Constraints

| Dimension | Constraint |
|-----------|------------|
| **D1 — Core Constraint** | **Simple-State Creator**: Program has exactly 2–3 modes or states |
| **D2 — Line Budget** | **Enterprise Creator**: 650 lines maximum |
| **D3 — Project Domain** | **Visual Creation**: ASCII art, charts, graphics, terminal UIs |
| **D4 — Language** | **Rust**: Ownership, lifetimes, and a compiler that keeps score |

---

## What It Does

VISTERM operates in **three states** (constraint-compliant, 2–3 modes):

1. **Image → ASCII Art**  
   Convert any image into high-resolution Braille or classic ASCII art. Supports true-colour RGB output or grayscale. Adjustable brightness, contrast, and resolution (120–600 columns). Save results as plain `.txt` or ANSI-coloured `.ans` files.

2. **Snake Game**  
   A full TUI snake game with WASD / arrow-key controls, per-level speed scaling, high-score tracking, pause/resume, and polished game-over visuals. Self-contained XorShift RNG — no external `rand` crate needed.

3. **ASCII Video Player**  
   Stream any video through `ffmpeg`, render frames in parallel to Braille art at 60 FPS, and play back with synced audio via `ffplay`. Delta-frame caching keeps playback smooth, with keyboard controls for pause/resume and quit.

---

## Tech Stack

- **Language**: Rust (Edition 2021)
- **Terminal UI**: `crossterm` 0.27 (cross-platform, no ncurses dependency)
- **Image I/O**: `image` 0.24
- **Parallel Processing**: `rayon` 1 (frame-preparation threading)
- **External Tools**: `ffmpeg` + `ffplay` + `ffprobe` (for State 3 video pipeline; auto-detected in common Windows install paths)

---

## Installation & Setup

### 1. Install Rust

If you don't have Rust installed, get it via [rustup](https://rustup.rs/):

**Windows (PowerShell)**
```powershell
Invoke-RestMethod https://sh.rustup.rs | Invoke-Expression
# or download from https://rustup.rs directly
```

**macOS / Linux**
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

After installation, open a new terminal and verify:
```bash
rustc --version
cargo --version
```

### 2. Clone or Download the Project

```bash
git clone https://github.com/vedantgpt/olym.git
cd olym
```

Or download the ZIP and extract it.

### 3. Install Rust Dependencies

All Rust crates are declared in `Cargo.toml` and fetched automatically on first build. No manual crate installation is needed.

```bash
cargo build --release
```

This will download and compile:
- `image` 0.24 — image decoding/encoding
- `crossterm` 0.27 — terminal control and input
- `rayon` 1 — data-parallelism for frame processing

Wait for the build to finish. The binary will appear at:
- **Windows**: `target\release\visterm.exe`
- **macOS / Linux**: `target/release/visterm`

### 4. Install ffmpeg (Required Only for State 3 — ASCII Video Player)

State 3 needs `ffmpeg`, `ffprobe`, and `ffplay` in your system PATH. VISTERM auto-detects them in these common Windows locations if they are not on PATH:
- `C:\ffmpeg\bin`
- `C:\Program Files\ffmpeg\bin`
- `C:\ProgramData\chocolatey\bin`
- WinGet shim directory (`%USERPROFILE%\AppData\Local\Microsoft\WinGet\Links`)

**Install via WinGet (Recommended on Windows)**
```powershell
winget install Gyan.FFmpeg
```

**Install via Chocolatey**
```powershell
choco install ffmpeg
```

**Install via Scoop**
```powershell
scoop install ffmpeg
```

**macOS**
```bash
brew install ffmpeg
```

**Linux (Debian/Ubuntu)**
```bash
sudo apt update
sudo apt install ffmpeg
```

Verify after install:
```bash
ffmpeg -version
ffprobe -version
ffplay -version
```

---

## Build

```bash
cargo build --release
```

The release profile is optimised (`opt-level = 3`, stripped binaries). A debug build is also possible with `cargo build`, but release mode is strongly recommended for smooth 60 FPS video playback.

---

## Run

```bash
cargo run --release
```

Or run the compiled binary directly:

**Windows**
```powershell
.\target\release\visterm.exe
```

**macOS / Linux**
```bash
./target/release/visterm
```

---

## Using Each State

From the main menu, enter the number for the state you want.

### State 1 — Image → ASCII Art

1. Select `1` from the menu.
2. Enter the **full path** to your image file when prompted.  
   Example: `C:\Users\You\Pictures\photo.png`
3. Choose a **resolution** (1–6):  
   `1` = 120 cols, `2` = 200, `3` = 300, `4` = 400, `5` = 500, `6` = 600.
4. Choose **colour mode**:  
   `1` = RGB true-colour ANSI, `2` = Grayscale.
5. Choose adjustment mode:  
   `1` = Default, `2` = Custom brightness / contrast.  
   If custom, enter brightness (`-50` to `50`) and contrast (`0.5` to `2.0`).
6. The art renders in the terminal.
7. After viewing, choose whether to save:
   - `y` to save, then pick format `1` (plain `.txt`) or `2` (ANSI `.ans`).
   - Confirm or edit the output filename.
8. Press **Enter** to return to the menu.

**Supported image formats** (via the `image` crate): PNG, JPEG, GIF, BMP, TIFF, WebP, and more.

### State 2 — Snake Game

1. Select `2` from the menu.
2. Press **Enter** to start.
3. **Controls during gameplay**:
   - `↑` `↓` `←` `→` or `W` `A` `S` `D` — move
   - `P` — pause / resume
   - `Q` or `Esc` — quit back to menu
4. Eat the apple (`🍏`) to grow and increase your score.
5. Speed increases every 5 points (levels 1–10).
6. Colliding with walls or yourself ends the round.
7. On **Game Over**, press `R` or **Enter** to replay, or any other key to return to the menu.

**Tips**
- The game grid automatically sizes itself to your terminal window.
- Each logical cell is drawn as 2 terminal characters wide to preserve a correct aspect ratio.
- High score persists across replays within the same session.

### State 3 — ASCII Video Player

1. Select `3` from the menu.
2. Enter the **full path** to your video file when prompted.  
   Example: `C:\Users\You\Videos\clip.mp4`
3. Choose a **resolution** (1–6) for the Braille output.  
   Lower resolutions render faster and play more smoothly.
4. Choose **colour mode** (`1` = RGB, `2` = Grayscale).  
   RGB at 400+ columns is heavy; grayscale is recommended for higher resolutions.
5. VISTERM launches **Phase 1**: `ffmpeg` streams raw RGB24 frames.  
   A progress bar shows frames read.
6. **Phase 2**: frames are converted to Braille in parallel using `rayon`, then delta-encoded for efficient playback.
7. **Playback starts automatically** at 60 FPS with audio synced via `ffplay`.
8. **Controls during playback**:
   - `Space` — pause / resume (audio pauses/resumes with video)
   - `Q` or `Esc` — stop and return to menu
9. Press **Enter** after playback to return to the menu.

**Video requirements**
- Any format `ffmpeg` can decode (MP4, AVI, MKV, MOV, WebM, etc.).
- `ffmpeg`, `ffprobe`, and `ffplay` must be installed and discoverable (see Installation step 4).
- Long or high-resolution source videos will take more time in Phase 1/2 but playback remains at 60 FPS.

---

## Files

| File | Purpose |
|------|---------|
| `src/main.rs` | Single-file application (~530 lines) |
| `Cargo.toml` | Crate manifest (dependencies & build profile) |
| `const.png` | Code Olympics 2026 constraint card (reference) |

---

## Why These Decisions?

- **Single-file architecture** keeps the project under the 650-line Enterprise Creator limit while still packing three fully interactive states.
- **Braille Unicode (U+2800–U+28FF)** doubles visual resolution over traditional ASCII blocks — each Braille character encodes a 2×4 pixel cell.
- **True-colour ANSI escape codes** (`38;2;R;G;B`) let the terminal render full-colour art without any GUI framework.
- **Delta-frame encoding** in the video player avoids redrawing entire screens every frame, which is critical for maintaining 60 FPS in a terminal.
- **No `rand` crate** — XorShift64 is ~5 lines of self-contained code, saving lines and avoiding an extra dependency while still producing quality gameplay RNG.

---

*Built under the 4D Code Olympics 2026 constraint system: Simple-State Creator · Enterprise Creator · Visual Creation · Rust.*
