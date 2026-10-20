//! Everything around the sections: the header rule, the warning banner, the
//! key footer, the help overlay and the unsaved-changes prompt.

use ratatui::{
    layout::{Alignment, Rect},
    style::Modifier,
    text::{Line, Span},
    widgets::{Clear, Paragraph, Wrap},
    Frame,
};

use super::app::App;
use super::common::{self, str_width, truncate};
use super::section::Section;
use super::theme;

/// Smallest terminal the layout is designed for. Below it the TUI asks for
/// more room instead of drawing a broken screen.
pub const MIN_WIDTH: u16 = 60;
pub const MIN_HEIGHT: u16 = 16;

/// Spacing between footer key groups. Plain space reads calmer than
/// separators, and the key names already carry the accent.
const FOOTER_GAP: &str = "   ";

pub fn render_too_small(f: &mut Frame, area: Rect) {
    let lines = vec![
        Line::from(Span::styled("Terminal too small", theme::heading())),
        Line::from(Span::styled(
            format!(
                "voxtype configure needs {}x{}; this is {}x{}.",
                MIN_WIDTH, MIN_HEIGHT, area.width, area.height
            ),
            theme::muted(),
        )),
        Line::from(Span::styled("q quits.", theme::muted())),
    ];
    let y = area.y + area.height.saturating_sub(lines.len() as u16) / 2;
    let rect = Rect {
        x: area.x,
        y,
        width: area.width,
        height: (lines.len() as u16).min(area.height),
    };
    f.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true }),
        rect,
    );
}

/// The top rule: what this is and which file it edits on the left, the
/// daemon that will act on the saved settings on the right. On narrow
/// terminals the daemon text shortens first, then the path goes.
pub fn render_header(f: &mut Frame, area: Rect) {
    let (daemon_full, daemon_short, daemon_style) = daemon_status();
    let line = header_line(
        area.width as usize,
        &common::display_config_path(),
        &[daemon_full, daemon_short],
        daemon_style,
    );
    f.render_widget(Paragraph::new(line), area);
}

/// Shortest room a path gets before the header drops it.
const HEADER_MIN_PATH: usize = 14;

fn header_line(
    width: usize,
    path: &str,
    daemon: &[String],
    daemon_style: ratatui::style::Style,
) -> Line<'static> {
    let title = "voxtype configure";
    let title_width = 2 + str_width(title) + 1; // "─ title "
    let right_width = |r: &str| str_width(r) + 3; // " right ─"

    // Prefer the whole path with the full daemon text, then the whole path
    // with the short one, then a shortened path, then no path.
    let whole_path = str_width(path).max(HEADER_MIN_PATH);
    let mut choice: Option<(&str, bool)> = None;
    'outer: for path_min in [Some(whole_path), Some(HEADER_MIN_PATH), None] {
        for right in daemon {
            let with_path = path_min.is_some();
            let path_need = path_min.map_or(0, |p| 2 + p + 1);
            if title_width + path_need + right_width(right) <= width {
                choice = Some((right.as_str(), with_path));
                break 'outer;
            }
        }
    }

    let mut spans = vec![
        Span::styled("─ ", theme::border()),
        Span::styled(title, theme::heading()),
        Span::raw(" "),
    ];
    let mut used = title_width;
    let (right, with_path) = choice.unwrap_or(("", false));
    let right_used = if right.is_empty() {
        0
    } else {
        right_width(right)
    };
    if with_path {
        let room = width.saturating_sub(used + 2 + 1 + right_used + 1);
        let shown = common::truncate_middle(path, room);
        used += 2 + str_width(&shown) + 1;
        spans.push(Span::styled("─ ", theme::border()));
        spans.push(Span::styled(shown, theme::muted()));
        spans.push(Span::raw(" "));
    }
    let fill = width.saturating_sub(used + right_used);
    spans.push(Span::styled("─".repeat(fill), theme::border()));
    if !right.is_empty() {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(right.to_string(), daemon_style));
        spans.push(Span::styled(" ─", theme::border()));
    }
    Line::from(spans)
}

/// The daemon's state for the header, in a full and a short form. The
/// version shown is the daemon's, not this TUI's: someone editing settings
/// wants to know what will act on them, and after an upgrade that was never
/// restarted those differ.
fn daemon_status() -> (String, String, ratatui::style::Style) {
    use crate::daemon_status::DaemonVersion;
    let daemon = crate::daemon_status::running_version();
    match &daemon {
        DaemonVersion::Running(v) if daemon.differs_from_caller() => (
            format!("! daemon {} · this TUI {}", v, crate::cli::VERSION),
            format!("! daemon {}", v),
            theme::warn(),
        ),
        DaemonVersion::Running(v) => (
            format!("● daemon {} running", v),
            "● daemon running".to_string(),
            theme::ok(),
        ),
        DaemonVersion::NotRunning => (
            "○ daemon stopped".to_string(),
            "○ stopped".to_string(),
            theme::muted(),
        ),
        DaemonVersion::Unknown => (
            "● daemon running".to_string(),
            "● running".to_string(),
            theme::ok(),
        ),
    }
}

/// Two-line warning shown on every section while the running binary cannot
/// serve the configured engine (#450), so it never hides behind navigation.
pub fn render_variant_mismatch_banner(f: &mut Frame, area: Rect, app: &App) {
    use crate::setup::variant_check::Remediation;
    let Some(m) = app.variant_mismatch.as_ref() else {
        return;
    };
    let width = area.width as usize;
    let active = m
        .active_variant_name
        .as_deref()
        .unwrap_or("the running binary");

    let line1 = format!(
        "engine = {}, but {} lacks --features {}",
        m.configured_engine, active, m.required_feature
    );
    let line2 = match &m.remediation {
        // The General page marks the recommended binary with ★.
        Remediation::SwitchToVariant { .. } => {
            "F2 to switch binary, or run: sudo voxtype setup onnx --enable".to_string()
        }
        Remediation::Rebuild { feature } => {
            format!("Rebuild voxtype with --features {}", feature)
        }
    };
    let lines = vec![
        Line::from(vec![
            Span::styled(" ! ", theme::warn().add_modifier(Modifier::BOLD)),
            Span::styled(truncate(&line1, width.saturating_sub(3)), theme::warn()),
        ]),
        Line::from(vec![
            Span::raw("   "),
            Span::styled(truncate(&line2, width.saturating_sub(3)), theme::muted()),
        ]),
    ];
    f.render_widget(Paragraph::new(lines), area);
}

/// Key hints for whatever has focus right now, most important first. The
/// footer drops hints from the end when the terminal is narrow.
pub fn footer_hints(app: &App) -> Vec<(&'static str, &'static str)> {
    if app.quit_pending {
        return vec![
            ("s", "save and quit"),
            ("d", "discard and quit"),
            ("Esc", "keep editing"),
        ];
    }
    if app.help_open {
        return vec![("↑↓", "scroll"), ("any key", "close")];
    }
    if app.is_editing() {
        return vec![
            ("Enter", "apply"),
            ("Esc", "cancel"),
            ("←→", "move"),
            ("Ctrl-U", "clear"),
            ("Ctrl-W", "delete word"),
        ];
    }
    if app.sidebar_focused {
        return vec![
            ("↑↓", "section"),
            ("Enter", "open"),
            ("Tab", "settings"),
            ("?", "keys"),
            ("q", "quit"),
        ];
    }
    if app.current_section == Section::General {
        return vec![
            ("↑↓←→", "variant"),
            ("Enter", "switch"),
            ("D", "start/restart daemon"),
            ("r", "refresh"),
            ("Tab", "sections"),
            ("?", "keys"),
            ("q", "quit"),
        ];
    }
    vec![
        ("↑↓", "setting"),
        ("←→", "change"),
        ("Enter", "edit"),
        ("s", "save"),
        ("r", "revert"),
        ("Tab", "sections"),
        ("PgDn", "about"),
        ("?", "keys"),
        ("q", "quit"),
    ]
}

/// The bottom line: key hints on the left, unsaved sections on the right.
pub fn render_footer(f: &mut Frame, area: Rect, app: &App) {
    let width = area.width as usize;
    let dirty = app.dirty_sections();
    let right = match dirty.len() {
        0 => String::new(),
        1 => format!("● {} unsaved ", dirty[0].label()),
        n => format!("● {} sections unsaved ", n),
    };
    let right_width = str_width(&right);

    let mut spans = vec![Span::raw(" ")];
    let mut used = 1;
    for (i, (key, desc)) in footer_hints(app).into_iter().enumerate() {
        let gap = if i == 0 { "" } else { FOOTER_GAP };
        let w = str_width(gap) + str_width(key) + 1 + str_width(desc);
        if used + w + right_width + 1 > width {
            break;
        }
        spans.push(Span::raw(gap));
        spans.push(Span::styled(key, theme::key()));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(desc, theme::muted()));
        used += w;
    }
    if !right.is_empty() && used + right_width <= width {
        spans.push(Span::raw(" ".repeat(width - used - right_width)));
        spans.push(Span::styled(right, theme::warn()));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

/// Asked on quit only when some section holds unsaved edits, naming them,
/// so browsing never ends in a question.
pub fn render_quit_prompt(f: &mut Frame, app: &App) {
    let dirty = app.dirty_sections();
    let names: Vec<&str> = dirty.iter().map(|s| s.label()).collect();
    let area = f.area();
    let width = 58.min(area.width.saturating_sub(4));
    let inner_width = width.saturating_sub(4) as usize;

    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            truncate(&format!("Unsaved: {}", names.join(", ")), inner_width),
            theme::warn(),
        )),
        Line::from(""),
        prompt_option("s", "Save and quit", inner_width),
        prompt_option("d", "Discard changes and quit", inner_width),
        prompt_option("Esc", "Keep editing", inner_width),
    ];
    lines.push(Line::from(""));
    let rect = centered(area, width, lines.len() as u16 + 2);
    f.render_widget(Clear, rect);
    let block = common::pane("Save before quitting?", true);
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    let inner = Rect {
        x: inner.x + 1,
        width: inner.width.saturating_sub(2),
        ..inner
    };
    f.render_widget(Paragraph::new(lines), inner);
}

fn prompt_option(key: &str, desc: &str, width: usize) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{:<5}", key), theme::key()),
        Span::raw(truncate(desc, width.saturating_sub(5))),
    ])
}

fn help_lines() -> Vec<Line<'static>> {
    let group = |title: &str| Line::from(Span::styled(title.to_string(), theme::heading()));
    let entry = |keys: &str, desc: &str| {
        Line::from(vec![
            Span::raw("  "),
            Span::styled(format!("{:<14}", keys), theme::key()),
            Span::raw(desc.to_string()),
        ])
    };
    let mut lines = vec![
        group("Anywhere"),
        entry("Tab", "Move between the section list and the open section"),
        entry("Esc", "Back to the section list; from there, quit"),
        entry("q, Ctrl-C", "Quit (asks first if anything is unsaved)"),
        entry("?", "Show these keys"),
        entry("F2", "Jump to General to pick a binary variant"),
        entry("PgUp PgDn", "Scroll the About pane"),
        Line::from(""),
        group("Section list"),
        entry("↑↓ j k", "Choose a section; it opens as you move"),
        entry("Enter → l", "Move into the section"),
        Line::from(""),
        group("Settings"),
        entry("↑↓ j k", "Choose a setting"),
        entry("←→ h l", "Change it to the previous or next value"),
        entry("Space", "Toggle or advance"),
        entry("Enter, i", "Type a value"),
        entry("s", "Save this section to the config file"),
        entry("r", "Revert this section to what is saved"),
        Line::from(""),
        group("Typing a value"),
        entry("Enter", "Apply"),
        entry("Esc, Ctrl-C", "Cancel"),
        entry("←→ Home End", "Move the cursor"),
        entry("Backspace Del", "Delete a character"),
        entry("Ctrl-W", "Delete the previous word"),
        entry("Ctrl-U", "Clear"),
        Line::from(""),
        group("General"),
        entry("↑↓←→ hjkl", "Choose a binary variant"),
        entry("Enter", "Switch to it (asks for your password)"),
        entry("D", "Start or restart the voxtype daemon"),
        entry("r", "Refresh"),
        Line::from(""),
        group("Markers"),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(format!("{:<14}", "●"), theme::warn()),
            Span::raw("Changed here, not saved yet"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(format!("{:<14}", "›"), theme::accent()),
            Span::raw("The open section"),
        ]),
        Line::from(""),
        group("Sections"),
    ];
    for section in Section::ALL {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(format!("{:<14}", section.label()), theme::text()),
            Span::styled(section.summary().to_string(), theme::muted()),
        ]));
    }
    lines
}

pub fn render_help_overlay(f: &mut Frame, app: &App) {
    let area = f.area();
    let lines = help_lines();
    let width = 80.min(area.width.saturating_sub(4));
    let height = (lines.len() as u16 + 2).min(area.height.saturating_sub(2));
    let rect = centered(area, width, height);
    f.render_widget(Clear, rect);

    let mut block = common::pane("Keys", true);
    let inner = block.inner(rect);
    let max_scroll = (lines.len() as u16).saturating_sub(inner.height);
    let scroll = app.help_scroll.get().min(max_scroll);
    app.help_scroll.set(scroll);
    if max_scroll > 0 {
        let hint = if scroll < max_scroll {
            " ↓ more "
        } else {
            " ↑ back "
        };
        block = block.title_bottom(Line::from(Span::styled(hint, theme::muted())).right_aligned());
    }
    f.render_widget(block, rect);
    let inner = Rect {
        x: inner.x + 1,
        width: inner.width.saturating_sub(2),
        ..inner
    };
    f.render_widget(Paragraph::new(lines).scroll((scroll, 0)), inner);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(line: &Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    fn daemon() -> Vec<String> {
        vec![
            "! daemon 1.1.0 · this TUI 1.1.1".to_string(),
            "! daemon 1.1.0".to_string(),
        ]
    }

    #[test]
    fn header_fills_the_width_exactly() {
        for width in [60, 80, 100, 140, 200] {
            let line = header_line(
                width,
                "~/.config/voxtype/config.toml",
                &daemon(),
                theme::warn(),
            );
            assert_eq!(
                str_width(&text(&line)),
                width,
                "width {width}: {:?}",
                text(&line)
            );
        }
    }

    #[test]
    fn header_shortens_the_daemon_before_dropping_the_path() {
        let path = "~/.config/voxtype/config.toml";
        let wide = text(&header_line(120, path, &daemon(), theme::warn()));
        assert!(wide.contains(path) && wide.contains("this TUI"), "{wide}");
        let medium = text(&header_line(66, path, &daemon(), theme::warn()));
        assert!(medium.contains("! daemon 1.1.0 ─"), "{medium}");
        assert!(!medium.contains("this TUI"), "{medium}");
        assert!(medium.contains("~/"), "{medium}");
    }
}
