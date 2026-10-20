//! Left-hand section list.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use super::app::App;
use super::common;
use super::section::Section;
use super::theme;

/// Width of the sidebar column, borders included. Fits "Notifications", the
/// current-section marker and the unsaved marker.
pub const WIDTH: u16 = 21;

/// One line of the sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    Heading(&'static str),
    Item(usize),
}

/// The sidebar's lines for an inner height of `height`. Group headings are
/// dropped when the terminal is too short for them.
pub fn rows(height: u16) -> Vec<Row> {
    let mut with_headings = Vec::new();
    let mut group = "";
    for (idx, section) in Section::ALL.iter().enumerate() {
        if section.group() != group {
            group = section.group();
            with_headings.push(Row::Heading(group));
        }
        with_headings.push(Row::Item(idx));
    }
    if with_headings.len() <= height as usize {
        with_headings
    } else {
        (0..Section::ALL.len()).map(Row::Item).collect()
    }
}

/// The lines that fit in `height`, scrolled so section `focus` is visible,
/// and how many are hidden above and below.
pub fn window(height: u16, focus: usize) -> (Vec<Row>, usize, usize) {
    let all = rows(height);
    let visible = height as usize;
    let at = all.iter().position(|r| *r == Row::Item(focus)).unwrap_or(0);
    let offset = common::scroll_offset(at, all.len(), visible);
    let shown: Vec<Row> = all.iter().skip(offset).take(visible).copied().collect();
    let below = all.len().saturating_sub(offset + shown.len());
    (shown, offset, below)
}

/// Section index under the sidebar line at `line` (0 = first inner line).
pub fn item_at(height: u16, focus: usize, line: u16) -> Option<usize> {
    match window(height, focus).0.get(line as usize) {
        Some(Row::Item(idx)) => Some(*idx),
        _ => None,
    }
}

/// The section the sidebar keeps in view: the one under the cursor while
/// the list has focus, otherwise the open one.
pub fn focus_index(app: &App) -> usize {
    if app.sidebar_focused {
        app.sidebar_cursor
    } else {
        Section::ALL
            .iter()
            .position(|s| *s == app.current_section)
            .unwrap_or(0)
    }
}

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let mut block = common::pane("Sections", app.sidebar_focused);
    let inner = block.inner(area);
    let (shown, above, below) = window(inner.height, focus_index(app));
    if above > 0 || below > 0 {
        let hint = match (above > 0, below > 0) {
            (true, true) => " ↑↓ ",
            (true, false) => " ↑ ",
            _ => " ↓ ",
        };
        block = block.title_bottom(Line::from(Span::styled(hint, theme::muted())).right_aligned());
    }
    f.render_widget(block, area);

    let width = inner.width as usize;
    let lines: Vec<Line> = shown
        .into_iter()
        .map(|row| match row {
            Row::Heading(name) => Line::from(Span::styled(
                name.to_string(),
                theme::muted().add_modifier(Modifier::ITALIC),
            )),
            Row::Item(idx) => item_line(app, idx, width),
        })
        .collect();

    f.render_widget(Paragraph::new(lines), inner);
}

fn item_line(app: &App, idx: usize, width: usize) -> Line<'static> {
    let section = Section::ALL[idx];
    let open = section == app.current_section;
    let hovered = app.sidebar_focused && idx == app.sidebar_cursor;
    let dirty = app.section_dirty(section);

    let marker = if open { "› " } else { "  " };
    let label = common::truncate(section.label(), width.saturating_sub(4));
    let pad = width.saturating_sub(2 + common::str_width(&label) + 2);
    let dot = if dirty { " ●" } else { "  " };

    let label_style = if hovered {
        theme::selected()
    } else if open {
        theme::accent().add_modifier(Modifier::BOLD)
    } else {
        theme::text()
    };
    let dot_style = if hovered {
        theme::selected()
    } else {
        theme::warn().add_modifier(Modifier::BOLD)
    };
    Line::from(vec![
        Span::styled(
            marker,
            if hovered {
                theme::selected()
            } else {
                theme::accent()
            },
        ),
        Span::styled(format!("{}{}", label, " ".repeat(pad)), label_style),
        Span::styled(dot, dot_style),
    ])
    .style(Style::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings_show_when_they_fit() {
        let all = rows(40);
        assert_eq!(all.first(), Some(&Row::Heading("Setup")));
        assert_eq!(
            all.iter().filter(|r| matches!(r, Row::Item(_))).count(),
            Section::ALL.len()
        );
    }

    #[test]
    fn short_terminals_drop_headings_but_keep_every_section() {
        let short = rows(Section::ALL.len() as u16);
        assert_eq!(short.len(), Section::ALL.len());
        assert!(short.iter().all(|r| matches!(r, Row::Item(_))));
    }

    #[test]
    fn clicks_on_headings_select_nothing() {
        assert_eq!(item_at(40, 0, 0), None);
        assert_eq!(item_at(40, 0, 1), Some(0));
        assert_eq!(item_at(12, 0, 0), Some(0));
    }

    /// At 60x16 under the warning banner the list has 10 lines for 12
    /// sections; the last ones must still be reachable.
    #[test]
    fn a_short_list_scrolls_to_keep_the_focused_section_visible() {
        let last = Section::ALL.len() - 1;
        let (shown, above, below) = window(10, last);
        assert_eq!(shown.len(), 10);
        assert!(shown.contains(&Row::Item(last)));
        assert_eq!((above, below), (2, 0));
        assert_eq!(item_at(10, last, 9), Some(last));
    }
}
