//! STT-specific helpers for the interactive setup TUI.

use crossterm::event::KeyCode;
use ratatui::prelude::*;
use ratatui::widgets::*;

pub const THRESHOLD_PRESETS: &[&str] = &["0.5", "1.0", "2.0", "3.0", "5.0", "7.5", "10.0"];
pub const SILENCE_PRESETS: &[&str] = &["0.5", "0.8", "1.0", "1.5", "2.0", "3.0", "5.0"];
pub const TOGGLE_COUNT: usize = 2;

pub fn toggle_option(selected: usize, trim_silence: &mut bool, auto_enter: &mut bool) {
    if selected == 0 {
        *trim_silence = !*trim_silence;
    } else {
        *auto_enter = !*auto_enter;
    }
}

pub fn handle_toggle_key(
    code: KeyCode,
    selected: usize,
    trim_silence: &mut bool,
    auto_enter: &mut bool,
) -> bool {
    match code {
        KeyCode::Enter | KeyCode::Char(' ') => {
            toggle_option(selected, trim_silence, auto_enter);
            true
        }
        _ => false,
    }
}

pub fn render_threshold_list(selected: usize, active: bool) -> List<'static> {
    render_preset_list("Threshold", THRESHOLD_PRESETS, selected, active)
}

pub fn render_silence_list(selected: usize, active: bool) -> List<'static> {
    render_preset_list("Silence", SILENCE_PRESETS, selected, active)
}

pub fn render_toggle_list(
    selected: usize,
    active: bool,
    trim_silence: bool,
    auto_enter: bool,
) -> List<'static> {
    let items = [(trim_silence, "trim silence"), (auto_enter, "auto enter")];

    let items: Vec<ListItem> = items
        .iter()
        .enumerate()
        .map(|(i, (enabled, label))| {
            let marker = if i == selected { "> " } else { "  " };
            let checkbox = if *enabled { "[x]" } else { "[ ]" };
            ListItem::new(format!("{marker}{checkbox} {label}"))
                .style(item_style(i, selected, active))
        })
        .collect();

    List::new(items).block(
        Block::bordered()
            .title(" Options ")
            .border_style(border_style(active)),
    )
}

pub fn summary_lines(
    threshold_idx: usize,
    silence_idx: usize,
    trim_silence: bool,
    auto_enter: bool,
) -> Vec<Line<'static>> {
    vec![
        Line::from(vec![
            Span::styled("Threshold: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                THRESHOLD_PRESETS[threshold_idx],
                Style::default().fg(Color::White),
            ),
            Span::styled("%", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled("Silence:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                SILENCE_PRESETS[silence_idx],
                Style::default().fg(Color::White),
            ),
            Span::styled("s", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled("Trim:      ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                if trim_silence { "on" } else { "off" },
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("AutoEnter: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                if auto_enter { "on" } else { "off" },
                Style::default().fg(Color::White),
            ),
        ]),
    ]
}

fn render_preset_list(
    title: &'static str,
    items: &'static [&'static str],
    selected: usize,
    active: bool,
) -> List<'static> {
    let items: Vec<ListItem> = items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let marker = if i == selected { "> " } else { "  " };
            ListItem::new(format!("{marker}{item}")).style(item_style(i, selected, active))
        })
        .collect();

    List::new(items).block(
        Block::bordered()
            .title(format!(" {title} "))
            .border_style(border_style(active)),
    )
}

fn item_style(index: usize, selected: usize, active: bool) -> Style {
    if index == selected && active {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else if index == selected {
        Style::default().fg(Color::White)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

fn border_style(active: bool) -> Style {
    if active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}
