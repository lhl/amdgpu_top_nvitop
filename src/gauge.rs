//! nvitop-style block gauges: `█` full, `▏▎▍▌▋▊▉` 8-substep partial, `░` empty.
//! Color is sourced from the loaded btop theme's gradient.

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

use crate::theme::{Theme, UtilKind};

const PARTIAL: [&str; 9] = ["", "▏", "▎", "▍", "▌", "▋", "▊", "▉", "█"];

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    Gpu,
    Mem,
    Npu,
}

impl Kind {
    fn util_kind(self) -> UtilKind {
        match self {
            Kind::Gpu => UtilKind::Gpu,
            Kind::Mem => UtilKind::Mem,
            Kind::Npu => UtilKind::Npu,
        }
    }
}

/// Build a nvitop-style gauge line: `LABEL: ██████░░░░ 45%`.
/// Color is sourced from the loaded btop theme's gradient.
pub fn line(
    label: &str,
    pct: Option<f64>,
    width: usize,
    kind: Kind,
    theme: &Theme,
) -> Line<'static> {
    let label_part = format!("{label}: ");
    let text = match pct {
        Some(p) => format!(" {:>3.0}%", p.round().clamp(0.0, 100.0)),
        None => "  N/A".to_string(),
    };
    let avail = width.saturating_sub(label_part.chars().count() + text.chars().count());

    let mut spans: Vec<Span<'static>> = Vec::with_capacity(4);
    spans.push(Span::raw(label_part));

    match pct {
        Some(p) => {
            let clamped = (p / 100.0).clamp(0.0, 1.0);
            let n = (avail as f64 * clamped * 8.0).round() as usize;
            let (q, r) = (n / 8, n % 8);
            let g = theme.util_color(p, kind.util_kind());
            let filled = "█".repeat(q);
            let partial = if r > 0 { PARTIAL[r].to_string() } else { String::new() };
            let used = q + partial.chars().count();
            let empty = "░".repeat(avail.saturating_sub(used));
            spans.push(Span::styled(format!("{filled}{partial}"), Style::default().fg(g)));
            spans.push(Span::styled(empty, Style::default().fg(theme.inactive_fg())));
        }
        None => {
            spans.push(Span::styled(
                "░".repeat(avail),
                Style::default().fg(theme.inactive_fg()),
            ));
        }
    }
    spans.push(Span::raw(text));
    Line::from(spans)
}

// Suppress unused import warning when Color is only used transitively.
#[allow(dead_code)]
fn _color_use(_c: Color) {}
