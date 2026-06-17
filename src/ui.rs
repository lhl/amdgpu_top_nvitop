//! Modern TUI rendering: rounded borders, btop-themed gradients, braille
//! history graphs, collapsible CPU/GPU/NPU/Processes sections.
//!
//! GPU layout (nvitop/nvtop-inspired): one horizontal band per GPU.
//!   left column  -> identity + stats (index, name, bus-id, temp/pwr/clk/fan)
//!   right column -> GFX gauge, MEM/GTT gauge, braille history
//! Multi-GPU friendly: bands stack vertically.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::symbols::border;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table};
use ratatui::Frame;

use crate::app::{gpu_mem_info, App, Section};
use crate::gauge::{self, Kind};
use crate::theme::{SectionBox, UtilKind};

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();
    if let Some(bg) = app.theme.main_bg() {
        f.render_widget(Block::default().style(Style::default().bg(bg)), area);
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(build_constraints(app))
        .split(area);

    let mut idx = 1;
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
    draw_footer(f, chunks[chunks.len() - 1], app);
}

fn build_constraints(app: &App) -> Vec<Constraint> {
    let mut c = vec![Constraint::Length(1)];
    c.push(section_height(app, Section::Cpu));
    c.push(section_height(app, Section::Gpu));
    if app.has_npu {
        c.push(section_height(app, Section::Npu));
    }
    c.push(section_height(app, Section::Processes));
    c.push(Constraint::Length(1));
    c
}

fn section_height(app: &App, s: Section) -> Constraint {
    if app.is_collapsed(s) {
        return Constraint::Length(3);
    }
    let inner = match s {
        Section::Cpu => {
            let (_, rows) = core_grid_dims(app.cpu.per_core_percent.len().max(1));
            4 + rows as u16
        }
        Section::Gpu => (4 * app.apps.len() as u16) + 1,
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

// ---------- shared ----------

fn section_block(app: &App, s: Section, title: &str, box_kind: SectionBox) -> Block<'static> {
    let focused = app.section == s;
    let indicator = if app.is_collapsed(s) { "▾" } else { "▸" };
    let title_span = Span::styled(
        format!(" {indicator} {title} "),
        Style::default()
            .fg(if focused { app.theme.hi_fg() } else { app.theme.title() })
            .add_modifier(Modifier::BOLD),
    );
    let border_color = if focused { app.theme.hi_fg() } else { app.theme.box_color(box_kind) };
    Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(border_color))
        .title(title_span)
}

fn gauge_line(app: &App, label: &str, pct: Option<f64>, width: usize, kind: Kind) -> Line<'static> {
    gauge::line(label, pct, width, kind, &app.theme)
}

fn bus_id(app: &libamdgpu_top::app::AppAmdgpuTop) -> String {
    format!("{}", app.device_path.pci)
}

// ---------- header ----------

fn draw_header(f: &mut Frame, area: Rect, app: &App) {
    let now = clock_string();
    let line = Line::from(vec![
        Span::styled(
            " amdgpu-top-nvitop ",
            Style::default().fg(app.theme.hi_fg()).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {} device{} ", app.apps.len(), if app.apps.len() == 1 { "" } else { "s" }),
            Style::default().fg(app.theme.graph_text()),
        ),
        Span::styled(
            format!(" {now} "),
            Style::default().fg(app.theme.graph_text()),
        ),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

fn clock_string() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (_wday, m, d, hh, mm, ss) = civil(secs);
    format!("{m:02} {d:02} {hh:02}:{mm:02}:{ss:02}")
}

fn civil(secs: u64) -> (u64, u64, u64, u64, u64, u64) {
    let days = secs / 86400;
    let rem = secs % 86400;
    let hh = rem / 3600;
    let mm = (rem % 3600) / 60;
    let ss = rem % 60;
    let z = days as i64 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let _y = if m <= 2 { y + 1 } else { y };
    let wday = (days + 4) % 7;
    (wday, m, d, hh, mm, ss)
}

// ---------- CPU ----------

fn draw_cpu(f: &mut Frame, area: Rect, app: &mut App) {
    let title = format!("CPU  {}", short_model(&app.cpu_model));
    let block = section_block(app, Section::Cpu, &title, SectionBox::Cpu);
    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.is_collapsed(Section::Cpu) {
        let line = Line::from(vec![
            Span::styled(
                format!(" {:>3.0}% ", app.cpu.cpu_percent.round()),
                Style::default().fg(app.theme.util_color(app.cpu.cpu_percent, UtilKind::Gpu)),
            ),
            Span::styled(
                format!("load {:.2} {:.2} {:.2}  ", app.mem.load1, app.mem.load5, app.mem.load15),
                Style::default().fg(app.theme.graph_text()),
            ),
            Span::styled(
                format!(
                    "MEM {:.1}G/{:.0}G  SWP {:.1}G/{:.0}G",
                    app.mem.mem_used_gb(),
                    app.mem.mem_total_gb(),
                    app.mem.swap_used_gb(),
                    app.mem.swap_total_gb()
                ),
                Style::default().fg(app.theme.proc_misc()),
            ),
        ]);
        f.render_widget(Paragraph::new(line), inner);
        return;
    }

    let w = inner.width as usize;
    let rl = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // CPU bar
            Constraint::Length(1), // stats
            Constraint::Length(1), // MEM
            Constraint::Length(1), // SWP
            Constraint::Min(1),    // per-core grid
        ])
        .split(inner);

    // CPU aggregate bar
    f.render_widget(
        Paragraph::new(gauge_line(app, "CPU", Some(app.cpu.cpu_percent), w, Kind::Gpu)),
        rl[0],
    );

    // stats: freq, temp, package power, load avg
    let tctl = app.apps.iter().find_map(|a| a.stat.sensors.as_ref().and_then(|s| s.tctl));
    let cores = app
        .apps
        .iter()
        .find_map(|a| a.stat.sensors.as_ref().map(|s| s.all_cpu_core_freq_info.clone()))
        .unwrap_or_default();
    let freq = cores.iter().map(|c| c.cur).max().unwrap_or(0);
    let pkg = app
        .apps
        .iter()
        .find_map(|a| a.stat.sensors.as_ref().and_then(|s| s.average_power.as_ref().map(|p| p.value)));
    let stats = Line::from(vec![
        Span::styled(
            format!(" {:.2} GHz ", freq as f64 / 1000.0),
            Style::default().fg(app.theme.proc_misc()),
        ),
        Span::styled(
            format!(" {} ", tctl.map(|t| format!("{}°C", t / 1000)).unwrap_or_else(|| "—".into())),
            Style::default().fg(app.theme.temp().sample(0.6)),
        ),
        Span::styled(
            format!(" {} ", pkg.map(|w| format!("{w}W")).unwrap_or_else(|| "—".into())),
            Style::default().fg(app.theme.graph_text()),
        ),
        Span::styled(
            format!(" load {:.2} {:.2} {:.2}", app.mem.load1, app.mem.load5, app.mem.load15),
            Style::default().fg(app.theme.graph_text()),
        ),
    ]);
    f.render_widget(Paragraph::new(stats), rl[1]);

    // MEM + SWP bars with absolute numbers
    let mem_ann = format!(
        "{} / {}  {:>3.0}%",
        fmt_gb(app.mem.mem_used_gb()),
        fmt_gb(app.mem.mem_total_gb()),
        app.mem.mem_used_pct()
    );
    f.render_widget(
        Paragraph::new(gauge::bar("MEM", Some(app.mem.mem_used_pct()), &mem_ann, w, Kind::Mem, &app.theme)),
        rl[2],
    );
    let swp_ann = format!(
        "{} / {}  {:>3.0}%",
        fmt_gb(app.mem.swap_used_gb()),
        fmt_gb(app.mem.swap_total_gb()),
        app.mem.swap_used_pct()
    );
    f.render_widget(
        Paragraph::new(gauge::bar("SWP", Some(app.mem.swap_used_pct()), &swp_ann, w, Kind::Mem, &app.theme)),
        rl[3],
    );

    // per-core grid (btop-style)
    draw_core_grid(f, rl[4], app);
}

/// Grid dimensions: aim for <= 8 rows, columns grow with core count.
fn core_grid_dims(n: usize) -> (usize, usize) {
    if n == 0 {
        return (1, 1);
    }
    let cols = ((n + 7) / 8).max(1);
    let rows = (n + cols - 1) / cols;
    (cols, rows)
}

fn draw_core_grid(f: &mut Frame, area: Rect, app: &App) {
    let n = app.cpu.per_core_percent.len();
    if n == 0 || area.height == 0 {
        return;
    }
    let (cols, rows) = core_grid_dims(n);
    let col_areas = Layout::default()
        .direction(Direction::Horizontal)
        .constraints((0..cols).map(|_| Constraint::Ratio(1, cols as u32)).collect::<Vec<_>>())
        .split(area);

    for c in 0..cols {
        let row_areas = Layout::default()
            .direction(Direction::Vertical)
            .constraints((0..rows).map(|_| Constraint::Length(1)).collect::<Vec<_>>())
            .split(col_areas[c]);
        for r in 0..rows {
            let idx = c * rows + r; // column-major like btop
            if idx >= n {
                continue;
            }
            let pct = app.cpu.per_core_percent[idx];
            let label = format!("C{idx}");
            let pct_s = format!("{:>3.0}%", pct);
            let cell_w = col_areas[c].width as usize;
            let graph_w = cell_w.saturating_sub(label.chars().count() + 2 + pct_s.chars().count() + 1).max(1);
            let mut spans: Vec<Span<'static>> = vec![Span::styled(
                format!("{label:>3} "),
                Style::default().fg(app.theme.graph_text()),
            )];
            if let Some(g) = app.hist_cores[idx].braille_graph(graph_w, 1, app.theme.cpu()).into_iter().next() {
                spans.extend(g.spans);
            }
            spans.push(Span::styled(
                format!(" {pct_s}"),
                Style::default().fg(app.theme.util_color(pct, UtilKind::Gpu)),
            ));
            f.render_widget(Paragraph::new(Line::from(spans)), row_areas[r]);
        }
    }
}

fn fmt_gb(gb: f64) -> String {
    format!("{gb:.1}G")
}

fn fmt_bytes(b: u64) -> String {
    const G: u64 = 1 << 30;
    const M: u64 = 1 << 20;
    if b >= G {
        format!("{:.1}G", b as f64 / G as f64)
    } else {
        format!("{}M", b / M)
    }
}

fn short_model(s: &str) -> String {
    let s = s.split(" w/").next().unwrap_or(s);
    s.replace("AMD ", "").replace("Processor", "").trim().to_string()
}

// ---------- GPU ----------

fn draw_gpu(f: &mut Frame, area: Rect, app: &mut App) {
    let block = section_block(app, Section::Gpu, "GPU", SectionBox::Mem);
    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.is_collapsed(Section::Gpu) {
        let mut spans: Vec<Span> = Vec::new();
        for (i, a) in app.apps.iter().enumerate() {
            let gfx = a.stat.activity.gfx.unwrap_or(0);
            let (_, mem_pct, _) = gpu_mem_info(a);
            spans.push(Span::styled(
                format!(" GPU{} {:>3}% gfx {:>3}% mem  ", i, gfx, mem_pct.round() as i64),
                Style::default().fg(app.theme.util_color(gfx as f64, UtilKind::Gpu)),
            ));
        }
        f.render_widget(Paragraph::new(Line::from(spans)), inner);
        return;
    }

    // each GPU = 4-row band
    let bands = Layout::default()
        .direction(Direction::Vertical)
        .constraints((0..app.apps.len()).map(|_| Constraint::Length(4)))
        .split(inner);

    for (i, a) in app.apps.iter().enumerate() {
        let band = bands[i];
        // left: identity+stats (38), right: gauges+history
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(38), Constraint::Min(20)])
            .split(band);

        let left = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1); 4])
            .split(cols[0]);

        // line 0: GPU index + name
        let name = a.device_path.device_name.clone();
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    format!(" GPU{} ", i),
                    Style::default().fg(app.theme.hi_fg()).add_modifier(Modifier::BOLD),
                ),
                Span::styled(short_name(&name), Style::default().fg(app.theme.title())),
            ])),
            left[0],
        );
        // line 1: bus-id + type
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    format!(" {} ", bus_id(a)),
                    Style::default().fg(app.theme.graph_text()),
                ),
                Span::styled(
                    if a.device_info.is_apu { "APU" } else { "dGPU" }.to_string(),
                    Style::default().fg(app.theme.proc_misc()),
                ),
            ])),
            left[1],
        );
        // line 2: temp + power
        let (temp_s, pwr_s) = gpu_temp_power(a);
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(format!(" {} ", temp_s), Style::default().fg(app.theme.temp().sample(0.5))),
                Span::styled(format!(" {} ", pwr_s), Style::default().fg(app.theme.graph_text())),
            ])),
            left[2],
        );
        // line 3: clocks + fan
        let (sclk_s, mclk_s, fan_s) = gpu_clocks_fan(a);
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(format!(" {} ", sclk_s), Style::default().fg(app.theme.proc_misc())),
                Span::styled(format!(" {} ", mclk_s), Style::default().fg(app.theme.proc_misc())),
                Span::styled(fan_s, Style::default().fg(app.theme.graph_text())),
            ])),
            left[3],
        );

        // right: GFX gauge, MEM gauge, braille history
        let right = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1), Constraint::Length(2)])
            .split(cols[1]);

        let gfx = a.stat.activity.gfx.unwrap_or(0) as f64;
        let (_, mem_pct, mi) = gpu_mem_info(a);
        let mem_label = mi.label.to_string();
        let rw = cols[1].width as usize;

        let gfx_ann = format!("{gfx:>3.0}%");
        f.render_widget(
            Paragraph::new(gauge::bar("GPU", Some(gfx), &gfx_ann, rw, Kind::Gpu, &app.theme)),
            right[0],
        );
        let mem_ann = format!(
            "{} / {}  {:>3.0}%",
            fmt_bytes(mi.used_bytes),
            fmt_bytes(mi.total_bytes),
            mem_pct
        );
        f.render_widget(
            Paragraph::new(gauge::bar(&mem_label, Some(mem_pct), &mem_ann, rw, Kind::Mem, &app.theme)),
            right[1],
        );

        // two history graphs: utilization (top, GPU gradient) and memory
        // (bottom, used gradient) so they're visually distinguishable and
        // line up under the GPU/MEM bars above.
        let grows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .split(right[2]);
        if let Some(g) = app.hist_gpu[i].braille_graph(rw, 1, app.theme.cpu()).into_iter().next() {
            f.render_widget(Paragraph::new(g), grows[0]);
        }
        if let Some(g) = app.hist_mem[i].braille_graph(rw, 1, app.theme.used()).into_iter().next() {
            f.render_widget(Paragraph::new(g), grows[1]);
        }
    }
}

fn gpu_temp_power(a: &libamdgpu_top::app::AppAmdgpuTop) -> (String, String) {
    let st = &a.stat;
    let s = st.sensors.as_ref();
    let temp = s.and_then(|x| x.junction_temp.as_ref().or(x.edge_temp.as_ref()));
    let temp_s = temp.map_or("  -".into(), |t| format!("{:>3}°C", t.current));
    let pwr_s = if let Some(s) = s {
        let pw = s.average_power.as_ref().map(|p| p.value);
        let cap = s.power_cap.as_ref().map(|c| c.current);
        match (pw, cap) {
            (Some(p), Some(c)) => format!("{p}/{c}W"),
            (Some(p), None) => format!("{p}W"),
            _ => "  -".into(),
        }
    } else {
        "  -".into()
    };
    (temp_s, pwr_s)
}

fn gpu_clocks_fan(a: &libamdgpu_top::app::AppAmdgpuTop) -> (String, String, String) {
    let s = match a.stat.sensors.as_ref() {
        Some(s) => s,
        None => return ("sclk -".into(), "mclk -".into(), "fan -".into()),
    };
    let sclk = s.sclk.map_or("sclk -".into(), |v| format!("sclk {}M", v));
    let mclk = s.mclk.map_or("mclk -".into(), |v| format!("mclk {}M", v));
    let fan = s.fan_rpm.map_or("fan -".into(), |v| format!("fan {}r", v));
    (sclk, mclk, fan)
}

fn short_name(s: &str) -> String {
    s.replace("AMD Radeon Graphics", "Radeon")
        .replace("AMD Radeon", "Radeon")
        .chars()
        .take(26)
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
                Style::default().fg(app.theme.util_color(npu_pct, UtilKind::Npu)),
            ),
            Span::styled(format!("{} contexts  ", npu_ctx_count(app)), Style::default().fg(app.theme.graph_text())),
        ]);
        f.render_widget(Paragraph::new(line), inner);
        return;
    }

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(38), Constraint::Min(20)])
        .split(inner);
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1), Constraint::Min(0)])
        .split(cols[0]);

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" NPU ", Style::default().fg(app.theme.hi_fg()).add_modifier(Modifier::BOLD)),
            Span::styled("XDNA", Style::default().fg(app.theme.title())),
        ])),
        left[0],
    );
    let fw = app.apps.iter().find_map(|a| a.xdna_fw_version.clone()).unwrap_or_default();
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" fw ", Style::default().fg(app.theme.graph_text())),
            Span::styled(fw, Style::default().fg(app.theme.proc_misc())),
        ])),
        left[1],
    );

    let npu_pct = app.hist_npu.buf_last() as f64;
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(2), Constraint::Min(0)])
        .split(cols[1]);
    f.render_widget(Paragraph::new(gauge_line(app, "NPU", Some(npu_pct), cols[1].width as usize, Kind::Npu)), right[0]);
    let graph = app.hist_npu.braille_graph(cols[1].width as usize, 2, app.theme.process());
    let grows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(right[1]);
    for (gi, line) in graph.iter().enumerate() {
        if gi < grows.len() {
            f.render_widget(Paragraph::new(line.clone()), grows[gi]);
        }
    }

    // contexts table below
    let table_area = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(inner)[1];
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
        [
            Constraint::Length(8),
            Constraint::Min(20),
            Constraint::Length(5),
            Constraint::Length(10),
            Constraint::Length(6),
        ],
    )
    .header(header);
    f.render_widget(table, table_area);
}

fn npu_ctx_count(app: &App) -> usize {
    app.apps.iter().map(|a| a.stat.xdna_fdinfo.proc_usage.len()).sum()
}

// ---------- Processes ----------

fn draw_processes(f: &mut Frame, area: Rect, app: &mut App) {
    let block = section_block(app, Section::Processes, "PROCESSES", SectionBox::Proc);
    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.is_collapsed(Section::Processes) {
        let n: usize = app.apps.iter().map(|a| a.stat.fdinfo.proc_usage.len()).sum();
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
        .enumerate()
        .flat_map(|(gi, a)| {
            a.stat.fdinfo.proc_usage.iter().map(move |pu| {
                Row::new(vec![
                    format!("{}", gi),
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
    let bg = app.theme.selected_bg();
    let key = Style::default().fg(app.theme.hi_fg()).add_modifier(Modifier::BOLD).bg(bg);
    let lbl = Style::default().fg(app.theme.main_fg()).bg(bg);
    let spans = vec![
        Span::styled(" q ", key),
        Span::styled("quit  ", lbl),
        Span::styled("tab ", key),
        Span::styled("section  ", lbl),
        Span::styled("space ", key),
        Span::styled("collapse  ", lbl),
        Span::styled("t/T ", key),
        Span::styled("theme ", lbl),
        Span::styled(
            format!(" {} ", app.theme_name),
            Style::default().fg(app.theme.selected_fg()).add_modifier(Modifier::BOLD).bg(bg),
        ),
    ];
    f.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(bg)),
        area,
    );
}
