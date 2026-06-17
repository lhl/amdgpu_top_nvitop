//! Ring-buffer history + braille graph rendering (nvitop-faithful algorithm).
//!
//! Each terminal cell holds a 2×4 braille grid = 2 horizontal data samples ×
//! 4 vertical dots. With 5 levels per column (0..=4 dots), each cell encodes
//! a (left_level, right_level) pair. A graph `H` rows tall has `4*H` vertical
//! sub-pixels. Colored per-cell via the theme gradient.

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

use crate::theme::Gradient;

pub struct History {
    buf: Vec<u64>,
    cap: usize,
}

impl History {
    pub fn new(cap: usize) -> Self {
        Self { buf: Vec::with_capacity(cap), cap }
    }

    pub fn push(&mut self, v: u64) {
        if self.buf.len() >= self.cap {
            self.buf.remove(0);
        }
        self.buf.push(v.min(100));
    }

    pub fn buf_last(&self) -> u64 {
        *self.buf.last().unwrap_or(&0)
    }

    /// Render a multi-row braille area graph of the most recent `2*width`
    /// samples. `height` = terminal rows. Values 0..100. Each cell colored by
    /// the theme gradient sampled at the cell's peak value.
    pub fn braille_graph(
        &self,
        width: usize,
        height: usize,
        grad: Gradient,
    ) -> Vec<Line<'static>> {
        if height == 0 || width == 0 {
            return Vec::new();
        }
        // We need 2*width samples, right-aligned so the newest is at the right
        // edge. Pad the LEFT with baseline zeros when the buffer isn't full.
        let need = 2 * width;
        let n = self.buf.len();
        let take = n.min(need);
        let pad = need - take;
        let window: Vec<u64> = (0..need)
            .map(|i| if i < pad { 0 } else { self.buf[n - take + (i - pad)] })
            .collect();

        // Build per-row strings. Row 0 = bottom (h=0).
        // bound=100, baseline=0. scaled value = height * (v/100).
        let mut rows: Vec<String> = vec![String::with_capacity(width); height];
        let mut row_colors: Vec<Vec<Color>> = vec![vec![Color::Reset; width]; height];

        for col in 0..width {
            let v1 = window.get(2 * col).copied().unwrap_or(0) as f64;
            let v2 = window.get(2 * col + 1).copied().unwrap_or(0) as f64;
            let s1f = height as f64 * (v1 / 100.0);
            let s2f = height as f64 * (v2 / 100.0);
            let s1f = if v1 >= 0.0 { s1f.max(0.2) } else { s1f };
            let s2f = if v2 >= 0.0 { s2f.max(0.2) } else { s2f };
            let peak = v1.max(v2);
            let cell_color = grad.sample(peak / 100.0);
            for h in 0..height {
                let l1 = (5.0 * (s1f - h as f64)).round().clamp(0.0, 4.0) as u8;
                let l2 = (5.0 * (s2f - h as f64)).round().clamp(0.0, 4.0) as u8;
                rows[h].push(braille_char(l1, l2));
                row_colors[h][col] = if l1 == 0 && l2 == 0 {
                    Color::Reset
                } else {
                    cell_color
                };
            }
        }

        // nvitop reverses so index 0 = top row. We render top-first.
        let mut lines: Vec<Line<'static>> = Vec::with_capacity(height);
        for h in (0..height).rev() {
            let spans: Vec<Span<'static>> = rows[h]
                .chars()
                .enumerate()
                .map(|(i, c)| {
                    let col = row_colors[h][i];
                    if col == Color::Reset {
                        Span::raw(c.to_string())
                    } else {
                        Span::styled(c.to_string(), Style::default().fg(col))
                    }
                })
                .collect();
            lines.push(Line::from(spans));
        }
        lines
    }

    /// Single-row eighths sparkline (kept for compact/collapsed views).
    pub fn sparkline(&self, width: usize, color: Color) -> Line<'static> {
        const RAMP: [char; 9] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
        let n = self.buf.len();
        let start = n.saturating_sub(width);
        let mut spans: Vec<Span<'static>> = Vec::with_capacity(width);
        for i in 0..width {
            let idx = start + i;
            if idx >= n {
                spans.push(Span::raw(" "));
            } else {
                let v = self.buf[idx] as usize;
                let r = (v * 8 / 100).min(8);
                if r == 0 {
                    spans.push(Span::styled(
                        "▁",
                        Style::default().fg(Color::Rgb(0x4, 0x4, 0x4a)),
                    ));
                } else {
                    spans.push(Span::styled(RAMP[r].to_string(), Style::default().fg(color)));
                }
            }
        }
        Line::from(spans)
    }
}

/// Build a braille char from left/right column fill levels (0..=4 each).
/// Left column dots (bottom-up): 7,3,2,1. Right column: 8,6,5,4.
fn braille_char(l1: u8, l2: u8) -> char {
    let mut bits: u32 = 0;
    if l1 >= 1 { bits |= 0x40; } // dot 7
    if l1 >= 2 { bits |= 0x04; } // dot 3
    if l1 >= 3 { bits |= 0x02; } // dot 2
    if l1 >= 4 { bits |= 0x01; } // dot 1
    if l2 >= 1 { bits |= 0x80; } // dot 8
    if l2 >= 2 { bits |= 0x20; } // dot 6
    if l2 >= 3 { bits |= 0x10; } // dot 5
    if l2 >= 4 { bits |= 0x08; } // dot 4
    char::from_u32(0x2800 + bits).unwrap_or(' ')
}
