// VISTERM — Visual Terminal Tool  |  Code Olympics 2026
// D1: Simple-State Creator (3 states)  D2: Enterprise Creator (≤650 lines)
// D3: Visual Creation (ASCII art · snake TUI · 60 FPS video)  D4: Rust
// Build: cargo run --release   deps: image="0.24" crossterm="0.27" rayon="1"
// State 3 needs ffmpeg + ffprobe in PATH (WinGet / common dirs auto-found)

use std::{
    collections::VecDeque,
    io::{self, BufReader, Read, Write},
    process::{Command, Stdio},
    time::{Duration, Instant},
    path::Path, thread,
};
use rayon::prelude::*;
use image::GenericImageView;
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{poll, read, Event, KeyCode, KeyEvent},
    execute,
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};

// ── True-colour ANSI palette ──────────────────────────────────────────────────
const RST: &str = "\x1b[0m";  const BLD: &str = "\x1b[1m";  const DIM: &str = "\x1b[2m";
const PUR: &str = "\x1b[38;2;176;64;255m";   // brand purple
const CYN: &str = "\x1b[38;2;0;229;255m";    // cyan
const GRN: &str = "\x1b[38;2;0;255;140m";    // neon green
const YLW: &str = "\x1b[38;2;255;210;0m";    // amber yellow
const RED: &str = "\x1b[38;2;255;80;80m";    // error red
const WHT: &str = "\x1b[38;2;215;215;235m";  // soft white
const GRY: &str = "\x1b[38;2;75;75;105m";    // muted border

// 70-level ASCII density ramp (brightness → character)
const CHARS: &[u8] =
    b" .'`^\",;Il!i><~+_-?][}{1)(|\\/tfjrxnuvczXYUJCLQ0OZmwqpdbkhao*#MW&8%B@$";
// Braille dot-bit masks [row 0-3][col 0-1] → Unicode U+2800–U+28FF
const DOT: [[u8; 2]; 4] = [[0x01,0x08],[0x02,0x10],[0x04,0x20],[0x40,0x80]];

// ── Snake direction ───────────────────────────────────────────────────────────
#[derive(Clone, Copy, PartialEq, Eq)]
enum Dir { Up, Down, Left, Right }

// ── Core helpers ──────────────────────────────────────────────────────────────
fn luma(v: u8) -> char { CHARS[(v as usize * (CHARS.len()-1)) / 255] as char }
fn flush()             { io::stdout().flush().ok(); }
fn cls()               { execute!(io::stdout(), Clear(ClearType::All), MoveTo(0,0)).ok(); }
fn wait(m: &str)       { print!("{GRY}{m}{RST}"); flush(); io::stdin().read_exact(&mut [0u8]).ok(); }
fn ask(p: &str) -> String {
    print!("  {CYN}{BLD}›{RST} {WHT}{p}{RST} "); flush();
    let mut s = String::new(); io::stdin().read_line(&mut s).ok(); s.trim().to_string()
}
fn find_bin(name: &str) -> String {
    if Command::new(name).arg("-version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok() {
        return name.to_string();
    }
    let wg = format!("{}\\AppData\\Local\\Microsoft\\WinGet\\Links\\{name}.exe",
        std::env::var("USERPROFILE").unwrap_or_default());
    if Path::new(&wg).exists() { return wg; }
    for d in &["C:\\ffmpeg\\bin","C:\\Program Files\\ffmpeg\\bin","C:\\ProgramData\\chocolatey\\bin"] {
        let p = format!("{d}\\{name}.exe"); if Path::new(&p).exists() { return p; }
    }
    name.to_string()
}
fn pbar(done: u64, tot: u64, eta: f64) {
    let pct = (done * 100 / tot.max(1)).min(100); let f = (pct/5) as usize;
    print!("\r  [{GRN}{}{GRY}{}{RST}] {YLW}{pct:3}%{RST} {done}/{tot}  ETA {CYN}{eta:.0}s{RST}  ",
        "█".repeat(f), "░".repeat(20-f)); flush();
}
fn banner(title: &str, col: &str) {
    let inner = format!("╡ {title} ╞");
    let pad   = "═".repeat(64usize.saturating_sub(inner.chars().count()));
    println!("\n{col}{BLD}  ╔{inner}{pad}╗\n  ╚{:═<64}╝{RST}", "");
}
fn pick_res() -> u32 {
    banner("RESOLUTION", CYN);
    println!("  {GRY}[1]{RST} 120  {GRY}[2]{RST} 200  {GRY}[3]{RST} 300  {GRY}[4]{RST} 400  {GRY}[5]{RST} 500  {GRY}[6]{RST} 600");
    match ask("Resolution (1-6):").as_str() { "1"=>120,"2"=>200,"4"=>400,"5"=>500,"6"=>600,_=>300 }
}
fn pick_color() -> bool {
    banner("COLOR MODE", CYN);
    println!("  {GRY}[1]{RST} {PUR}RGB Color{RST}   {GRY}[2]{RST} {WHT}Grayscale{RST}");
    ask("Mode (1/2):") != "2"
}

// ── Braille renderer: RGB24 buffer → Unicode Braille art (2×4 px per char) ───
fn to_braille(buf: &[u8], bw: u32, bh: u32, use_rgb: bool) -> Vec<Vec<u8>> {
    let (cols, rows) = ((bw/2) as usize, (bh/4) as usize);
    let mut out = Vec::with_capacity(rows);
    for row in 0..rows {
        let mut line = Vec::with_capacity(cols * if use_rgb {25} else {4});
        for col in 0..cols {
            let (mut dots,mut rs,mut gs,mut bs,mut cnt) = (0u8,0u32,0u32,0u32,0u32);
            for dy in 0u32..4 { for dx in 0u32..2 {
                let (px,py) = ((col as u32*2+dx) as usize,(row as u32*4+dy) as usize);
                if px < bw as usize && py < bh as usize {
                    let o = (py * bw as usize + px) * 3;
                    let (r,g,b) = (buf[o],buf[o+1],buf[o+2]);
                    let lm = (0.299*r as f32+0.587*g as f32+0.114*b as f32) as u8;
                    if lm < 128 { dots |= DOT[dy as usize][dx as usize]; }
                    rs+=r as u32; gs+=g as u32; bs+=b as u32; cnt+=1;
                }
            }}
            let ch = char::from_u32(0x2800 + u32::from(dots)).unwrap_or(' ');
            if use_rgb && cnt > 0 {
                let (r,g,b) = (rs/cnt,gs/cnt,bs/cnt);
                line.extend_from_slice(format!("\x1b[38;2;{r};{g};{b}m{ch}\x1b[0m").as_bytes());
            } else {
                let mut tmp=[0u8;4]; line.extend_from_slice(ch.encode_utf8(&mut tmp).as_bytes());
            }
        }
        line.push(b'\n'); out.push(line);
    }
    out
}

// Keyframe + delta-encoded video cache
enum Frame { Key(Vec<Vec<u8>>), Delta(Vec<(usize,Vec<u8>)>) }

// ── Snake helpers ─────────────────────────────────────────────────────────────
fn xorshift(s: &mut u64) -> u64 {
    *s ^= *s << 13; *s ^= *s >> 7; *s ^= *s << 17; *s
}
fn snake_food(body: &VecDeque<(i16,i16)>, s: &mut u64, gw: i16, gh: i16) -> (i16,i16) {
    loop {
        let x=(xorshift(s) % gw as u64) as i16; let y=(xorshift(s) % gh as u64) as i16;
        if !body.contains(&(x,y)) { return (x,y); }
    }
}
fn draw_snake(body: &VecDeque<(i16,i16)>, food: (i16,i16), score: u32, hi: u32,
              gw: i16, gh: i16, _dir: Dir, paused: bool) {
    // iw = terminal chars wide for the game area (2 chars per logical cell fixes aspect ratio)
    let iw  = gw as usize * 2;
    let lv  = score / 5 + 1;
    let hd  = *body.front().unwrap_or(&(0,0));
    let spd_bar = format!("{}{}", "▰".repeat((lv as usize).min(10)), "▱".repeat(10usize.saturating_sub(lv as usize)));
    execute!(io::stdout(), MoveTo(0,0)).ok();
    // Header
    print!("{GRY}  ┌{:─<w$}┐\r\n","",w=iw+2);
    let hdr = format!(" {GRN}{BLD}🐍 VISTERM SNAKE{RST}  \
        {GRY}Score{RST}:{YLW}{BLD}{score:04}{RST}  \
        {GRY}Best{RST}:{CYN}{hi:04}{RST}  \
        {GRY}Lv{RST}:{PUR}{BLD}{lv:02}{RST}  \
        {GRY}Spd{RST}:{GRN}{spd_bar}{RST} ");
    let hdr_vis = 15 + 10 + 4 + 4 + 2 + 2 + 10 + 2;
    let pad = (iw+2).saturating_sub(hdr_vis);
    print!("{GRY}  │{hdr}{:>pad$}{GRY}│\r\n","");
    print!("{GRY}  ├{:─<w$}┤\r\n","",w=iw+2);
    // Grid — each cell = 2 terminal chars wide to match terminal aspect ratio (~2:1 h:w)
    for row in 0..gh {
        print!("{GRY}  │{RST} ");
        for col in 0..gw {
            let pos=(col,row);
            if      pos == hd || body.contains(&pos) { print!("{GRN}██{RST}"); } // uniform snake body
            else if pos == food                       { print!("🍏"); }           // apple emoji (2-wide)
            else                                      { print!("  "); }           // empty
        }
        print!(" {GRY}│\r\n");
    }
    print!("{GRY}  └{:─<w$}┘\r\n","",w=iw+2);
    if paused {
        print!("{YLW}{BLD}    ⏸  PAUSED — P to resume  ·  Q to quit        {RST}\r\n");
    } else {
        print!("{DIM}    ↑↓←→ / WASD  ·  P = pause  ·  Q = quit           {RST}\r\n");
    }
    flush();
}

// ── Main menu ─────────────────────────────────────────────────────────────────
fn show_menu() {
    cls();
    let logo = [
        ("\x1b[38;2;176;64;255m",  "  ██╗   ██╗██╗███████╗████████╗███████╗██████╗ ███╗   ███╗"),
        ("\x1b[38;2;135;85;255m",  "  ██║   ██║██║██╔════╝╚══██╔══╝██╔════╝██╔══██╗████╗ ████║"),
        ("\x1b[38;2;90;130;255m",  "  ██║   ██║██║███████╗   ██║   █████╗  ██████╔╝██╔████╔██║"),
        ("\x1b[38;2;45;175;255m",  "  ╚██╗ ██╔╝██║╚════██║   ██║   ██╔══╝  ██╔══██╗██║╚██╔╝██║"),
        ("\x1b[38;2;0;210;255m",   "   ╚████╔╝ ██║███████║   ██║   ███████╗██║  ██║██║ ╚═╝ ██║"),
        ("\x1b[38;2;0;229;255m",   "    ╚═══╝  ╚═╝╚══════╝   ╚═╝   ╚══════╝╚═╝  ╚═╝╚═╝     ╚═╝"),
    ];
    print!("{BLD}");
    for (c,l) in &logo { println!("{c}{l}"); }
    println!("{RST}{DIM}  Visual Terminal Tool  ·  Code Olympics 2026  ·  Rust  ·  v1.0{RST}");
    let (tc,_) = terminal::size().unwrap_or((72,24));
    println!();
    println!("{GRY}  ╔══════════════════════════════════════════════════════════════════╗");
    println!("{GRY}  ║{CYN}{BLD}                   ✦  SELECT A STATE  ✦                         {GRY}║{RST}");
    println!("{GRY}  ╠══════════════════════════════════════════════════════════════════╣{RST}");
    println!("{GRY}  ║{RST}                                                              {GRY}║{RST}");
    println!("{GRY}  ║{RST}  {PUR}{BLD}[ 1 ]{RST}  {WHT}Image → ASCII Art   {DIM}Braille · RGB · Adjustable      {GRY}║{RST}");
    println!("{GRY}  ║{RST}  {CYN}{BLD}[ 2 ]{RST}  {WHT}Snake Game          {DIM}WASD/Arrows · Levels · Hi-Score  {GRY}║{RST}");
    println!("{GRY}  ║{RST}  {GRN}{BLD}[ 3 ]{RST}  {WHT}ASCII Video Player  {DIM}60 FPS · Braille · Sync Audio   {GRY}║{RST}");
    println!("{GRY}  ║{RST}  {RED}{BLD}[ 0 ]{RST}  {WHT}Exit{RST}                                                  {GRY}║{RST}");
    println!("{GRY}  ║{RST}                                                              {GRY}║{RST}");
    println!("{GRY}  ╚══════════════════════════════════════════════════════════════════╝{RST}");
    println!("{DIM}  Terminal: {tc} cols  ·  3 states  ·  ≤650 lines  ·  Rust{RST}");
    println!();
}

// ═══════════════════════════════════════════════════════════════════════════════
// STATE 1 — Image → ASCII Art
// ═══════════════════════════════════════════════════════════════════════════════
fn state_image() {
    cls(); banner("STATE 1  ─  IMAGE → ASCII ART  🎨", PUR);
    let path = ask("Image path:");
    if !Path::new(&path).is_file() {
        println!("  {RED}✗  Not a valid image file.{RST}"); wait("\n  [Enter]"); return;
    }
    let width   = pick_res();
    let use_rgb = pick_color();
    println!("\n  {GRY}[1]{RST} Default  {GRY}[2]{RST} Custom brightness / contrast");
    let do_edit = ask("Mode (1/2):") == "2";
    let bri: f32 = if do_edit { ask("Brightness (–50..50):").parse().unwrap_or(0.0) } else { 0.0 };
    let con: f32 = if do_edit { ask("Contrast (0.5..2.0):").parse().unwrap_or(1.0) } else { 1.0 };
    println!("\n  {CYN}⏳ Converting…{RST}");
    let t0  = Instant::now();
    let img = match image::open(&path) {
        Ok(i)  => i,
        Err(e) => { println!("  {RED}✗  {e}{RST}"); wait("\n  [Enter]"); return; }
    };
    let (ow,oh) = img.dimensions();
    let height  = ((oh as f32 / ow as f32) * width as f32 * 0.55) as u32;
    let img     = img.resize_exact(width, height, image::imageops::FilterType::Nearest);
    let adj = |v: u8| -> u8 {
        (((v as f32/255.0 - 0.5)*con + 0.5 + bri/100.0).clamp(0.0,1.0)*255.0) as u8
    };
    let mut art = String::with_capacity((width*height*20) as usize);
    for y in 0..height { for x in 0..width {
        let [r,g,b,_] = img.get_pixel(x,y).0;
        let (r,g,b)   = (adj(r),adj(g),adj(b));
        let ch        = luma((0.299*r as f32+0.587*g as f32+0.114*b as f32) as u8);
        if use_rgb { art.push_str(&format!("\x1b[38;2;{r};{g};{b}m{ch}\x1b[0m")); } else { art.push(ch); }
    } art.push('\n'); }
    let elapsed = t0.elapsed().as_secs_f32();
    cls(); print!("{art}");
    println!("\n{GRN}  ✓  {ow}×{oh} → {width}×{height} chars  ·  {}  ·  {elapsed:.2}s{RST}",
        if use_rgb{"RGB"}else{"BW"});
    if ask("Save to file? (y/N):").eq_ignore_ascii_case("y") {
        println!("  {GRY}[1]{RST} Plain .txt   {GRY}[2]{RST} ANSI colour .ans");
        let ansi = ask("Format (1/2):") == "2" && use_rgb;
        let stem = Path::new(&path).file_stem().and_then(|s|s.to_str()).unwrap_or("out");
        let def  = format!("{stem}{}", if ansi{"_color.ans"}else{"_ascii.txt"});
        let dest = { let r = ask(&format!("Output [{def}]:")); if r.is_empty(){def}else{r} };
        let data = if ansi { art.clone() } else {
            art.lines().map(|l|{
                let mut s=String::new(); let mut esc=false;
                for c in l.chars() {
                    if c=='\x1b'{esc=true;continue;}
                    if esc{if c=='m'{esc=false;}continue;}
                    s.push(c);
                } s
            }).collect::<Vec<_>>().join("\n")+"\n"
        };
        match std::fs::write(&dest,&data) {
            Ok(_)  => println!("  {GRN}✓  Saved → {dest}{RST}"),
            Err(e) => println!("  {RED}✗  {e}{RST}"),
        }
    }
    wait("\n  [Enter to continue]");
}

// ═══════════════════════════════════════════════════════════════════════════════
// STATE 2 — Snake Game  (inspired by github.com/BookOwl/snake-rs)
// XorShift RNG — no external rand crate; self-contained in ≤650 lines
// ═══════════════════════════════════════════════════════════════════════════════
fn state_snake() {
    cls(); banner("STATE 2  ─  SNAKE GAME  🐍", CYN);
    println!("  {WHT}Controls:{RST}  {GRN}↑ ↓ ← →{RST} or {GRN}W A S D{RST}  {GRY}·{RST}  {YLW}P{RST} = pause  {YLW}Q{RST} = quit");
    println!("  {DIM}Eat ★ to grow · Speed increases every 5 points · Avoid walls!{RST}");
    wait("\n  [Enter to start]");

    let (tw,th) = terminal::size().unwrap_or((80,28));
    // gw = logical grid columns; each renders as 2 terminal chars → grid fills screen correctly
    let gw      = ((tw as i16 - 6) / 2).min(31).max(10);
    let gh      = (th  as i16 - 8).min(24).max(10);
    // XorShift64 seed — no rand crate needed
    let mut seed = Instant::now().elapsed().as_nanos() as u64 ^ 0xDEAD_CAFE_BEEF_1337;
    let mut hi   = 0u32;

    terminal::enable_raw_mode().ok();
    execute!(io::stdout(), EnterAlternateScreen, Hide).ok();

    'session: loop {
        let (cx,cy) = (gw/2, gh/2);
        let mut body: VecDeque<(i16,i16)> =
            VecDeque::from(vec![(cx,cy),(cx-1,cy),(cx-2,cy)]);
        let mut dir=Dir::Right; let mut ndir=Dir::Right;
        let mut food = snake_food(&body,&mut seed,gw,gh);
        let mut score=0u32; let mut paused=false;
        execute!(io::stdout(), Clear(ClearType::All), MoveTo(0,0)).ok();

        // Inner game loop — breaks with bool: true=user quit, false=snake died
        let quit = 'game: loop {
            let spd = 200u64.saturating_sub(u64::from(score/5)*20).max(55);
            if poll(Duration::from_millis(spd)).unwrap_or(false) {
                if let Ok(Event::Key(KeyEvent{code,..})) = read() { match code {
                    KeyCode::Up    |KeyCode::Char('w')|KeyCode::Char('W')=>{if dir!=Dir::Down  {ndir=Dir::Up;}}
                    KeyCode::Down  |KeyCode::Char('s')|KeyCode::Char('S')=>{if dir!=Dir::Up    {ndir=Dir::Down;}}
                    KeyCode::Left  |KeyCode::Char('a')|KeyCode::Char('A')=>{if dir!=Dir::Right {ndir=Dir::Left;}}
                    KeyCode::Right |KeyCode::Char('d')|KeyCode::Char('D')=>{if dir!=Dir::Left  {ndir=Dir::Right;}}
                    KeyCode::Char('p')|KeyCode::Char('P') => { paused=!paused; }
                    KeyCode::Char('q')|KeyCode::Esc        => { break 'game true; }
                    _ => {}
                }}
            }
            if paused { draw_snake(&body,food,score,hi,gw,gh,dir,true); continue 'game; }
            dir = ndir;
            let (hx,hy) = *body.front().unwrap();
            let nh = match dir {
                Dir::Up=>(hx,hy-1), Dir::Down=>(hx,hy+1),
                Dir::Left=>(hx-1,hy), Dir::Right=>(hx+1,hy),
            };
            if nh.0<0||nh.0>=gw||nh.1<0||nh.1>=gh||body.contains(&nh) {
                break 'game false; // collision → death
            }
            body.push_front(nh);
            if nh==food { score+=1; food=snake_food(&body,&mut seed,gw,gh); }
            else        { body.pop_back(); }
            draw_snake(&body,food,score,hi,gw,gh,dir,false);
        };

        if quit { break 'session; }
        let new_best = score > hi;
        if new_best { hi = score; }

        // Death flash (3 alternations)
        for i in 0..6u8 {
            if i % 2 == 0 {
                execute!(io::stdout(), MoveTo(0,3)).ok();
                print!("{RED}{BLD}");
                for _ in 0..gh { print!("  {}\r\n","░".repeat(gw as usize*2+2)); }
                print!("{RST}"); flush();
            } else {
                draw_snake(&body,food,score,hi,gw,gh,dir,false);
            }
            thread::sleep(Duration::from_millis(90));
        }

        // Game-over overlay (centred on the board)
        let oy = (3 + gh as u16/2).saturating_sub(5);
        execute!(io::stdout(), MoveTo(4, oy)).ok();
        print!("{RED}{BLD}╔══════════════════════════════╗\r\n");
        print!("    ║   ☠   GAME  OVER   ☠        ║\r\n");
        print!("    ╠══════════════════════════════╣\r\n");
        print!("    ║  Score  : {score:>18}  ║\r\n");
        print!("    ║  Best   : {hi:>18}  ║\r\n");
        if new_best { print!("    ║  {YLW}★  NEW HIGH SCORE!  ★{RED}       ║\r\n"); }
        else        { print!("    ║                              ║\r\n"); }
        print!("    ╠══════════════════════════════╣\r\n");
        print!("    ║  {GRN}[R]{RED} Play Again  {WHT}[Q]{RED} Quit     ║\r\n");
        print!("    ╚══════════════════════════════╝{RST}\r\n");
        flush();

        // Wait for R or Q
        loop {
            if poll(Duration::from_millis(50)).unwrap_or(false) {
                if let Ok(Event::Key(KeyEvent{code,..})) = read() { match code {
                    KeyCode::Char('r')|KeyCode::Char('R')|KeyCode::Enter => { continue 'session; }
                    _ => { break 'session; }
                }}
            }
        }
    }

    execute!(io::stdout(), Show, LeaveAlternateScreen).ok();
    terminal::disable_raw_mode().ok();
    wait("\n  [Enter to continue]");
}

// ═══════════════════════════════════════════════════════════════════════════════
// STATE 3 — ASCII Video Player 🎬
// ffmpeg → raw RGB24 pipe → parallel Braille + delta-encode → 60 FPS playback
// ═══════════════════════════════════════════════════════════════════════════════
fn vid_info(path: &str) -> Option<(u32,u32,f64,u64)> {
    let out = Command::new(find_bin("ffprobe"))
        .args(["-v","quiet","-select_streams","v:0",
               "-show_entries","stream=width,height,r_frame_rate,nb_frames",
               "-of","csv=p=0",path])
        .output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    let p: Vec<&str> = s.trim().split(',').collect();
    if p.len() < 3 { return None; }
    let w: u32 = p[0].parse().ok()?;
    let h: u32 = p[1].parse().ok()?;
    let fps = if let Some((n,d)) = p[2].split_once('/') {
        n.parse::<f64>().ok()? / d.parse::<f64>().ok()?.max(1.0)
    } else { p[2].parse().ok()? };
    Some((w,h,fps,p.get(3).and_then(|v|v.parse().ok()).unwrap_or(0)))
}

fn state_video() {
    cls(); banner("STATE 3  ─  ASCII VIDEO PLAYER  🎬", GRN);
    let path = ask("Video path:").trim_matches('"').to_string();
    let fp   = Path::new(&path);
    if fp.is_dir()  { println!("  {RED}✗  That is a directory — include the filename.{RST}"); wait("\n  [Enter]"); return; }
    if !fp.exists() { println!("  {RED}✗  File not found: {path}{RST}");                     wait("\n  [Enter]"); return; }
    let (vw,vh,fps,total) = match vid_info(&path) {
        Some(v) => v,
        None    => { println!("  {RED}✗  Cannot read video — is ffprobe in PATH?{RST}"); wait("[Enter]"); return; }
    };
    println!("  {GRN}Source:{RST} {vw}×{vh} @ {fps:.1} FPS  ·  ~{total} frames");
    let tw  = pick_res(); let rgb = pick_color();
    if tw>=400&&rgb { println!("  {YLW}⚠  RGB at ≥400 cols is heavy — BW recommended.{RST}"); }
    let th         = ((vh as f32/vw as f32)*tw as f32*0.55) as u32;
    let fbytes     = (tw*th*3) as usize;
    let target_fps = 60.0f64; let delay = 1.0/target_fps;
    let est        = (total as f64*target_fps/fps.max(1.0)) as u64;
    let ncpu       = rayon::current_num_threads();
    let sz         = format!("{tw}x{th}");
    println!("  {YLW}🚀 {ncpu} cores · Target 60 FPS · Output {tw}×{th} Braille chars{RST}");
    println!("  {CYN}⏳ Phase 1/2 — Streaming frames via ffmpeg…{RST}");
    let mut proc = match Command::new(find_bin("ffmpeg"))
        .args(["-i",&path,"-vf","fps=60","-f","rawvideo","-pix_fmt","rgb24","-s",&sz,"-v","quiet","-"])
        .stdout(Stdio::piped()).stderr(Stdio::null()).spawn() {
        Ok(p)  => p,
        Err(_) => { println!("  {RED}✗  Cannot launch ffmpeg — is it in PATH?{RST}"); wait("[Enter]"); return; }
    };
    let mut reader = BufReader::with_capacity(fbytes*4, proc.stdout.take().unwrap());
    let mut raw: Vec<Vec<u8>> = Vec::new();
    let t0 = Instant::now();
    loop {
        let mut buf = vec![0u8; fbytes];
        if reader.read_exact(&mut buf).is_err() { break; }
        raw.push(buf);
        let n = raw.len() as u64;
        if n%60==0 {
            let el  = t0.elapsed().as_secs_f64();
            let eta = if est>0&&n>0 { (el/n as f64)*est.saturating_sub(n) as f64 } else { 0.0 };
            pbar(n,est,eta);
        }
    }
    drop(reader); proc.wait().ok();
    println!("\n  {GRN}✓ Phase 1 — {} frames in {:.1}s{RST}", raw.len(), t0.elapsed().as_secs_f64());
    println!("  {CYN}⚡ Phase 2/2 — Braille + delta-encode ({ncpu} threads)…{RST}");
    let ct = Instant::now();
    let brows: Vec<Vec<Vec<u8>>> = raw.par_iter().map(|buf| to_braille(buf,tw,th,rgb)).collect();
    drop(raw);
    let mut cache: Vec<Frame> = Vec::with_capacity(brows.len());
    let mut prev: Option<Vec<Vec<u8>>> = None;
    for rows in brows {
        let frame = match &prev {
            None    => Frame::Key(rows.clone()),
            Some(p) => {
                let dl: Vec<(usize,Vec<u8>)> = rows.iter().enumerate()
                    .filter(|(i,r)| p.get(*i).map(|o|o!=*r).unwrap_or(true))
                    .map(|(i,r)|(i,r.clone())).collect();
                if dl.len()>rows.len()/2 { Frame::Key(rows.clone()) } else { Frame::Delta(dl) }
            }
        };
        prev=Some(rows); cache.push(frame);
    }
    let keys = cache.iter().filter(|f|matches!(f,Frame::Key(_))).count();
    println!("  {GRN}✓ Phase 2 — {} frames in {:.1}s · {keys} keys + {} deltas{RST}",
        cache.len(), ct.elapsed().as_secs_f64(), cache.len()-keys);
    println!("  {GRN}🎬 Playing!  {DIM}SPACE = pause/resume · Q / Esc = quit{RST}\n");
    let mut out_h = io::stdout();
    terminal::enable_raw_mode().ok();
    execute!(out_h, EnterAlternateScreen, Hide).ok();
    let mut audio = Command::new(find_bin("ffplay"))
        .args(["-nodisp","-autoexit","-v","quiet",&path])
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().ok();
    let (mut paused, play_t0) = (false, Instant::now());
    let mut pause_total = Duration::ZERO; let mut pause_t = Instant::now();
    let mut fi = 0usize;
    let pause_msg = b"\x1b[H\x1b[1;33m  \xe2\x8f\xb8  PAUSED \xe2\x80\x94 SPACE to resume \xc2\xb7 Q to quit\x1b[0m";
    {
        let stdout = io::stdout(); let mut locked = stdout.lock();
        locked.write_all(b"\x1b[H").ok();
        while fi < cache.len() {
            if poll(Duration::ZERO).unwrap_or(false) {
                if let Ok(Event::Key(KeyEvent{code,..})) = read() { match code {
                    KeyCode::Char(' ') => {
                        paused=!paused;
                        if paused {
                            pause_t=Instant::now();
                            if let Some(ref mut a)=audio { a.kill().ok(); }
                        } else {
                            pause_total+=pause_t.elapsed();
                            let off=(play_t0.elapsed()-pause_total).as_secs_f64();
                            audio=Command::new(find_bin("ffplay"))
                                .args(["-nodisp","-autoexit","-v","quiet","-ss",&format!("{off:.2}"),&path])
                                .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().ok();
                        }
                    }
                    KeyCode::Char('q')|KeyCode::Esc => break,
                    _ => {}
                }}
            }
            if paused {
                locked.write_all(pause_msg).ok(); locked.flush().ok();
                thread::sleep(Duration::from_millis(50)); continue;
            }
            match &cache[fi] {
                Frame::Key(rows) => {
                    locked.write_all(b"\x1b[H").ok();
                    for row in rows { locked.write_all(row).ok(); }
                }
                Frame::Delta(dl) => {
                    for (ri,rb) in dl {
                        locked.write_all(format!("\x1b[{};1H",ri+1).as_bytes()).ok();
                        locked.write_all(rb).ok();
                    }
                }
            }
            locked.flush().ok(); fi+=1;
            let target = Duration::from_secs_f64(fi as f64 * delay);
            let actual = play_t0.elapsed() - pause_total;
            if target>actual { thread::sleep(target-actual); }
        }
    }
    execute!(out_h, Show, LeaveAlternateScreen).ok();
    terminal::disable_raw_mode().ok();
    if let Some(ref mut a)=audio { a.kill().ok(); a.wait().ok(); }
    println!("\n  {GRN}✓ Playback complete!{RST}");
    wait("\n  [Enter to continue]");
}

// ═══════════════════════════════════════════════════════════════════════════════
// MAIN — 3-state menu loop
// ═══════════════════════════════════════════════════════════════════════════════
fn main() {
    loop {
        show_menu();
        match ask("Select state (0–3):").as_str() {
            "1" => state_image(),
            "2" => state_snake(),
            "3" => state_video(),
            "0" => { cls(); println!("  {PUR}{BLD}Goodbye! 👋{RST}\n"); break; }
            _   => { println!("  {RED}✗  Invalid — enter 1, 2, 3 or 0.{RST}"); thread::sleep(Duration::from_secs(1)); }
        }
    }
}
