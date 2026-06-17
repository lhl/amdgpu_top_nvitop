// amdgpu_top_nvitop — nvitop-style TUI frontend for libamdgpu_top.
//
// Samples real telemetry via AppAmdgpuTop and renders a nvitop-style layout:
//   - top:    per-device summary (GPU%, MEM%, VRAM, TEMP, POWER, SCLK/MCLK, FAN)
//   - bottom: live process table aggregated across all devices (from fdinfo)

use std::io;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use libamdgpu_top::app::{AppAmdgpuTop, AppOption};
use libamdgpu_top::DevicePath;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Row, Table};
use ratatui::Terminal;

const TICK: Duration = Duration::from_millis(1000);

fn main() -> io::Result<()> {
    let mut device_paths = DevicePath::get_device_path_list();
    for dp in device_paths.iter_mut() {
        dp.fill_amdgpu_device_name();
    }

    let (mut apps, suspended) =
        AppAmdgpuTop::create_app_and_suspended_list(&device_paths, &AppOption::default());

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal, &mut apps, &suspended);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn run(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    apps: &mut [AppAmdgpuTop],
    suspended: &[DevicePath],
) -> io::Result<()> {
    loop {
        for app in apps.iter_mut() {
            app.update(TICK);
        }

        terminal.draw(|f| draw(f, apps, suspended))?;

        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(k) = event::read()? {
                match k.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    _ => {}
                }
            }
        }
    }
}

fn draw(f: &mut ratatui::Frame, apps: &[AppAmdgpuTop], _suspended: &[DevicePath]) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length((apps.len() as u16) + 4), // device table
            Constraint::Min(8),                           // process table
            Constraint::Length(1),                        // footer
        ])
        .split(f.area());

    // ---- Device summary (nvitop-style green header) ----
    let header = Row::new(vec![
        "GPU", "TYPE", "NAME", "GFX%", "MEM%", "VRAM", "TEMP", "POWER", "SCLK", "MCLK", "FAN",
    ])
    .style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD));

    let rows = apps.iter().map(|app| {
        let st = &app.stat;
        let dp = &app.device_path;
        let kind = if dp.is_xdna() { "NPU" } else { "GPU" };

        let gfx = st.activity.gfx.map_or("   -".into(), |v| format!("{:>4}", v));

        let (mem_pct, vram_str) = {
            let v = &st.vram_usage.0.vram;
            let used = v.heap_usage;
            let total = v.usable_heap_size.max(1);
            let pct = (used as f64 / total as f64 * 100.0) as u64;
            (format!("{:>4}", pct), format!("{}/{}M", used >> 20, total >> 20))
        };

        let temp = st.sensors.as_ref().and_then(|s| {
            s.junction_temp.as_ref().or(s.edge_temp.as_ref())
        });
        let temp_s = temp.map_or("  -".into(), |t| format!("{:>3}C", t.current));

        let (power_s, sclk_s, mclk_s, fan_s) = if let Some(s) = st.sensors.as_ref() {
            let pw = s.average_power.as_ref().map(|p| format!("{:>3}W", p.value));
            let cap = s.power_cap.as_ref().map(|c| c.current);
            let pw = match (pw, cap) {
                (Some(p), Some(c)) => format!("{}/{c}W", p),
                (Some(p), None) => p,
                _ => "  -".into(),
            };
            let sclk = s.sclk.map_or("    -".into(), |v| format!("{:>4}M", v));
            let mclk = s.mclk.map_or("    -".into(), |v| format!("{:>4}M", v));
            let fan = s.fan_rpm.map_or("    -".into(), |v| format!("{:>4}r", v));
            (pw, sclk, mclk, fan)
        } else {
            ("  -".into(), "    -".into(), "    -".into(), "    -".into())
        };

        Row::new(vec![
            format!("{}", dp.pci.bus),
            kind.to_string(),
            dp.menu_entry(),
            gfx,
            mem_pct,
            vram_str,
            temp_s,
            power_s,
            sclk_s,
            mclk_s,
            fan_s,
        ])
    });

    let dev_table = Table::new(
        rows,
        [
            Constraint::Length(4),
            Constraint::Length(4),
            Constraint::Min(20),
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Length(16),
            Constraint::Length(6),
            Constraint::Length(10),
            Constraint::Length(7),
            Constraint::Length(7),
            Constraint::Length(7),
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::ALL).title(" AMDGPU Devices "));
    f.render_widget(dev_table, chunks[0]);

    // ---- Process table (fdinfo, aggregated across devices) ----
    let proc_header = Row::new(vec![
        "GPU", "PID", "NAME", "VRAM", "GTT", "GFX%", "COMP%", "DMA%", "CPU%",
    ])
    .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));

    let mut proc_rows: Vec<Row> = Vec::new();
    for app in apps {
        let bus = app.device_path.pci.bus;
        for pu in &app.stat.fdinfo.proc_usage {
            proc_rows.push(Row::new(vec![
                format!("{bus}"),
                format!("{}", pu.pid),
                pu.name.chars().take(24).collect::<String>(),
                format!("{}M", pu.usage.vram_usage >> 10),
                format!("{}M", pu.usage.gtt_usage >> 10),
                format!("{}", pu.usage.gfx),
                format!("{}", pu.usage.compute),
                format!("{}", pu.usage.dma),
                format!("{}", pu.usage.cpu),
            ]));
        }
    }
    if proc_rows.is_empty() {
        proc_rows.push(Row::new(vec!["No processes using GPU memory".to_string()]));
    }

    let proc_table = Table::new(
        proc_rows,
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
    .header(proc_header)
    .block(Block::default().borders(Borders::ALL).title(" Processes "));
    f.render_widget(proc_table, chunks[1]);

    let footer = ratatui::widgets::Paragraph::new(" q: quit ")
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(footer, chunks[2]);
}
