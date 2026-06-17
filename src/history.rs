//! Ring-buffer history + braille/eighths sparkline rendering.

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

// vertical eighths ramp; index 0 = empty baseline
const RAMP: [char; 9] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

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

    /// Most recent pushed value, or 0 if empty.
    pub fn buf_last(&self) -> u64 {
        *self.buf.last().unwrap_or(&0)
    }

    /// Render the most recent `width` samples as a sparkline.
    /// Zero values use a dim baseline char; nonzero use `color` on the ramp.
    pub fn sparkline(&self, width: usize, color: Color) -> Line<'static> {
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
                    spans.push(Span::styled(
                        RAMP[r].to_string(),
                        Style::default().fg(color),
                    ));
                }
            }
        }
        Line::from(spans)
    }
}
