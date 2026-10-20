//! Terminal UI for `voxtype configure`.
//!
//! A section list on the left, the open section on the right, a header rule
//! naming the config file and daemon, and a footer of keys for whatever has
//! focus. See `docs/TUI_DESIGN.md` for the design this follows.

mod advanced_section;
mod app;
mod audio;
mod chrome;
mod common;
mod compositor_bindings;
mod config_editor;
mod engine;
mod general;
mod hotkey;
mod meeting_section;
mod notifications_section;
mod osd_section;
mod output_section;
mod section;
mod sidebar;
mod text_section;
mod theme;
mod vad_section;
mod waybar_section;

// Exported beyond the crate so the binary's `voxtype config` handlers write
// through the same atomic-save-and-validate path the TUI uses, rather than
// growing a second config writer.
#[allow(unused_imports)]
pub use config_editor::{ConfigEditor, EditorError, OverlayChange};

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
        MouseButton, MouseEvent, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    Frame, Terminal,
};
use std::io::{self, Stdout};
use std::time::Duration;

use app::{Action, App};
use section::Section;

type Tui = Terminal<CrosstermBackend<Stdout>>;

/// Entry point for the hidden `voxtype configure --probe-audio-devices`
/// flag: print detected input devices and exit. See
/// `audio::start_device_scan` for why this runs in its own process.
pub fn probe_audio_devices() {
    audio::print_input_devices();
}

/// Launch the TUI.
///
/// `config_path` is the `-c/--config` value; when set, every section reads and
/// writes that file instead of the default. Installed before the first section
/// loads, because a section that has already read the default would save back
/// to it (#595).
pub fn run(
    force_package_mode: bool,
    config_path: Option<std::path::PathBuf>,
) -> anyhow::Result<()> {
    config_editor::set_tui_config_path(config_path);
    let mut terminal = enter_terminal()?;
    let result = event_loop(&mut terminal, force_package_mode);
    leave_terminal(&mut terminal)?;
    if result? {
        // Printed after the alternate screen is torn down so it lands in
        // the user's shell, not the vanished TUI.
        println!("Restarted the voxtype daemon to apply saved config changes.");
    }
    Ok(())
}

fn enter_terminal() -> anyhow::Result<Tui> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    Ok(Terminal::new(CrosstermBackend::new(stdout))?)
}

fn leave_terminal(terminal: &mut Tui) -> anyhow::Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

/// Returns `true` when the daemon was restarted on the way out so `run()`
/// can tell the user once the terminal is back to normal.
fn event_loop(terminal: &mut Tui, force_package_mode: bool) -> anyhow::Result<bool> {
    let mut app = App::new(force_package_mode);
    let mut last_general_refresh = std::time::Instant::now();
    let general_refresh_interval = Duration::from_secs(2);

    loop {
        app.poll_background();
        terminal.draw(|f| draw(f, &app))?;

        if !event::poll(Duration::from_millis(250))? {
            // Idle tick. Refresh the General-screen state (daemon status,
            // active variant, inventory) so the green/red dot stays current
            // without the user pressing `r`.
            if app.current_section == Section::General
                && last_general_refresh.elapsed() >= general_refresh_interval
            {
                app.refresh_inventory();
                last_general_refresh = std::time::Instant::now();
            }
            continue;
        }
        match event::read()? {
            Event::Key(key) => {
                if !matches!(
                    key.kind,
                    crossterm::event::KeyEventKind::Press | crossterm::event::KeyEventKind::Repeat
                ) {
                    continue;
                }

                // Global shortcuts handled before delegating to the focused pane.
                if let Some(action) = handle_global_key(&mut app, key) {
                    match dispatch_action(terminal, &mut app, action)? {
                        LoopControl::Continue => continue,
                        LoopControl::Quit => return Ok(restart_if_config_newer(&app)),
                    }
                }

                let action = if app.sidebar_focused {
                    handle_sidebar_key(&mut app, key)
                } else {
                    handle_section_key(&mut app, key)
                };

                match dispatch_action(terminal, &mut app, action)? {
                    LoopControl::Continue => {}
                    LoopControl::Quit => return Ok(restart_if_config_newer(&app)),
                }
            }
            Event::Mouse(mouse) => {
                let size = terminal.size()?;
                handle_mouse(&mut app, mouse, Rect::new(0, 0, size.width, size.height));
            }
            _ => {}
        }
    }
}

enum LoopControl {
    Continue,
    Quit,
}

fn dispatch_action(
    terminal: &mut Tui,
    app: &mut App,
    action: Action,
) -> anyhow::Result<LoopControl> {
    match action {
        // Quit asks first only when some section holds unsaved edits, and
        // names them. Browsing, or quitting after saving, exits at once.
        Action::Quit => {
            if !app.dirty_sections().is_empty() {
                app.quit_pending = true;
                Ok(LoopControl::Continue)
            } else {
                Ok(LoopControl::Quit)
            }
        }
        // ForceQuit is the prompt's resolved-decision exit: the user already
        // chose Save or Discard, so skip the dirty-sections re-check that
        // would otherwise re-open the prompt and loop forever.
        Action::ForceQuit => Ok(LoopControl::Quit),
        Action::SwitchVariant(variant) => {
            // Drop out of the alternate screen so pkexec can prompt.
            leave_terminal(terminal)?;
            let outcome = run_pkexec_switch(variant);
            *terminal = enter_terminal()?;
            terminal.clear()?;
            app.record_switch_attempt(variant, outcome);
            restart_voxtype_daemon(app);
            Ok(LoopControl::Continue)
        }
        Action::DownloadModel { engine, model } => {
            leave_terminal(terminal)?;
            let outcome = run_setup_model(&engine, &model);
            *terminal = enter_terminal()?;
            terminal.clear()?;
            app.record_download_attempt(&engine, &model, outcome);
            restart_voxtype_daemon(app);
            Ok(LoopControl::Continue)
        }
        Action::SwitchVariantAndDownload {
            variant,
            engine,
            model,
        } => {
            leave_terminal(terminal)?;
            let switch_outcome = run_pkexec_switch(variant);
            // Always attempt the download even if the switch failed: the
            // download writes to ~/.local/share/voxtype/models, no symlink
            // needed. The user can retry the switch from General afterwards.
            let download_outcome = run_setup_model(&engine, &model);
            *terminal = enter_terminal()?;
            terminal.clear()?;
            app.record_switch_attempt(variant, switch_outcome);
            app.record_download_attempt(&engine, &model, download_outcome);
            restart_voxtype_daemon(app);
            Ok(LoopControl::Continue)
        }
        Action::None => Ok(LoopControl::Continue),
    }
}

/// Download a model in the parent shell so the user sees the progress. We
/// invoke whichever voxtype is on PATH because that's the user's installed
/// binary; the TUI's own image might be an uninstalled host build that
/// doesn't have the engine feature flags.
///
/// `setup model` takes no positional argument (see `SetupAction::Model` in
/// `src/cli/setup.rs`) — the download entry point is the top-level
/// `setup --download --model <NAME>`. Passing the name positionally made
/// clap reject the command before it did anything.
fn run_setup_model(engine: &str, model: &str) -> Result<(), String> {
    let _ = engine;
    let status = std::process::Command::new("voxtype")
        .args(["setup", "--download", "--model", model])
        .status()
        .map_err(|e| format!("could not invoke `voxtype setup --download`: {}", e))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("voxtype setup --download exited with {}", status))
    }
}

/// Best-effort `systemctl --user restart voxtype` after a binary swap or
/// model download so the changes take effect immediately. Logs the outcome
/// to the General switch-outcome banner so the user sees it on next focus.
fn restart_voxtype_daemon(app: &mut App) {
    let _ = std::process::Command::new("systemctl")
        .args(["--user", "restart", "voxtype"])
        .status();
    // The freshly (re)started daemon has read the config as it stands now,
    // so quitting doesn't need another restart unless a later save lands.
    app.config_synced_mtime = app::config_file_mtime();
    app.refresh_inventory();
}

/// Restart the daemon on the way out of the TUI when a save landed after
/// the daemon's last known (re)start, so config-only changes (an engine
/// switch on a binary that supports both, a hotkey edit, ...) take effect
/// without a manual `systemctl --user restart voxtype`. The decision logic
/// lives in `app::should_restart_on_quit`; see its doc for the cases.
fn restart_if_config_newer(app: &App) -> bool {
    let restart = app::should_restart_on_quit(
        crate::daemon_status::is_daemon_running(),
        app.config_synced_mtime,
        app::config_file_mtime(),
    );
    if restart {
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "restart", "voxtype"])
            .status();
    }
    restart
}

/// Routes keypresses while the save-on-exit prompt is showing.
///
/// - `s` / Enter save every section with unsaved edits, then quit. A save
///   that fails keeps the TUI open on that section, where its status line
///   shows the error, rather than quitting with the edit lost.
/// - `d` discard: quit without saving.
/// - `c` or `Esc` cancel: dismiss the prompt and stay in the TUI.
/// - anything else is swallowed.
fn handle_quit_prompt_key(app: &mut App, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Char('s') | KeyCode::Char('S') | KeyCode::Enter => {
            app.quit_pending = false;
            let failed = app.save_dirty_sections();
            match failed.first() {
                None => Action::ForceQuit,
                Some(section) => {
                    app.jump_to_section(*section);
                    Action::None
                }
            }
        }
        KeyCode::Char('d') | KeyCode::Char('D') => {
            app.quit_pending = false;
            Action::ForceQuit
        }
        KeyCode::Char('c') | KeyCode::Char('C') | KeyCode::Esc => {
            app.quit_pending = false;
            Action::None
        }
        _ => Action::None,
    }
}

/// Keys while the help overlay is open: arrows and paging scroll it, any
/// other key closes it.
fn handle_help_key(app: &mut App, key: KeyEvent) {
    let scroll = app.help_scroll.get();
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => app.help_scroll.set(scroll.saturating_add(1)),
        KeyCode::Up | KeyCode::Char('k') => app.help_scroll.set(scroll.saturating_sub(1)),
        KeyCode::PageDown | KeyCode::Char(' ') => app.help_scroll.set(scroll.saturating_add(10)),
        KeyCode::PageUp => app.help_scroll.set(scroll.saturating_sub(10)),
        _ => app.help_open = false,
    }
}

fn handle_global_key(app: &mut App, key: KeyEvent) -> Option<Action> {
    // While the active section is inline-editing a text field, swallow
    // global shortcuts so the user can type 'q', press Esc, etc. into the
    // input. The section's handle_key gets the key instead.
    if app.is_editing() {
        return None;
    }

    // Save-on-exit prompt: takes precedence over every other key. Only
    // recognized keys do anything; everything else is swallowed so the
    // user can't accidentally interact with the obscured TUI.
    if app.quit_pending {
        return Some(handle_quit_prompt_key(app, key));
    }

    if app.help_open {
        handle_help_key(app, key);
        return Some(Action::None);
    }
    if matches!(key.code, KeyCode::Char('?')) {
        app.help_open = true;
        app.help_scroll.set(0);
        return Some(Action::None);
    }

    match (key.code, key.modifiers) {
        (KeyCode::Char('q'), KeyModifiers::NONE) => Some(Action::Quit),
        (KeyCode::Char('c'), m) if m.contains(KeyModifiers::CONTROL) => Some(Action::Quit),
        // F2 jumps to the General section, where the variant-matrix picker
        // lives. Mentioned in the variant-mismatch banner so a user landing
        // on (say) Audio can fix the engine/binary mismatch without
        // navigating the sidebar by hand. See #450.
        (KeyCode::F(2), _) => {
            app.jump_to_section(Section::General);
            Some(Action::None)
        }
        (KeyCode::PageDown, _) => {
            let s = app.guidance_scroll.get();
            app.guidance_scroll.set(s.saturating_add(5));
            Some(Action::None)
        }
        (KeyCode::PageUp, _) => {
            let s = app.guidance_scroll.get();
            app.guidance_scroll.set(s.saturating_sub(5));
            Some(Action::None)
        }
        (KeyCode::Tab, _) | (KeyCode::BackTab, _) => {
            if app.sidebar_focused {
                app.focus_content();
            } else {
                app.focus_sidebar();
            }
            Some(Action::None)
        }
        (KeyCode::Esc, _) => {
            if !app.sidebar_focused {
                // First Esc returns focus to sidebar, second quits.
                app.focus_sidebar();
                Some(Action::None)
            } else {
                Some(Action::Quit)
            }
        }
        _ => None,
    }
}

fn handle_sidebar_key(app: &mut App, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            app.move_sidebar(-1);
            app.open_hovered_section();
            app.guidance_scroll.set(0);
            Action::None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.move_sidebar(1);
            app.open_hovered_section();
            app.guidance_scroll.set(0);
            Action::None
        }
        KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => {
            app.open_hovered_section();
            app.focus_content();
            Action::None
        }
        _ => Action::None,
    }
}

/// Screen regions, computed once so drawing and mouse hit-testing agree.
struct ScreenLayout {
    header: Rect,
    banner: Rect,
    sidebar: Rect,
    content: Rect,
    footer: Rect,
}

fn screen_layout(area: Rect, app: &App) -> ScreenLayout {
    // The variant-mismatch banner sits between the header and the body so
    // it stays visible on every section, not only General. See #450.
    let banner_height = if app.variant_mismatch.is_some() { 2 } else { 0 };
    let [header, banner, body, footer] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(banner_height),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .areas(area);
    let [sidebar, content] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(sidebar::WIDTH), Constraint::Min(0)])
        .areas(body);
    ScreenLayout {
        header,
        banner,
        sidebar,
        content,
        footer,
    }
}

fn handle_mouse(app: &mut App, mouse: MouseEvent, screen: Rect) {
    // Ignore mouse input while an overlay is open or a text field is editing.
    if app.help_open || app.quit_pending || app.is_editing() {
        return;
    }
    if !matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
        return;
    }
    let layout = screen_layout(screen, app);
    let (col, row) = (mouse.column, mouse.row);
    let inside = |r: Rect| col >= r.x && col < r.x + r.width && row >= r.y && row < r.y + r.height;

    if inside(layout.sidebar) {
        // Inner rows start one below the pane's top border.
        let inner_height = layout.sidebar.height.saturating_sub(2);
        let line = row.saturating_sub(layout.sidebar.y + 1);
        if row > layout.sidebar.y {
            if let Some(idx) = sidebar::item_at(inner_height, sidebar::focus_index(app), line) {
                app.sidebar_cursor = idx;
                app.open_hovered_section();
                app.guidance_scroll.set(0);
            }
        }
        app.focus_sidebar();
        return;
    }
    if inside(layout.content) {
        app.focus_content();
    }
}

fn handle_section_key(app: &mut App, key: KeyEvent) -> Action {
    // Moving to another setting starts its explanation from the top.
    if !app.is_editing()
        && matches!(
            key.code,
            KeyCode::Up | KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('k')
        )
    {
        app.guidance_scroll.set(0);
    }
    match app.current_section {
        Section::General => general::handle_key(app, key),
        Section::Hotkey => hotkey::handle_key(app, key),
        Section::Audio => audio::handle_key(app, key),
        Section::Engine => engine::handle_key(app, key),
        Section::Output => output_section::handle_key(app, key),
        Section::Text => text_section::handle_key(app, key),
        Section::Vad => vad_section::handle_key(app, key),
        Section::Meeting => meeting_section::handle_key(app, key),
        Section::Notifications => notifications_section::handle_key(app, key),
        Section::Osd => osd_section::handle_key(app, key),
        Section::Waybar => waybar_section::handle_key(app, key),
        Section::Advanced => advanced_section::handle_key(app, key),
    }
}

fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    if area.width < chrome::MIN_WIDTH || area.height < chrome::MIN_HEIGHT {
        chrome::render_too_small(f, area);
        return;
    }
    let layout = screen_layout(area, app);

    chrome::render_header(f, layout.header);
    if app.variant_mismatch.is_some() {
        chrome::render_variant_mismatch_banner(f, layout.banner, app);
    }
    sidebar::render(f, layout.sidebar, app);
    render_section(f, layout.content, app);
    chrome::render_footer(f, layout.footer, app);

    if app.help_open {
        chrome::render_help_overlay(f, app);
    }
    if app.quit_pending {
        chrome::render_quit_prompt(f, app);
    }
}

fn render_section(f: &mut Frame, area: Rect, app: &App) {
    match app.current_section {
        Section::General => general::render(f, area, app),
        Section::Hotkey => hotkey::render(f, area, app),
        Section::Audio => audio::render(f, area, app),
        Section::Engine => engine::render(f, area, app),
        Section::Output => output_section::render(f, area, app),
        Section::Text => text_section::render(f, area, app),
        Section::Vad => vad_section::render(f, area, app),
        Section::Meeting => meeting_section::render(f, area, app),
        Section::Notifications => notifications_section::render(f, area, app),
        Section::Osd => osd_section::render(f, area, app),
        Section::Waybar => waybar_section::render(f, area, app),
        Section::Advanced => advanced_section::render(f, area, app),
    }
}

fn run_pkexec_switch(variant: crate::setup::binary::Variant) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {}", e))?;
    let status = std::process::Command::new("pkexec")
        .arg(exe)
        .arg("setup")
        .arg("variant")
        .arg("--to")
        .arg(variant.binary_name())
        .status()
        .map_err(|e| format!("failed to launch pkexec: {} (is polkit installed?)", e))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("pkexec exited with {}", status))
    }
}
