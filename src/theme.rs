//! btop theme loader. Parses standard `.theme` files (hex `#RRGGBB`,
//! 2-char grayscale `#BW`, or `R G B` decimal) and exposes resolved colors
//! + gradient samplers. Defaults to `everforest-dark-hard`.
//!
//! Search paths (first hit wins):
//!   $XDG_CONFIG_HOME/btop/themes/  (~/.config/btop/themes)
//!   /usr/local/share/btop/themes/
//!   /usr/share/btop/themes/

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use ratatui::style::Color;

pub const DEFAULT_THEME: &str = "everforest-dark-hard";

#[derive(Clone, Copy)]
pub struct Gradient {
    pub start: Color,
    pub mid: Option<Color>,
    pub end: Option<Color>,
}

impl Gradient {
    /// Sample the gradient at `t` in [0,1].
    /// - start only -> flat
    /// - start+end -> linear lerp
    /// - start+mid+end -> start->mid (t<0.5), mid->end (t>=0.5)
    pub fn sample(self, t: f64) -> Color {
        let t = t.clamp(0.0, 1.0);
        match (self.mid, self.end) {
            (None, None) => self.start,
            (None, Some(e)) => lerp(self.start, e, t),
            (Some(m), Some(e)) => {
                if t < 0.5 {
                    lerp(self.start, m, t * 2.0)
                } else {
                    lerp(m, e, (t - 0.5) * 2.0)
                }
            }
            (Some(_), None) => self.start,
        }
    }
}

fn lerp(a: Color, b: Color, t: f64) -> Color {
    let (ar, ag, ab) = to_rgb(a);
    let (br, bg, bb) = to_rgb(b);
    Color::Rgb(
        (ar as f64 + (br as f64 - ar as f64) * t).round() as u8,
        (ag as f64 + (bg as f64 - ag as f64) * t).round() as u8,
        (ab as f64 + (bb as f64 - ab as f64) * t).round() as u8,
    )
}

fn to_rgb(c: Color) -> (u8, u8, u8) {
    match c {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Indexed(i) => {
            // 6x6x6 cube
            if (216..=231).contains(&i) {
                let i = i - 216;
                (36 + (i / 36) * 51, 36 + ((i / 6) % 6) * 51, 36 + (i % 6) * 51)
            } else if (232..=255).contains(&i) {
                let v = 8 + (i - 232) * 10;
                (v, v, v)
            } else {
                (200, 200, 200)
            }
        }
        _ => (200, 200, 200),
    }
}

/// Parse a btop color value. Formats: `#RRGGBB`, `#BW` (2-hex grayscale),
/// `R G B` (decimal). Empty string -> None (terminal default / transparent).
fn parse_color(s: &str) -> Option<Color> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if let Some(h) = s.strip_prefix('#') {
        if h.len() == 6 {
            let r = u8::from_str_radix(&h[0..2], 16).ok()?;
            let g = u8::from_str_radix(&h[2..4], 16).ok()?;
            let b = u8::from_str_radix(&h[4..6], 16).ok()?;
            return Some(Color::Rgb(r, g, b));
        }
        if h.len() == 2 {
            // 2-hex grayscale: "#ff" -> 255
            let v = u8::from_str_radix(h, 16).ok()?;
            return Some(Color::Rgb(v, v, v));
        }
        return None;
    }
    // decimal "R G B"
    let nums: Vec<u8> = s.split_whitespace().filter_map(|p| p.parse().ok()).collect();
    if nums.len() == 3 {
        return Some(Color::Rgb(nums[0], nums[1], nums[2]));
    }
    None
}

pub struct Theme {
    raw: HashMap<String, String>,
}

impl Theme {
    /// Load a named btop theme from the standard search paths.
    /// Falls back to the built-in everforest-dark-hard palette if not found.
    pub fn load(name: &str) -> Self {
        for dir in search_dirs() {
            let p = dir.join(format!("{name}.theme"));
            if p.exists() {
                if let Ok(text) = fs::read_to_string(&p) {
                    return Self::parse(&text);
                }
            }
        }
        Self::parse(EVERFOREST_FALLBACK)
    }

    /// Load the default theme (everforest-dark-hard).
    pub fn default_theme() -> Self {
        Self::load(DEFAULT_THEME)
    }

    /// List all available theme names found in the search paths (sorted, unique).
    pub fn list_available() -> Vec<String> {
        let mut set = std::collections::BTreeSet::new();
        for dir in search_dirs() {
            if let Ok(rd) = fs::read_dir(&dir) {
                for e in rd.flatten() {
                    let p = e.path();
                    if p.extension().and_then(|x| x.to_str()) == Some("theme") {
                        if let Some(stem) = p.file_stem().and_then(|x| x.to_str()) {
                            set.insert(stem.to_string());
                        }
                    }
                }
            }
        }
        set.insert(DEFAULT_THEME.to_string());
        set.into_iter().collect()
    }

    fn parse(text: &str) -> Self {
        let mut raw = HashMap::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            // theme[key]="value"
            if let Some(rest) = line.strip_prefix("theme[") {
                if let Some(close) = rest.find(']') {
                    let key = rest[..close].to_string();
                    let val_part = rest[close + 1..].trim_start();
                    let val = val_part.strip_prefix('=').unwrap_or(val_part).trim();
                    // strip quotes
                    let val = val.trim_matches('"');
                    raw.insert(key, val.to_string());
                }
            }
        }
        Self { raw }
    }

    fn color(&self, key: &str) -> Option<Color> {
        self.raw.get(key).and_then(|s| parse_color(s))
    }

    fn color_or(&self, key: &str, default: Color) -> Color {
        self.color(key).unwrap_or(default)
    }

    fn gradient(&self, name: &str) -> Gradient {
        // name like "cpu" -> cpu_start / cpu_mid / cpu_end
        let start = self
            .color(&format!("{name}_start"))
            .unwrap_or(Color::Rgb(0xa7, 0xc0, 0x80));
        let mid = self.color(&format!("{name}_mid"));
        let end = self.color(&format!("{name}_end"));
        Gradient { start, mid, end }
    }

    // --- resolved accessors ---

    /// Main background. None => transparent (use terminal default).
    pub fn main_bg(&self) -> Option<Color> {
        self.color("main_bg")
    }
    pub fn main_fg(&self) -> Color {
        self.color_or("main_fg", Color::Rgb(0xd3, 0xc6, 0xaa))
    }
    pub fn title(&self) -> Color {
        self.color_or("title", self.main_fg())
    }
    pub fn hi_fg(&self) -> Color {
        self.color_or("hi_fg", Color::Rgb(0xe6, 0x7e, 0x80))
    }
    pub fn selected_bg(&self) -> Color {
        self.color_or("selected_bg", Color::Rgb(0x37, 0x41, 0x45))
    }
    pub fn selected_fg(&self) -> Color {
        self.color_or("selected_fg", Color::Rgb(0xdb, 0xbc, 0x7f))
    }
    pub fn inactive_fg(&self) -> Color {
        self.color_or("inactive_fg", Color::Rgb(0x50, 0x49, 0x45))
    }
    pub fn graph_text(&self) -> Color {
        self.color_or("graph_text", self.main_fg())
    }
    pub fn proc_misc(&self) -> Color {
        self.color_or("proc_misc", Color::Rgb(0xa7, 0xc0, 0x80))
    }
    pub fn div_line(&self) -> Color {
        self.color_or("div_line", Color::Rgb(0x37, 0x41, 0x45))
    }

    /// Box outline color for a given section kind.
    pub fn box_color(&self, kind: SectionBox) -> Color {
        let key = match kind {
            SectionBox::Cpu => "cpu_box",
            SectionBox::Mem => "mem_box",
            SectionBox::Net => "net_box",
            SectionBox::Proc => "proc_box",
        };
        self.color_or(key, self.div_line())
    }

    // gradients
    pub fn temp(&self) -> Gradient { self.gradient("temp") }
    pub fn cpu(&self) -> Gradient { self.gradient("cpu") }
    pub fn used(&self) -> Gradient { self.gradient("used") }
    pub fn free(&self) -> Gradient { self.gradient("free") }
    pub fn cached(&self) -> Gradient { self.gradient("cached") }
    pub fn available(&self) -> Gradient { self.gradient("available") }
    pub fn process(&self) -> Gradient { self.gradient("process") }

    /// nvitop-style discrete threshold color for a percentage, but sourced
    /// from the theme's `used`/`temp` gradients so it matches the palette.
    pub fn util_color(&self, pct: f64, kind: UtilKind) -> Color {
        // map nvitop thresholds onto gradient positions:
        // light (<10%) -> start, moderate (10-75/80%) -> mid-ish, heavy -> end
        let g = match kind {
            UtilKind::Gpu => self.cpu(),
            UtilKind::Mem => self.used(),
            UtilKind::Npu => self.process(),
        };
        let t = (pct / 100.0).clamp(0.0, 1.0);
        g.sample(t)
    }
}

#[derive(Clone, Copy)]
pub enum SectionBox { Cpu, Mem, Net, Proc }

#[derive(Clone, Copy)]
pub enum UtilKind { Gpu, Mem, Npu }

fn search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        dirs.push(PathBuf::from(xdg).join("btop/themes"));
    }
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(PathBuf::from(home).join(".config/btop/themes"));
    }
    dirs.push(PathBuf::from("/usr/local/share/btop/themes"));
    dirs.push(PathBuf::from("/usr/share/btop/themes"));
    dirs
}

/// Bundled fallback in case no theme files are installed on the system.
/// Minimal everforest-dark-hard palette.
const EVERFOREST_FALLBACK: &str = r##"
theme[main_bg]="#272e33"
theme[main_fg]="#d3c6aa"
theme[title]="#d3c6aa"
theme[hi_fg]="#e67e80"
theme[selected_bg]="#374145"
theme[selected_fg]="#dbbc7f"
theme[inactive_fg]="#272e33"
theme[graph_text]="#d3c6aa"
theme[proc_misc]="#a7c080"
theme[cpu_box]="#374145"
theme[mem_box]="#374145"
theme[net_box]="#374145"
theme[proc_box]="#374145"
theme[div_line]="#374145"
theme[temp_start]="#a7c080"
theme[temp_mid]="#dbbc7f"
theme[temp_end]="#f85552"
theme[cpu_start]="#a7c080"
theme[cpu_mid]="#dbbc7f"
theme[cpu_end]="#f85552"
theme[used_start]="#a7c080"
theme[used_mid]="#dbbc7f"
theme[used_end]="#f85552"
theme[free_start]="#f85552"
theme[free_mid]="#dbbc7f"
theme[free_end]="#a7c080"
theme[process_start]="#a7c080"
theme[process_mid]="#f85552"
theme[process_end]="#CC241D"
"##;
