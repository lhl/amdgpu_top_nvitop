//! nvitop/btop-style block gauges with a FIXED-width track so bars align.
//! Layout:  `LABEL ███████░░░░  62%   used / total`
//!          [label][----- track -----][pct][--- value field ---]
//! The track width is `width - label - pct - value_field`, so as long as
//! callers in the same band pass the same `width` and `value_field`, every
//! bar's track is identical length and the percentages line up in a column.

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

use crate::theme::{Theme, UtilKind};

const SMOOTH_RAMP: [&str; 9] = ["", "▏", "▎", "▍", "▌", "▋", "▊", "▉", "█"];

/// A user-cycleable gauge fill style. `full` is the glyph for a complete cell,
/// `empty` for the unfilled track. `ramp` Some(..) renders precise fractional
/// sub-cells (the leading edge); None rounds to the nearest whole cell.
pub struct BlockStyle {
    pub name: &'static str,
    pub full: &'static str,
    pub empty: &'static str,
    pub ramp: Option<[&'static str; 9]>,
}

pub const BLOCK_STYLES: &[BlockStyle] = &[
    // index 0 is the default: ¾ blocks give a lighter, "broken up" look
    BlockStyle { name: "3/4", full: "▊", empty: "░", ramp: None },
    BlockStyle { name: "smooth", full: "█", empty: "░", ramp: Some(SMOOTH_RAMP) },
    BlockStyle { name: "full", full: "█", empty: "░", ramp: None },
    BlockStyle { name: "7/8", full: "▉", empty: "░", ramp: None },
    BlockStyle { name: "5/8", full: "▋", empty: "░", ramp: None },
    BlockStyle { name: "1/2", full: "▌", empty: "░", ramp: None },
    BlockStyle { name: "3/8", full: "▍", empty: "░", ramp: None },
    BlockStyle { name: "1/4", full: "▎", empty: "░", ramp: None },
    BlockStyle { name: "1/8", full: "▏", empty: "░", ramp: None },
    BlockStyle { name: "dark", full: "▓", empty: "░", ramp: None },
    BlockStyle { name: "medium", full: "▒", empty: "░", ramp: None },
    BlockStyle { name: "dots", full: "●", empty: "·", ramp: None },
    BlockStyle { name: "lines", full: "━", empty: "─", ramp: None },
    BlockStyle { name: "squares", full: "■", empty: "□", ramp: None },
    BlockStyle { name: "pills", full: "▰", empty: "▱", ramp: None },
];

pub fn block_style(i: usize) -> &'static BlockStyle {
    &BLOCK_STYLES[i % BLOCK_STYLES.len()]
}

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

/// Gauge with a fixed-width track. `value` is an absolute string (e.g.
/// "60.4G / 117.1G") shown right-aligned in a `value_field`-wide column after
/// the percentage. Pass `value=""` to reserve the field without text (keeps the
/// track aligned with sibling bars that do have values).
pub fn bar(
    label: &str,
    pct: Option<f64>,
    value: &str,
    width: usize,
    value_field: usize,
    kind: Kind,
    theme: &Theme,
    style: &BlockStyle,
) -> Line<'static> {
    let label_part = format!("{label} ");
    let pct_str = match pct {
        Some(p) => format!("{:>3.0}%", p.round().clamp(0.0, 100.0)),
        None => " N/A".to_string(),
    };
    // reserved = label + " " + pct(4) + (value_field + 2 separators)
    let reserved = label_part.chars().count()
        + 1
        + pct_str.chars().count()
        + if value_field > 0 { 2 + value_field } else { 0 };
    let track = width.saturating_sub(reserved);

    let mut spans: Vec<Span<'static>> = Vec::with_capacity(5);
    spans.push(Span::styled(label_part, Style::default().fg(theme.graph_text())));

    let fill_color = pct
        .map(|p| theme.util_color(p, kind.util_kind()))
        .unwrap_or(theme.inactive_fg());

    match pct {
        Some(p) => {
            let clamped = (p / 100.0).clamp(0.0, 1.0);
            let n = (track as f64 * clamped * 8.0).round() as usize;
            let (mut q, r) = (n / 8, n % 8);
            let partial = match style.ramp {
                Some(ramp) if r > 0 => ramp[r].to_string(),
                Some(_) => String::new(),
                None => {
                    if r >= 4 {
                        q += 1; // round to nearest whole cell
                    }
                    String::new()
                }
            };
            let q = q.min(track);
            let filled = style.full.repeat(q);
            let used = q + partial.chars().count();
            let empty = style.empty.repeat(track.saturating_sub(used));
            spans.push(Span::styled(format!("{filled}{partial}"), Style::default().fg(fill_color)));
            spans.push(Span::styled(empty, Style::default().fg(theme.inactive_fg())));
        }
        None => spans.push(Span::styled(
            style.empty.repeat(track),
            Style::default().fg(theme.inactive_fg()),
        )),
    }

    spans.push(Span::styled(format!(" {pct_str}"), Style::default().fg(fill_color)));
    if value_field > 0 {
        spans.push(Span::styled(
            format!("  {value:>value_field$}"),
            Style::default().fg(theme.main_fg()),
        ));
    }
    Line::from(spans)
}

/// Convenience: a gauge whose only annotation is its percentage.
pub fn line(
    label: &str,
    pct: Option<f64>,
    width: usize,
    kind: Kind,
    theme: &Theme,
    style: &BlockStyle,
) -> Line<'static> {
    bar(label, pct, "", width, 0, kind, theme, style)
}

#[allow(dead_code)]
fn _color_use(_c: Color) {}
