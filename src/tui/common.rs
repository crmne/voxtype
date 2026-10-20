//! Shared rendering helpers for form-style sections (Hotkey, Audio, Output, …).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Padding, Paragraph, Wrap},
    Frame,
};

use super::app::App;
use super::theme;

/// Minimal single-line text input. Owned by a section's state when a free-text
/// field is being edited; sections check whether `editing` is `Some` and route
/// keys to [`TextInput::handle_key`] while it is.
#[derive(Debug, Clone)]
pub struct TextInput {
    buffer: String,
    /// Byte offset within `buffer`. Always lands on a UTF-8 char boundary.
    cursor: usize,
    /// Original value at the time editing started — used to detect "no change"
    /// and reportable on Cancel.
    #[allow(dead_code)]
    original: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextInputResult {
    /// Key was consumed but editing continues.
    Continue,
    /// User pressed Enter; commit `buffer()` to the underlying field.
    Commit,
    /// User pressed Esc; discard buffer.
    Cancel,
}

impl TextInput {
    pub fn new(initial: impl Into<String>) -> Self {
        let buffer: String = initial.into();
        let cursor = buffer.len();
        Self {
            original: buffer.clone(),
            buffer,
            cursor,
        }
    }

    pub fn buffer(&self) -> &str {
        &self.buffer
    }

    #[allow(dead_code)]
    pub fn changed(&self) -> bool {
        self.buffer != self.original
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> TextInputResult {
        match key.code {
            KeyCode::Enter => TextInputResult::Commit,
            KeyCode::Esc => TextInputResult::Cancel,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                TextInputResult::Cancel
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    let prev = prev_char_boundary(&self.buffer, self.cursor);
                    self.buffer.replace_range(prev..self.cursor, "");
                    self.cursor = prev;
                }
                TextInputResult::Continue
            }
            KeyCode::Delete => {
                if self.cursor < self.buffer.len() {
                    let next = next_char_boundary(&self.buffer, self.cursor);
                    self.buffer.replace_range(self.cursor..next, "");
                }
                TextInputResult::Continue
            }
            KeyCode::Left => {
                if self.cursor > 0 {
                    self.cursor = prev_char_boundary(&self.buffer, self.cursor);
                }
                TextInputResult::Continue
            }
            KeyCode::Right => {
                if self.cursor < self.buffer.len() {
                    self.cursor = next_char_boundary(&self.buffer, self.cursor);
                }
                TextInputResult::Continue
            }
            KeyCode::Home => {
                self.cursor = 0;
                TextInputResult::Continue
            }
            KeyCode::End => {
                self.cursor = self.buffer.len();
                TextInputResult::Continue
            }
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.cursor = 0;
                TextInputResult::Continue
            }
            KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.cursor = self.buffer.len();
                TextInputResult::Continue
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Clear the line.
                self.buffer.clear();
                self.cursor = 0;
                TextInputResult::Continue
            }
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Delete the previous word.
                let prev_word = prev_word_boundary(&self.buffer, self.cursor);
                self.buffer.replace_range(prev_word..self.cursor, "");
                self.cursor = prev_word;
                TextInputResult::Continue
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                let mut tmp = [0u8; 4];
                let s = c.encode_utf8(&mut tmp);
                self.buffer.insert_str(self.cursor, s);
                self.cursor += s.len();
                TextInputResult::Continue
            }
            _ => TextInputResult::Continue,
        }
    }

    /// Plain-text rendering of the buffer with a `│` caret inserted at the
    /// cursor position. Suitable for slotting into a form row's value column
    /// where we can't easily run multi-span styling.
    pub fn caret_string(&self) -> String {
        let mut out = String::with_capacity(self.buffer.len() + 1);
        out.push_str(&self.buffer[..self.cursor]);
        out.push(CARET);
        out.push_str(&self.buffer[self.cursor..]);
        out
    }
}

fn prev_char_boundary(s: &str, idx: usize) -> usize {
    let mut i = idx.saturating_sub(1);
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn next_char_boundary(s: &str, idx: usize) -> usize {
    let mut i = (idx + 1).min(s.len());
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

fn prev_word_boundary(s: &str, idx: usize) -> usize {
    let bytes = s.as_bytes();
    let mut i = idx;
    // Skip trailing spaces.
    while i > 0 && bytes[i - 1].is_ascii_whitespace() {
        i -= 1;
    }
    // Skip non-space characters.
    while i > 0 && !bytes[i - 1].is_ascii_whitespace() {
        i -= 1;
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEventKind, KeyEventState};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn ctrl(c: char) -> KeyEvent {
        KeyEvent {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    #[test]
    fn typing_appends_to_buffer() {
        let mut input = TextInput::new("");
        input.handle_key(key(KeyCode::Char('h')));
        input.handle_key(key(KeyCode::Char('i')));
        assert_eq!(input.buffer(), "hi");
    }

    #[test]
    fn backspace_deletes_prev_char() {
        let mut input = TextInput::new("hello");
        assert_eq!(
            input.handle_key(key(KeyCode::Backspace)),
            TextInputResult::Continue
        );
        assert_eq!(input.buffer(), "hell");
    }

    #[test]
    fn left_then_insert_inserts_mid_string() {
        let mut input = TextInput::new("ac");
        input.handle_key(key(KeyCode::Left));
        input.handle_key(key(KeyCode::Char('b')));
        assert_eq!(input.buffer(), "abc");
    }

    #[test]
    fn enter_signals_commit() {
        let mut input = TextInput::new("done");
        assert_eq!(
            input.handle_key(key(KeyCode::Enter)),
            TextInputResult::Commit
        );
    }

    #[test]
    fn esc_signals_cancel() {
        let mut input = TextInput::new("x");
        assert_eq!(input.handle_key(key(KeyCode::Esc)), TextInputResult::Cancel);
    }

    #[test]
    fn ctrl_u_clears() {
        let mut input = TextInput::new("hello");
        input.handle_key(ctrl('u'));
        assert_eq!(input.buffer(), "");
    }

    #[test]
    fn ctrl_w_deletes_prev_word() {
        let mut input = TextInput::new("hello world");
        input.handle_key(ctrl('w'));
        assert_eq!(input.buffer(), "hello ");
    }

    #[test]
    fn changed_tracks_buffer_vs_original() {
        let mut input = TextInput::new("abc");
        assert!(!input.changed());
        input.handle_key(key(KeyCode::Char('d')));
        assert!(input.changed());
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedbackLevel {
    Ok,
    Err,
    Warn,
}

/// The caret glyph [`TextInput::caret_string`] inserts. A row whose value
/// contains it is mid-edit, so it never becomes the saved baseline.
pub const CARET: char = '│';

/// What a boolean setting reads as everywhere in the TUI.
pub fn on_off(b: bool) -> String {
    (if b { "On" } else { "Off" }).to_string()
}

/// What an optional setting with no value reads as everywhere in the TUI.
pub const NOT_SET: &str = "Not set";

/// Content width at which a form and its About pane sit side by side. Below
/// it they stack, so neither column is squeezed into wrapping every label.
pub const SIDE_BY_SIDE_MIN_WIDTH: u16 = 100;

/// Rows the About pane keeps when stacked under a long form.
const STACKED_GUIDANCE_MIN_HEIGHT: u16 = 6;

/// Specification for a row in a two-pane form.
pub struct FormRowSpec {
    pub focused: bool,
    pub dimmed: bool,
    pub label: String,
    pub value: String,
}

impl FormRowSpec {
    pub fn new(focused: bool, label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            focused,
            dimmed: false,
            label: label.into(),
            value: value.into(),
        }
    }

    pub fn dimmed(mut self, dimmed: bool) -> Self {
        self.dimmed = dimmed;
        self
    }
}

/// The config file every section saves to, with the home directory shown as
/// `~` so it fits in a status line.
pub fn display_config_path() -> String {
    let Some(path) = super::config_editor::tui_config_path() else {
        return "config.toml".to_string();
    };
    let shown = path.display().to_string();
    match dirs::home_dir() {
        Some(home) => match path.strip_prefix(&home) {
            Ok(rest) => format!("~/{}", rest.display()),
            Err(_) => shown,
        },
        None => shown,
    }
}

/// Display width of `s` in terminal cells.
pub fn str_width(s: &str) -> usize {
    Span::raw(s).width()
}

/// Cut `s` to at most `max` cells, ending in `…` when anything was cut.
pub fn truncate(s: &str, max: usize) -> String {
    if str_width(s) <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut used = 0;
    for c in s.chars() {
        let w = str_width(c.encode_utf8(&mut [0u8; 4]));
        if used + w > max - 1 {
            break;
        }
        out.push(c);
        used += w;
    }
    out.push('…');
    out
}

/// Cut `s` to at most `max` cells by dropping the middle, which keeps both
/// the start and the file name of a long path readable.
pub fn truncate_middle(s: &str, max: usize) -> String {
    if str_width(s) <= max || max < 5 {
        return truncate(s, max);
    }
    let chars: Vec<char> = s.chars().collect();
    let keep = max - 1;
    let head = keep / 2;
    let tail = keep - head;
    let start: String = chars[..head.min(chars.len())].iter().collect();
    let end: String = chars[chars.len().saturating_sub(tail)..].iter().collect();
    format!("{}…{}", start, end)
}

/// A rounded pane. The focused pane takes the accent border so it is clear
/// where the arrow keys go.
pub fn pane<'a>(title: impl Into<String>, focused: bool) -> Block<'a> {
    let title: String = title.into();
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .padding(Padding::horizontal(1))
        .border_style(if focused {
            theme::border_focused()
        } else {
            theme::border()
        })
        .title(Span::styled(
            format!(" {} ", title),
            theme::pane_title(focused),
        ))
}

/// One `label  value` line for read-only facts, with the label muted and
/// padded to `label_width`.
pub fn fact_line<'a>(label: &str, label_width: usize, value: Vec<Span<'a>>) -> Line<'a> {
    let mut spans = vec![Span::styled(
        format!("{:<width$}  ", label, width = label_width),
        theme::muted(),
    )];
    spans.extend(value);
    Line::from(spans)
}

/// The status line under a section: the last save/revert outcome when there
/// is one, otherwise whether this section holds unsaved edits, otherwise
/// whether saved settings still wait for voxtype to restart.
///
/// `apply_pending` means the file holds settings the running daemon (and
/// the OSD it supervises) has not read yet. Every message then ends with the
/// one thing that applies them: R, or quitting.
pub fn render_status_line(
    f: &mut Frame,
    area: Rect,
    dirty: bool,
    apply_pending: bool,
    feedback: Option<(FeedbackLevel, &str)>,
) {
    // Indented to line up with the text inside the panes above.
    let area = Rect {
        x: area.x + 2,
        width: area.width.saturating_sub(2),
        ..area
    };
    let lines = status_lines(area.width as usize, dirty, apply_pending, feedback);
    f.render_widget(Paragraph::new(lines), area);
}

/// Most lines a status message may take. A message that tells the user to
/// press a key must not be cut off before the key.
const STATUS_MAX_LINES: usize = 3;

/// Rows the status line needs in an area `width` cells wide.
pub fn status_height(
    width: u16,
    dirty: bool,
    apply_pending: bool,
    feedback: Option<(FeedbackLevel, &str)>,
) -> u16 {
    status_lines(
        width.saturating_sub(2) as usize,
        dirty,
        apply_pending,
        feedback,
    )
    .len() as u16
}

/// Greedy word wrap to `width` cells.
pub fn wrap_words(text: &str, width: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        match lines.last_mut() {
            Some(line) if str_width(line) + 1 + str_width(word) <= width => {
                line.push(' ');
                line.push_str(word);
            }
            _ => lines.push(word.to_string()),
        }
    }
    lines
}

fn status_lines(
    width: usize,
    dirty: bool,
    apply_pending: bool,
    feedback: Option<(FeedbackLevel, &str)>,
) -> Vec<Line<'static>> {
    let line = match feedback {
        Some((level, message)) if !(level == FeedbackLevel::Ok && apply_pending) => {
            let message = match super::config_editor::tui_config_path() {
                Some(full) => message.replace(&full.display().to_string(), &display_config_path()),
                None => message.to_string(),
            };
            let (glyph, style) = match level {
                FeedbackLevel::Ok => ("✓ ", theme::ok()),
                FeedbackLevel::Err => ("✗ ", theme::error()),
                FeedbackLevel::Warn => ("! ", theme::warn()),
            };
            let mut wrapped = wrap_words(&message, width.saturating_sub(2).max(1));
            if wrapped.len() > STATUS_MAX_LINES {
                wrapped.truncate(STATUS_MAX_LINES);
                let last = wrapped.last_mut().expect("non-empty");
                *last = truncate(&format!("{} …", last), width.saturating_sub(2));
            }
            return wrapped
                .into_iter()
                .enumerate()
                .map(|(i, text)| {
                    let lead = if i == 0 { glyph } else { "  " };
                    Line::from(vec![
                        Span::styled(lead, style.add_modifier(Modifier::BOLD)),
                        Span::styled(text, style),
                    ])
                })
                .collect();
        }
        feedback => feedback,
    };
    vec![status_line(width, dirty, apply_pending, line)]
}

fn status_line(
    width: usize,
    dirty: bool,
    apply_pending: bool,
    feedback: Option<(FeedbackLevel, &str)>,
) -> Line<'static> {
    match feedback {
        Some((level, message)) => {
            // Sections report the full path they saved to; show it the way
            // the header does.
            let message = match super::config_editor::tui_config_path() {
                Some(full) => message.replace(&full.display().to_string(), &display_config_path()),
                None => message.to_string(),
            };
            let room = width.saturating_sub(2);
            let message = if level == FeedbackLevel::Ok && apply_pending {
                let base = message.trim_end_matches('.');
                first_that_fits(
                    &[
                        format!("{}. In use once voxtype restarts: press R, or quit", base),
                        format!("{}. R restarts voxtype to use it", base),
                        "Saved. R restarts voxtype to use it".to_string(),
                    ],
                    room,
                )
            } else {
                truncate(&message, room)
            };
            let (glyph, style) = match level {
                FeedbackLevel::Ok => ("✓ ", theme::ok()),
                FeedbackLevel::Err => ("✗ ", theme::error()),
                FeedbackLevel::Warn => ("! ", theme::warn()),
            };
            Line::from(vec![
                Span::styled(glyph, style.add_modifier(Modifier::BOLD)),
                Span::styled(message, style),
            ])
        }
        None if !dirty && apply_pending => {
            let text = first_that_fits(
                &[
                    "Saved, not in use yet: R restarts voxtype and its OSD (quitting does too)"
                        .to_string(),
                    "Saved, not in use yet: R restarts voxtype and its OSD".to_string(),
                    "Saved, not in use yet: press R".to_string(),
                ],
                width.saturating_sub(2),
            );
            Line::from(vec![
                Span::styled("! ", theme::warn().add_modifier(Modifier::BOLD)),
                Span::styled(text, theme::warn()),
            ])
        }
        None if dirty => {
            let path = display_config_path();
            let text = first_that_fits(
                &[
                    format!("Not saved yet: s writes {}, r reverts", path),
                    format!("Not saved yet: s writes {}", path),
                    "Not saved yet: s saves, r reverts".to_string(),
                ],
                width.saturating_sub(2),
            );
            Line::from(vec![
                Span::styled("● ", theme::warn().add_modifier(Modifier::BOLD)),
                Span::styled(text, theme::warn()),
            ])
        }
        None => {
            let path = display_config_path();
            let text = first_that_fits(
                &[format!("All saved to {}", path), "All saved".to_string()],
                width,
            );
            Line::from(Span::styled(text, theme::muted()))
        }
    }
}

/// The first message that fits in `width` cells, or the last one cut to fit.
/// Lets a status line drop whole clauses on narrow terminals instead of
/// ending mid-word.
pub fn first_that_fits(candidates: &[String], width: usize) -> String {
    candidates
        .iter()
        .find(|c| str_width(c) <= width)
        .cloned()
        .unwrap_or_else(|| truncate(candidates.last().map_or("", |c| c.as_str()), width))
}

/// Width of the label column: as wide as the longest label, but leaving the
/// value at least a third of the row.
fn label_column_width(rows: &[FormRowSpec], inner_width: usize) -> usize {
    let longest = rows.iter().map(|r| str_width(&r.label)).max().unwrap_or(0);
    let value_room = (inner_width / 3).max(16);
    let cap = inner_width.saturating_sub(4 + value_room).max(12);
    longest.min(cap)
}

/// First visible row so the focused row stays on screen, keeping one row of
/// context below it when there is more to show.
pub fn scroll_offset(focused: usize, total: usize, visible: usize) -> usize {
    if visible == 0 || total <= visible {
        return 0;
    }
    let want_bottom = (focused + 2).min(total);
    want_bottom.saturating_sub(visible)
}

/// One form row: an unsaved marker gutter, the label, then the value. The
/// focused row is reversed video across the whole width; it alone shows the
/// `◂ ▸` cue that arrows change it. With focus in the section list no row
/// is highlighted, so there is one selection on screen at a time.
fn form_line(
    row: &FormRowSpec,
    modified: bool,
    pane_focused: bool,
    label_width: usize,
    width: usize,
) -> Line<'static> {
    let editing = row.value.contains(CARET);
    let row_focused = row.focused && pane_focused;
    let gutter = if modified { "● " } else { "  " };
    let label = format!("{:<w$}", truncate(&row.label, label_width), w = label_width);
    let value_room = width.saturating_sub(2 + label_width + 2);
    let shown_value = if row_focused && !editing && !row.dimmed {
        format!("◂ {} ▸", row.value)
    } else {
        row.value.clone()
    };
    let shown_value = truncate(&shown_value, value_room);
    let pad = value_room.saturating_sub(str_width(&shown_value));

    if row_focused {
        let mut style = theme::selected();
        if row.dimmed {
            style = style.add_modifier(Modifier::DIM);
        }
        let gutter_style = if modified {
            theme::warn().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        return Line::from(vec![
            Span::styled(gutter, gutter_style),
            Span::styled(
                format!("{}  {}{}", label, shown_value, " ".repeat(pad)),
                style,
            ),
        ]);
    }

    let (label_style, value_style) = if row.dimmed {
        (theme::muted(), theme::muted())
    } else if row.value == NOT_SET {
        (theme::text(), theme::muted())
    } else {
        (theme::text(), theme::text())
    };
    Line::from(vec![
        Span::styled(gutter, theme::warn().add_modifier(Modifier::BOLD)),
        Span::styled(label, label_style),
        Span::raw("  "),
        Span::styled(shown_value, value_style),
    ])
}

/// Which rows differ from what was last saved. While a section is clean its
/// rows become the baseline; once it is dirty, a row whose value no longer
/// matches the baseline for its label is an unsaved edit.
fn modified_rows(app: &App, title: &str, dirty: bool, rows: &[FormRowSpec]) -> Vec<bool> {
    let mut baselines = app.baselines.borrow_mut();
    let saved = baselines.entry(title.to_string()).or_default();
    if !dirty {
        for row in rows {
            if !row.value.contains(CARET) {
                saved.insert(row.label.clone(), row.value.clone());
            }
        }
        return vec![false; rows.len()];
    }
    rows.iter()
        .map(|row| {
            !row.value.contains(CARET)
                && saved
                    .get(&row.label)
                    .is_some_and(|before| *before != row.value)
        })
        .collect()
}

/// Lines `lines` will occupy once wrapped to `width` cells.
fn wrapped_height(lines: &[Line<'_>], width: u16) -> u16 {
    let width = width.max(1) as usize;
    lines
        .iter()
        .map(|l| (l.width().max(1)).div_ceil(width) as u16)
        .sum()
}

/// Render a form section: the settings form and an About pane explaining the
/// focused setting, side by side on wide terminals and stacked on narrow
/// ones, with a status line underneath.
///
/// The form scrolls to keep the focused row visible. Rows edited since the
/// last save carry a `●`, and the form's title counts them.
#[allow(clippy::too_many_arguments)]
pub fn render_form_with_guidance(
    f: &mut Frame,
    area: Rect,
    app: &App,
    title: &str,
    dirty: bool,
    feedback: Option<(FeedbackLevel, &str)>,
    rows: &[FormRowSpec],
    guidance: Vec<Line<'_>>,
) {
    // An app-wide notice (the last restart) outranks the section's own message.
    let feedback = match &app.notice {
        Some((level, message)) => Some((*level, message.as_str())),
        None => feedback,
    };
    let pending = app.apply_pending.get();
    let status_rows = status_height(area.width, dirty, pending, feedback);
    let [body, status] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(status_rows)])
        .areas(area);
    render_status_line(f, status, dirty, pending, feedback);

    let (form_area, about_area) = if body.width >= SIDE_BY_SIDE_MIN_WIDTH {
        let [form, about] = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .areas(body);
        (form, about)
    } else {
        let wanted = rows.len() as u16 + 2;
        let room = body
            .height
            .saturating_sub(STACKED_GUIDANCE_MIN_HEIGHT)
            .max(5);
        let [form, about] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(wanted.min(room)), Constraint::Min(0)])
            .areas(body);
        (form, about)
    };

    let modified = modified_rows(app, title, dirty, rows);
    let modified_count = modified.iter().filter(|m| **m).count();
    let focused = app.content_focused();

    let mut block = pane(title, focused);
    if dirty {
        let label = match modified_count {
            0 => " ● unsaved ".to_string(),
            1 => " ● 1 unsaved change ".to_string(),
            n => format!(" ● {} unsaved changes ", n),
        };
        block = block.title(Line::from(Span::styled(label, theme::warn())).right_aligned());
    }
    let inner = block.inner(form_area);

    let visible = inner.height as usize;
    let focused_idx = rows.iter().position(|r| r.focused).unwrap_or(0);
    let offset = scroll_offset(focused_idx, rows.len(), visible);
    let below = rows.len().saturating_sub(offset + visible);
    if offset > 0 || below > 0 {
        let hint = match (offset > 0, below > 0) {
            (true, true) => format!(" ↑↓ {} more ", offset + below),
            (true, false) => format!(" ↑ {} more ", offset),
            _ => format!(" ↓ {} more ", below),
        };
        block = block.title_bottom(Line::from(Span::styled(hint, theme::muted())).right_aligned());
    }
    f.render_widget(block, form_area);

    let width = inner.width as usize;
    let label_width = label_column_width(rows, width);
    let lines: Vec<Line> = rows
        .iter()
        .zip(modified.iter())
        .skip(offset)
        .take(visible)
        .map(|(row, m)| form_line(row, *m, focused, label_width, width))
        .collect();
    f.render_widget(Paragraph::new(lines), inner);

    if about_area.height >= 3 {
        render_guidance(f, about_area, app, guidance);
    }
}

/// The About pane, scrolled by PgUp/PgDn. Long text says how much is left
/// rather than vanishing past the border.
pub fn render_guidance(f: &mut Frame, area: Rect, app: &App, guidance: Vec<Line<'_>>) {
    let mut block = pane("About", false);
    let inner = block.inner(area);
    let total = wrapped_height(&guidance, inner.width);
    let max_scroll = total.saturating_sub(inner.height);
    let scroll = app.guidance_scroll.get().min(max_scroll);
    app.guidance_scroll.set(scroll);
    if max_scroll > 0 {
        let hint = if scroll < max_scroll {
            " PgDn more "
        } else {
            " PgUp back "
        };
        block = block.title_bottom(Line::from(Span::styled(hint, theme::muted())).right_aligned());
    }
    f.render_widget(block, area);
    f.render_widget(
        Paragraph::new(guidance)
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0)),
        inner,
    );
}

/// Shown in place of a section whose state could not be loaded.
pub fn render_load_error(f: &mut Frame, area: Rect, title: &str) {
    let block = pane(title, false);
    let inner = block.inner(area);
    f.render_widget(block, area);
    f.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                format!("✗ Could not read {}", display_config_path()),
                theme::error(),
            )),
            Line::from(""),
            Line::from("Fix the file by hand or run `voxtype config` to see the parse error."),
        ])
        .wrap(Wrap { trim: true }),
        inner,
    );
}

#[cfg(test)]
mod render_tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    fn line_text(line: &Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    fn row(focused: bool, label: &str, value: &str) -> FormRowSpec {
        FormRowSpec::new(focused, label, value)
    }

    #[test]
    fn truncate_marks_the_cut() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 6), "hello…");
        assert_eq!(truncate("hello", 0), "");
    }

    #[test]
    fn truncate_middle_keeps_both_ends_of_a_path() {
        let cut = truncate_middle("/home/someone/.config/voxtype/config.toml", 20);
        assert_eq!(str_width(&cut), 20);
        assert!(cut.starts_with("/home"));
        assert!(cut.ends_with(".toml"));
        assert!(cut.contains('…'));
    }

    #[test]
    fn first_that_fits_drops_whole_clauses() {
        let options = [
            "Not saved yet: s writes the file, r reverts".to_string(),
            "Not saved yet".to_string(),
        ];
        assert_eq!(first_that_fits(&options, 80), options[0]);
        assert_eq!(first_that_fits(&options, 20), "Not saved yet");
        assert_eq!(first_that_fits(&options, 5), "Not …");
    }

    #[test]
    fn scrolling_keeps_the_focused_row_and_one_after_it_visible() {
        assert_eq!(scroll_offset(0, 15, 10), 0);
        assert_eq!(scroll_offset(8, 15, 10), 0);
        // Row 9 is the last visible one; one row of context follows it.
        assert_eq!(scroll_offset(9, 15, 10), 1);
        assert_eq!(scroll_offset(14, 15, 10), 5);
        assert_eq!(scroll_offset(3, 5, 10), 0);
    }

    #[test]
    fn long_labels_leave_the_value_a_third_of_the_row() {
        let rows = [row(false, &"x".repeat(60), "v")];
        let width = label_column_width(&rows, 60);
        assert!(60 - width >= 20, "label took {} of 60", width);
        let short = [row(false, "Mode", "v")];
        assert_eq!(label_column_width(&short, 60), 4);
    }

    #[test]
    fn only_the_focused_row_of_a_focused_pane_shows_the_change_cue() {
        let focused = row(true, "Mode", "toggle");
        let other = row(false, "Key", "HOME");
        assert!(line_text(&form_line(&focused, false, true, 6, 40)).contains("◂ toggle ▸"));
        assert!(!line_text(&form_line(&other, false, true, 6, 40)).contains('◂'));
        // Focus in the section list: nothing in the form is highlighted.
        assert!(!line_text(&form_line(&focused, false, false, 6, 40)).contains('◂'));
    }

    #[test]
    fn unsaved_rows_carry_a_marker_that_is_not_only_color() {
        let r = row(false, "Mode", "toggle");
        assert!(line_text(&form_line(&r, true, true, 6, 40)).starts_with("● "));
        assert!(line_text(&form_line(&r, false, true, 6, 40)).starts_with("  "));
    }

    #[test]
    fn rows_being_typed_into_show_no_change_cue() {
        let r = row(true, "Key", &format!("HO{}ME", CARET));
        let text = line_text(&form_line(&r, false, true, 6, 40));
        assert!(!text.contains('◂'));
        assert!(text.contains("HO│ME"));
    }

    fn status_text(dirty: bool, feedback: Option<(FeedbackLevel, &str)>) -> String {
        let mut terminal = Terminal::new(TestBackend::new(70, 1)).unwrap();
        terminal
            .draw(|f| render_status_line(f, f.area(), dirty, false, feedback))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        buffer.content().iter().map(|c| c.symbol()).collect()
    }

    #[test]
    fn status_line_reports_outcome_then_unsaved_then_saved() {
        let saved = status_text(false, Some((FeedbackLevel::Ok, "Saved")));
        assert!(saved.trim_start().starts_with("✓ Saved"), "{saved:?}");
        let failed = status_text(true, Some((FeedbackLevel::Err, "save: bad value")));
        assert!(
            failed.trim_start().starts_with("✗ save: bad value"),
            "{failed:?}"
        );
        let dirty = status_text(true, None);
        assert!(
            dirty.trim_start().starts_with("● Not saved yet"),
            "{dirty:?}"
        );
        let clean = status_text(false, None);
        assert!(clean.trim_start().starts_with("All saved"), "{clean:?}");
    }
}
