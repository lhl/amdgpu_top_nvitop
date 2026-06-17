//! nvitop/btop-style block gauges with numeric annotations.
//! `█` full, `▏▎▍▌▋▊▉` 8-substep partial, `░` empty. Gradient fill from theme.

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

/// Gauge with a custom right-aligned numeric annotation (e.g. "33.4/64.0G 52%").
pub fn bar(
    label: &str,
    pct: Option<f64>,
    annotation: &str,
    width: usize,
    kind: Kind,
    theme: &Theme,
) -> Line<'static> {
    let label_part = format!("{label} ");
    let ann = if annotation.is_empty() {
        String::new()
    } else {
        format!("  {annotation}")
    };
    let reserved = label_part.chars().count() + ann.chars().count();
    let avail = width.saturating_sub(reserved);

    let mut spans: Vec<Span<'static>> = Vec::with_capacity(4);
    spans.push(Span::styled(label_part, Style::default().fg(theme.graph_text())));

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
        None => spans.push(Span::styled(
            "░".repeat(avail),
            Style::default().fg(theme.inactive_fg()),
        )),
    }
    if !ann.is_empty() {
        spans.push(Span::styled(ann, Style::default().fg(theme.main_fg())));
    }
    Line::from(spans)
}

/// Convenience: gauge annotated with just its percentage.
pub fn line(label: &str, pct: Option<f64>, width: usize, kind: Kind, theme: &Theme) -> Line<'static> {
    let ann = match pct {
        Some(p) => format!("{:>3.0}%", p.round().clamp(0.0, 100.0)),
        None => "N/A".to_string(),
    };
    bar(label, pct, &ann, width, kind, theme)
}

#[allow(dead_code)]
fn _color_use(_c: Color) {}
