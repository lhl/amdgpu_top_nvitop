//! Modern TUI rendering: rounded borders, btop-themed gradients, braille/eighths
//! sparklines, collapsible CPU/GPU/NPU/Processes sections.

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::symbols::border;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table};
use ratatui::Frame;

use crate::app::{gpu_mem_info, App, Section};
use crate::gauge::{self, Kind};
use crate::theme::SectionBox;

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();

    // background fill
    if let Some(bg) = app.theme.main_bg() {
        f.render_widget(
            Block::default().style(Style::default().bg(bg)),
            area,
        );
    }

    let constraints = build_constraints(app, area.height);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    let mut idx = 1; // skip header
    draw_header(f, chunks[0], app);

    draw_cpu(f, chunks[idx], app);
    idx += 1;
    draw_gpu(f, chunks[idx], app);
    idx += 1;
    if app.has_npu {
        draw_npu(f, chunks[idx], app);
        idx += 1;
    }
    draw_processes(f, chunks[idx], app);
    draw_footer(f, chunks[area_height_idx(app)], app);
}

fn area_height_idx(app: &App) -> usize {
    // last chunk index = number of sections + header
    1 + 3 + if app.has_npu { 1 } else { 0 }
}

fn build_constraints(app: &App, _h: u16) -> Vec<Constraint> {
    let mut c = vec![Constraint::Length(1)]; // header
    c.push(section_height(app, Section::Cpu));
    c.push(section_height(app, Section::Gpu));
    if app.has_npu {
        c.push(section_height(app, Section::Npu));
    }
    c.push(section_height(app, Section::Processes));
    c.push(Constraint::Length(1)); // footer
    c
}

fn section_height(app: &App, s: Section) -> Constraint {
    if app.is_collapsed(s) {
        return Constraint::Length(3);
    }
    let inner = match s {
        Section::Cpu => 7,
        Section::Gpu => 2 + (4 * app.apps.len() as u16),
        Section::Npu => {
            let ctx = app
                .apps
                .iter()
                .map(|a| a.stat.xdna_fdinfo.proc_usage.len())
                .sum::<usize>();
            4 + ctx.min(6) as u16
        }
        Section::Processes => 3 + 10,
    };
    Constraint::Length(inner + 2)
}

// ---------- helpers ----------

fn section_block(app: &App, s: Section, title: &str, box_kind: SectionBox) -> Block<'static> {
    let focused = app.section == s;
    let indicator = if app.is_collapsed(s) { "▾" } else { "▸" };
    let title_span = Span::styled(
        format!(" {indicator} {title} "),
        Style::default()
            .fg(if focused { app.theme.hi_fg() } else { app.theme.title() })
            .add_modifier(Modifier::BOLD),
    );
    let border_color = if focused {
        app.theme.hi_fg()
    } else {
        app.theme.box_color(box_kind)
    };
    Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(border_color))
        .title(title_span)
}

fn gauge_line(app: &App, label: &str, pct: Option<f64>, width: usize, kind: Kind) -> Line<'static> {
    gauge::line(label, pct, width, kind, &app.theme)
}

// ---------- header ----------

fn draw_header(f: &mut Frame, area: Rect, app: &App) {
    let now = chrono_like();
    let left = Span::styled(
        " amdgpu-top-nvitop ",
        Style::default()
            .fg(app.theme.hi_fg())
            .add_modifier(Modifier::BOLD),
    );
    let mid = Span::styled(
        format!(" {} devices ", app.apps.len()),
        Style::default().fg(app.theme.graph_text()),
    );
    let right = Span::styled(
        format!(" {now}  q quit · tab section · space collapse "),
        Style::default().fg(app.theme.inactive_fg()),
    );
    let line = Line::from(vec![left, mid, right]);
    f.render_widget(Paragraph::new(line).alignment(Alignment::Left), area);
}

fn chrono_like() -> String {
    // avoid a chrono dependency; use the system date command via /proc uptime? simpler: std time
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // crude civil time conversion
    let (wday, mon, day, hh, mm, ss) = civil(secs);
    let _ = wday;
    format!("{mon:02} {day:02} {hh:02}:{mm:02}:{ss:02}")
}

fn civil(secs: u64) -> (u64, u64, u64, u64, u64, u64) {
    // local timezone offset: read /etc/localtime is heavy; use TZ env via libc? keep UTC for now.
    let days = secs / 86400;
    let rem = secs % 86400;
    let hh = rem / 3600;
    let mm = (rem % 3600) / 60;
    let ss = rem % 60;
    // civil from days since 1970-01-01 (Howard Hinnant)
    let z = days as i64 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let _y = if m <= 2 { y + 1 } else { y };
    let wday = (days + 4) % 7; // 1970-01-01 was Thursday=4
    (wday, m, d, hh, mm, ss)
}

// ---------- CPU ----------

fn draw_cpu(f: &mut Frame, area: Rect, app: &mut App) {
    let block = section_block(app, Section::Cpu, "CPU", SectionBox::Cpu);
    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.is_collapsed(Section::Cpu) {
        let line = Line::from(vec![
            Span::styled(
                format!(" {:>3.0}% ", app.cpu.cpu_percent.round()),
                Style::default().fg(app.theme.util_color(app.cpu.cpu_percent, crate::theme::UtilKind::Gpu)),
            ),
            Span::styled(
                format!("load {:.2} {:.2} {:.2}  ", app.mem.load1, app.mem.load5, app.mem.load15),
                Style::default().fg(app.theme.graph_text()),
            ),
            Span::styled(
                format!("MEM {:.1}G/{:.0}G  SWP {:.1}G/{:.0}G", app.mem.mem_used_gb(), app.mem.mem_total_gb(), app.mem.swap_used_gb(), app.mem.swap_total_gb()),
                Style::default().fg(app.theme.proc_misc()),
            ),
        ]);
        f.render_widget(Paragraph::new(line), inner);
        return;
    }

    let w = inner.width as usize;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // cpu gauge
            Constraint::Length(1), // mem gauge
            Constraint::Length(1), // swap gauge
            Constraint::Length(1), // sparkline
            Constraint::Length(1), // cores / temp
            Constraint::Min(0),
        ])
        .split(inner);

    f.render_widget(
        Paragraph::new(gauge_line(app, "CPU", Some(app.cpu.cpu_percent), w, Kind::Gpu)),
        chunks[0],
    );
    f.render_widget(
        Paragraph::new(gauge_line(app, "MEM", Some(app.mem.mem_used_pct()), w, Kind::Mem)),
        chunks[1],
    );
    f.render_widget(
        Paragraph::new(gauge_line(app, "SWP", Some(app.mem.swap_used_pct()), w, Kind::Mem)),
        chunks[2],
    );
    f.render_widget(
        Paragraph::new(app.hist_cpu.sparkline(w, app.theme.cpu().sample(0.5))),
        chunks[3],
    );

    // tctl temp + load avg + per-core freq
    let tctl = app
        .apps
        .iter()
        .find_map(|a| a.stat.sensors.as_ref().and_then(|s| s.tctl));
    let cores = app
        .apps
        .iter()
        .find_map(|a| a.stat.sensors.as_ref().map(|s| s.all_cpu_core_freq_info.clone()))
        .unwrap_or_default();
    let core_str = if cores.is_empty() {
        String::from("n/a")
    } else {
        let avg = cores.iter().map(|c| c.cur).sum::<u32>() / cores.len() as u32;
        format!("{avg}MHz ({} cores)", cores.len())
    };
    let line = Line::from(vec![
        Span::styled(
            format!(" CPU temp: {}  ", tctl.map(|t| format!("{}°C", t / 1000)).unwrap_or_else(|| "n/a".into())),
            Style::default().fg(app.theme.temp().sample(0.5)),
        ),
        Span::styled(
            format!("load {:.2}/{:.2}/{:.2}  ", app.mem.load1, app.mem.load5, app.mem.load15),
            Style::default().fg(app.theme.graph_text()),
        ),
        Span::styled(core_str, Style::default().fg(app.theme.proc_misc())),
    ]);
    f.render_widget(Paragraph::new(line), chunks[4]);
}

// ---------- GPU ----------

fn draw_gpu(f: &mut Frame, area: Rect, app: &mut App) {
    let block = section_block(app, Section::Gpu, "GPU", SectionBox::Mem);
    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.is_collapsed(Section::Gpu) {
        // compact summary line per gpu
        let mut spans: Vec<Span> = Vec::new();
        for (i, a) in app.apps.iter().enumerate() {
            let gfx = a.stat.activity.gfx.unwrap_or(0);
            let (_, mem_pct, mi) = gpu_mem_info(a);
            let name = short_name(&a.device_path.menu_entry());
            spans.push(Span::styled(
                format!(" {} {:>3}% gfx {:>3}% mem  ", name, gfx, mem_pct.round() as i64),
                Style::default().fg(app.theme.util_color(gfx as f64, crate::theme::UtilKind::Gpu)),
            ));
            let _ = i;
        }
        f.render_widget(Paragraph::new(Line::from(spans)), inner);
        return;
    }

    let per = 4;
    let constraints: Vec<Constraint> = (0..app.apps.len())
        .map(|_| Constraint::Length(per as u16))
        .collect();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    for (i, a) in app.apps.iter().enumerate() {
        let w = chunks[i].width as usize;
        let lines = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1); 4])
            .split(chunks[i]);
        let name = a.device_path.menu_entry();
        let (mem_label, mem_pct, mi) = gpu_mem_info(a);
        let _ = mi;
        let gfx = a.stat.activity.gfx.unwrap_or(0) as f64;

        // line 1: name + bus
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    format!(" {} ", short_name(&name)),
                    Style::default().fg(app.theme.hi_fg()).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("({}) ", a.device_path.pci.bus),
                    Style::default().fg(app.theme.graph_text()),
                ),
                Span::styled(
                    if a.device_info.is_apu { "APU".to_string() } else { "dGPU".to_string() },
                    Style::default().fg(app.theme.proc_misc()),
                ),
            ])),
            lines[0],
        );
        // line 2: GPU gauge + sparkline
        let mut l = gauge_line(app, "GFX", Some(gfx), w.min(40), Kind::Gpu);
        if w > 44 {
            let sp = app.hist_gpu[i].sparkline(w - 44, app.theme.cpu().sample(0.5));
            l.spans.push(Span::raw("  "));
            l.spans.extend(sp.spans);
        }
        f.render_widget(Paragraph::new(l), lines[1]);
        // line 3: MEM gauge
        f.render_widget(
            Paragraph::new(gauge_line(app, &mem_label, Some(mem_pct), w.min(40), Kind::Mem)),
            lines[2],
        );
        // line 4: stats
        f.render_widget(Paragraph::new(gpu_stats_line(a, &app.theme)), lines[3]);
    }
}

fn gpu_stats_line(a: &libamdgpu_top::app::AppAmdgpuTop, theme: &crate::theme::Theme) -> Line<'static> {
    let st = &a.stat;
    let s = st.sensors.as_ref();
    let temp = s.and_then(|x| x.junction_temp.as_ref().or(x.edge_temp.as_ref()));
    let temp_s = temp.map_or("  -".into(), |t| format!("{:>3}°C", t.current));
    let (pwr, sclk, mclk, fan) = if let Some(s) = s {
        let pw = s.average_power.as_ref().map(|p| p.value);
        let cap = s.power_cap.as_ref().map(|c| c.current);
        let pw = match (pw, cap) {
            (Some(p), Some(c)) => format!("{p}/{c}W"),
            (Some(p), None) => format!("{p}W"),
            _ => "  -".into(),
        };
        let sclk = s.sclk.map_or("    -".into(), |v| format!("{:>4}M", v));
        let mclk = s.mclk.map_or("    -".into(), |v| format!("{:>4}M", v));
        let fan = s.fan_rpm.map_or("    -".into(), |v| format!("{:>4}r", v));
        (pw, sclk, mclk, fan)
    } else {
        ("  -".into(), "    -".into(), "    -".into(), "    -".into())
    };
    Line::from(vec![
        Span::styled(format!(" {} ", temp_s), Style::default().fg(theme.temp().sample(0.5))),
        Span::styled(format!(" {} ", pwr), Style::default().fg(theme.graph_text())),
        Span::styled(format!(" sclk {} mclk {} ", sclk, mclk), Style::default().fg(theme.proc_misc())),
        Span::styled(format!("fan {}", fan), Style::default().fg(theme.graph_text())),
    ])
}

fn short_name(s: &str) -> String {
    // collapse long marketing names
    s.replace("AMD Radeon Graphics", "Radeon")
        .replace("AMD Radeon", "Radeon")
        .chars()
        .take(28)
        .collect()
}

// ---------- NPU ----------

fn draw_npu(f: &mut Frame, area: Rect, app: &mut App) {
    let block = section_block(app, Section::Npu, "NPU", SectionBox::Net);
    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.is_collapsed(Section::Npu) {
        let npu_pct = app.hist_npu.buf_last() as f64;
        let line = Line::from(vec![
            Span::styled(
                format!(" {:>3}% ", npu_pct.round()),
                Style::default().fg(app.theme.util_color(npu_pct, crate::theme::UtilKind::Npu)),
            ),
            Span::styled(
                format!("{} contexts  ", npu_ctx_count(app)),
                Style::default().fg(app.theme.graph_text()),
            ),
        ]);
        f.render_widget(Paragraph::new(line), inner);
        return;
    }

    let w = inner.width as usize;
    let top = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(0)])
        .split(inner);

    // header + aggregate gauge
    let hdr = Line::from(vec![
        Span::styled(
            " XDNA NPU ",
            Style::default().fg(app.theme.hi_fg()).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(
                "fw {}  ",
                app.apps
                    .iter()
                    .find_map(|a| a.xdna_fw_version.clone())
                    .unwrap_or_default()
            ),
            Style::default().fg(app.theme.graph_text()),
        ),
    ]);
    f.render_widget(Paragraph::new(hdr), top[0]);
    let npu_pct = app.hist_npu.buf_last() as f64;
    f.render_widget(
        Paragraph::new(gauge_line(app, "NPU", Some(npu_pct), w.min(40), Kind::Npu)),
        top[0],
    );

    // contexts table
    let header = Row::new(vec!["PID", "NAME", "CTX", "MEM", "NPU%"])
        .style(Style::default().fg(app.theme.proc_misc()).add_modifier(Modifier::BOLD));
    let rows: Vec<Row> = app
        .apps
        .iter()
        .flat_map(|a| a.stat.xdna_fdinfo.proc_usage.iter())
        .map(|pu| {
            Row::new(vec![
                format!("{}", pu.pid),
                pu.name.chars().take(24).collect::<String>(),
                format!("{}", pu.ids_count),
                format!("{}K", pu.usage.total_memory),
                format!("{}", pu.usage.npu),
            ])
        })
        .collect();
    let table = Table::new(
        rows,
        [Constraint::Length(8), Constraint::Min(20), Constraint::Length(5), Constraint::Length(10), Constraint::Length(6)],
    )
    .header(header);
    f.render_widget(table, top[1]);
}

fn npu_ctx_count(app: &App) -> usize {
    app.apps
        .iter()
        .map(|a| a.stat.xdna_fdinfo.proc_usage.len())
        .sum()
}

// ---------- Processes ----------

fn draw_processes(f: &mut Frame, area: Rect, app: &mut App) {
    let block = section_block(app, Section::Processes, "PROCESSES", SectionBox::Proc);
    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.is_collapsed(Section::Processes) {
        let n: usize = app
            .apps
            .iter()
            .map(|a| a.stat.fdinfo.proc_usage.len())
            .sum();
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!(" {n} processes "),
                Style::default().fg(app.theme.graph_text()),
            ))),
            inner,
        );
        return;
    }

    let header = Row::new(vec!["GPU", "PID", "NAME", "VRAM", "GTT", "GFX%", "COMP%", "DMA%", "CPU%"])
        .style(Style::default().fg(app.theme.proc_misc()).add_modifier(Modifier::BOLD));
    let rows: Vec<Row> = app
        .apps
        .iter()
        .flat_map(|a| {
            let bus = a.device_path.pci.bus;
            a.stat.fdinfo.proc_usage.iter().map(move |pu| {
                Row::new(vec![
                    format!("{bus}"),
                    format!("{}", pu.pid),
                    pu.name.chars().take(24).collect::<String>(),
                    format!("{}M", pu.usage.vram_usage >> 10),
                    format!("{}M", pu.usage.gtt_usage >> 10),
                    format!("{}", pu.usage.gfx),
                    format!("{}", pu.usage.compute),
                    format!("{}", pu.usage.dma),
                    format!("{}", pu.usage.cpu),
                ])
            })
        })
        .collect();
    let table = Table::new(
        rows,
        [
            Constraint::Length(4),
            Constraint::Length(7),
            Constraint::Min(20),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(6),
            Constraint::Length(7),
            Constraint::Length(6),
            Constraint::Length(6),
        ],
    )
    .header(header);
    f.render_widget(table, inner);
}

// ---------- footer ----------

fn draw_footer(f: &mut Frame, area: Rect, app: &App) {
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            " tab: next section · space: collapse/expand · q: quit ",
            Style::default().fg(app.theme.inactive_fg()),
        ))),
        area,
    );
}
