use std::path::Path;

use ratatui::prelude::Widget;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Tabs, Wrap};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
};

use super::*;

pub(crate) fn draw_ui(area: Rect, buf: &mut ratatui::buffer::Buffer, app: &TuiApp) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(5),
        ])
        .split(area);
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(28), Constraint::Percentage(72)])
        .split(outer[1]);

    let header = Paragraph::new("SSOT Manager TUI")
        .block(Block::default().borders(Borders::ALL).title("Header"));
    header.render(outer[0], buf);

    render_profile_list(body[0], buf, app);
    render_detail(body[1], buf, app);
    render_editor_overlay(body[1], buf, app);

    let footer = Paragraph::new(Text::from(vec![
        Line::from(vec![
            Span::raw("Status: "),
            Span::styled(app.message.clone(), style_for_message(&app.message)),
        ]),
        Line::from(footer_keys(app)),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("Status / Keys"),
    )
    .wrap(Wrap { trim: true });
    footer.render(outer[2], buf);
}

fn visible_window(total: usize, selected: usize, available: usize) -> (usize, usize) {
    if total == 0 || available == 0 {
        return (0, 0);
    }

    let selected = selected.min(total.saturating_sub(1));
    let mut start = selected.saturating_add(1).saturating_sub(available);
    let mut end = (start + available).min(total);
    if end - start < available {
        start = end.saturating_sub(available);
        end = total.min(start + available);
    }
    (start, end)
}

pub(super) fn render_profile_list(area: Rect, buf: &mut ratatui::buffer::Buffer, app: &TuiApp) {
    let visible_rows = area.height.saturating_sub(2) as usize;
    let (start, end) = visible_window(app.profiles.len(), app.selected_profile, visible_rows);
    let items = if app.profiles.is_empty() {
        vec![ListItem::new("No profiles configured")]
    } else {
        app.profiles
            .iter()
            .enumerate()
            .skip(start)
            .take(end.saturating_sub(start))
            .map(|(index, profile)| {
                let line = if index == app.selected_profile {
                    Line::styled(
                        format!("> {profile}"),
                        Style::default().add_modifier(Modifier::BOLD),
                    )
                } else {
                    Line::from(format!("  {profile}"))
                };
                ListItem::new(line)
            })
            .collect()
    };

    let title = if app.detail_focused() {
        "Profiles"
    } else {
        "Profiles [Browse]"
    };
    let list = List::new(items).block(Block::default().borders(Borders::ALL).title(title));
    list.render(area, buf);
}

pub(super) fn render_detail(area: Rect, buf: &mut ratatui::buffer::Buffer, app: &TuiApp) {
    if let Some(editor) = &app.editor {
        render_editor_detail(area, buf, editor);
        return;
    }

    let detail = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)])
        .split(area);
    let tabs = Tabs::new(DetailView::titles())
        .select(app.active_view.index())
        .block(Block::default().borders(Borders::ALL).title("View"))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));
    tabs.render(detail[0], buf);

    let text = app
        .detail_text()
        .unwrap_or_else(|error| Text::from(format!("Error: {error:#}")));
    let visible_lines = detail[1].height.saturating_sub(2) as usize;
    let total_lines = text.lines.len().max(1);
    let max_scroll = total_lines.saturating_sub(visible_lines) as u16;
    let scroll = app.detail_scroll.min(max_scroll);
    let title = detail_title(app, scroll, visible_lines, total_lines);
    let block = Block::default().borders(Borders::ALL).title(title);
    block.clone().render(detail[1], buf);
    let inner = block.inner(detail[1]);
    let (content_area, scrollbar_area) = split_scrollbar_column(inner, total_lines > visible_lines);

    let paragraph = Paragraph::new(text)
        .scroll((scroll, 0))
        .wrap(Wrap { trim: false });
    paragraph.render(content_area, buf);
    render_vertical_scrollbar(
        buf,
        scrollbar_area,
        ScrollbarMetrics::new(total_lines, visible_lines, scroll as usize),
    );
}

fn render_editor_overlay(area: Rect, buf: &mut ratatui::buffer::Buffer, app: &TuiApp) {
    let Some(editor) = &app.editor else {
        return;
    };

    match &editor.overlay {
        EditorOverlay::None => {}
        EditorOverlay::TextInput(state) => render_text_input_overlay(area, buf, state),
        EditorOverlay::StringList(state) => render_string_list_overlay(area, buf, editor, state),
        EditorOverlay::RuleList(state) => render_rule_list_overlay(area, buf, editor, state),
        EditorOverlay::RuleEditor(state) => render_rule_editor_overlay(area, buf, editor, state),
        EditorOverlay::Confirm(state) => render_confirm_overlay(area, buf, state),
    }
}

fn render_text_input_overlay(
    area: Rect,
    buf: &mut ratatui::buffer::Buffer,
    state: &TextInputState,
) {
    let completion_lines = state
        .completion
        .as_ref()
        .map(|completion| completion.matches.len().min(4) as u16)
        .unwrap_or(0);
    let popup = centered_rect(area, 70, 8 + completion_lines);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(state.title.clone());
    block.clone().render(popup, buf);
    let inner = block.inner(popup);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(inner);
    Paragraph::new(text_input_hint(state))
        .style(Style::default().fg(Color::Yellow))
        .render(chunks[0], buf);
    let input = Paragraph::new(state.value.clone()).block(Block::default().borders(Borders::ALL));
    input.render(chunks[1], buf);
    if let Some(completion) = &state.completion {
        let lines = completion
            .matches
            .iter()
            .take(4)
            .enumerate()
            .map(|(index, value)| {
                if index == completion.selected {
                    Line::styled(
                        format!("> {value}"),
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    Line::from(format!("  {value}"))
                }
            })
            .collect::<Vec<_>>();
        Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: true })
            .render(chunks[2], buf);
    }
}

pub(super) fn render_string_list_overlay(
    area: Rect,
    buf: &mut ratatui::buffer::Buffer,
    editor: &ProfileEditorState,
    state: &StringListEditorState,
) {
    let popup = centered_rect(area, 75, 14);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(state.title.clone());
    block.clone().render(popup, buf);
    let inner = block.inner(popup);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);
    let values = match state.kind {
        StringListKind::Requires => &editor.draft.requires,
        StringListKind::RuleDestinations { rule_index } => editor
            .draft
            .rules
            .get(rule_index)
            .map(|rule| &rule.to)
            .unwrap_or(&editor.draft.requires),
        StringListKind::RuleTags { rule_index } => editor
            .draft
            .rules
            .get(rule_index)
            .map(|rule| &rule.tags)
            .unwrap_or(&editor.draft.requires),
    };
    let visible_rows = chunks[1].height as usize;
    let (start, end) = visible_window(values.len(), state.selected, visible_rows);
    let (list_area, scrollbar_area) =
        split_scrollbar_column(chunks[1], values.len() > visible_rows);

    let items = if values.is_empty() {
        vec![ListItem::new("No entries")]
    } else {
        values
            .iter()
            .enumerate()
            .skip(start)
            .take(end.saturating_sub(start))
            .map(|(index, value)| {
                ListItem::new(if index == state.selected {
                    Line::styled(
                        format!("> {value}"),
                        Style::default().add_modifier(Modifier::BOLD),
                    )
                } else {
                    Line::from(format!("  {value}"))
                })
            })
            .collect()
    };
    Paragraph::new("a add | Enter edit | Del remove | J/K reorder | Esc back")
        .style(Style::default().fg(Color::Yellow))
        .render(chunks[0], buf);
    List::new(items).render(list_area, buf);
    render_vertical_scrollbar(
        buf,
        scrollbar_area,
        ScrollbarMetrics::new(values.len(), visible_rows, start),
    );
}

pub(super) fn render_rule_list_overlay(
    area: Rect,
    buf: &mut ratatui::buffer::Buffer,
    editor: &ProfileEditorState,
    state: &RuleListEditorState,
) {
    let popup = centered_rect(area, 85, 16);
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Profile Rules");
    block.clone().render(popup, buf);
    let inner = block.inner(popup);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);
    let visible_rows = chunks[1].height as usize;
    let (start, end) = visible_window(editor.draft.rules.len(), state.selected, visible_rows);
    let (list_area, scrollbar_area) =
        split_scrollbar_column(chunks[1], editor.draft.rules.len() > visible_rows);
    let items = if editor.draft.rules.is_empty() {
        vec![ListItem::new("No rules")]
    } else {
        editor
            .draft
            .rules
            .iter()
            .enumerate()
            .skip(start)
            .take(end.saturating_sub(start))
            .map(|(index, rule)| {
                let prefix = if index == state.selected { "> " } else { "  " };
                let line = Line::from(vec![
                    Span::styled(
                        prefix.to_string(),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(format!(
                        "{} [{}] to={} ",
                        rule.select,
                        rule.mode,
                        rule.to.len()
                    )),
                    Span::styled(
                        if rule.enabled { "enabled" } else { "disabled" },
                        Style::default()
                            .fg(if rule.enabled {
                                Color::Green
                            } else {
                                Color::Red
                            })
                            .add_modifier(Modifier::BOLD),
                    ),
                ]);
                ListItem::new(line)
            })
            .collect()
    };
    Paragraph::new("Space toggle | a add | Enter edit | Del remove | J/K reorder | Esc back")
        .style(Style::default().fg(Color::Yellow))
        .render(chunks[0], buf);
    List::new(items).render(list_area, buf);
    render_vertical_scrollbar(
        buf,
        scrollbar_area,
        ScrollbarMetrics::new(editor.draft.rules.len(), visible_rows, start),
    );
}

fn render_rule_editor_overlay(
    area: Rect,
    buf: &mut ratatui::buffer::Buffer,
    editor: &ProfileEditorState,
    state: &RuleEditorState,
) {
    let Some(rule) = editor.draft.rules.get(state.rule_index) else {
        return;
    };
    let popup = centered_rect(area, 85, 15);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!("Rule {}", state.rule_index + 1));
    block.clone().render(popup, buf);
    let inner = block.inner(popup);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);
    let lines = vec![
        editor_line(
            state.selected_field == RuleField::Select,
            "select",
            &rule.select,
        ),
        editor_line(state.selected_field == RuleField::Mode, "mode", &rule.mode),
        editor_status_line(state.selected_field == RuleField::Enabled, rule.enabled),
        editor_line(
            state.selected_field == RuleField::Note,
            "note",
            rule.note.as_deref().unwrap_or("<none>"),
        ),
        editor_line(
            state.selected_field == RuleField::Destinations,
            "destinations",
            &format!("{} entries", rule.to.len()),
        ),
        editor_line(
            state.selected_field == RuleField::Tags,
            "tags",
            &format!("{} entries", rule.tags.len()),
        ),
    ];
    Paragraph::new("Enter edit/action | Space toggle enabled | Esc back")
        .style(Style::default().fg(Color::Yellow))
        .render(chunks[0], buf);
    Paragraph::new(Text::from(lines))
        .wrap(Wrap { trim: false })
        .render(chunks[1], buf);
}

fn render_confirm_overlay(
    area: Rect,
    buf: &mut ratatui::buffer::Buffer,
    state: &EditorConfirmState,
) {
    let popup = centered_rect(area, 65, 7);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(state.title.clone());
    block.clone().render(popup, buf);
    let inner = block.inner(popup);
    Paragraph::new(state.message.clone())
        .wrap(Wrap { trim: true })
        .render(inner, buf);
}

fn centered_rect(area: Rect, width_percent: u16, height: u16) -> Rect {
    let width = area.width.saturating_mul(width_percent).saturating_div(100);
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width.max(10), height.min(area.height))
}

fn editor_line(selected: bool, label: &str, value: &str) -> Line<'static> {
    if selected {
        Line::styled(
            format!("> {label}: {value}"),
            Style::default().add_modifier(Modifier::BOLD),
        )
    } else {
        Line::from(format!("  {label}: {value}"))
    }
}

fn editor_status_line(selected: bool, enabled: bool) -> Line<'static> {
    let marker = if selected { ">" } else { " " };
    let value = if enabled { "enabled" } else { "disabled" };
    let color = if enabled { Color::Green } else { Color::Red };
    Line::from(vec![
        Span::raw(format!("{marker} enabled: ")),
        Span::styled(
            value.to_string(),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
    ])
}

fn text_input_hint(state: &TextInputState) -> &'static str {
    match state.target {
        TextInputTarget::ProfileSourceRoot | TextInputTarget::RuleDestination { .. } => {
            "Enter save | Tab cycle path matches | Esc back"
        }
        _ => "Enter save | Esc back",
    }
}

fn footer_keys(app: &TuiApp) -> &'static str {
    if app.editing() {
        "Keys: j/k or Up/Down move | Enter edit/open | s save | Esc close/back | d confirm discard"
    } else if app.detail_focused() {
        "Keys: q quit | Esc browse profiles | h prev view or leave on Show | j/k or Up/Down scroll detail | Tab or l or Left/Right switch view | PgUp/PgDn/Home/End scroll detail | e edit | n new | d delete | c compile deps | a apply | u undo | r refresh"
    } else {
        "Keys: q quit | Enter inspect detail | j/k or Up/Down move profiles | Tab or h/l or Left/Right switch view | PgUp/PgDn/Home/End scroll detail | e edit | n new | d delete | c compile deps | a apply | u undo | r refresh"
    }
}

#[derive(Debug, Clone, Copy)]
struct ScrollbarMetrics {
    total_items: usize,
    visible_items: usize,
    offset: usize,
}

impl ScrollbarMetrics {
    fn new(total_items: usize, visible_items: usize, offset: usize) -> Option<Self> {
        if visible_items == 0 || total_items <= visible_items {
            return None;
        }

        Some(Self {
            total_items,
            visible_items,
            offset,
        })
    }
}

fn split_scrollbar_column(area: Rect, show_scrollbar: bool) -> (Rect, Rect) {
    if !show_scrollbar || area.width <= 1 {
        return (area, Rect::new(area.right(), area.y, 0, area.height));
    }

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);
    (chunks[0], chunks[1])
}

fn render_vertical_scrollbar(buf: &mut Buffer, area: Rect, metrics: Option<ScrollbarMetrics>) {
    let Some(metrics) = metrics else {
        return;
    };
    if area.width == 0 || area.height == 0 {
        return;
    }

    let track_len = area.height as usize;
    let thumb_len = ((metrics.visible_items * track_len).div_ceil(metrics.total_items)).max(1);
    let max_offset = metrics.total_items.saturating_sub(metrics.visible_items);
    let max_thumb_start = track_len.saturating_sub(thumb_len);
    let thumb_start = if max_offset == 0 {
        0
    } else {
        metrics.offset.min(max_offset) * max_thumb_start / max_offset
    };

    for row in 0..track_len {
        let symbol = if row >= thumb_start && row < thumb_start + thumb_len {
            "#"
        } else {
            ":"
        };
        let style = if symbol == "#" {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        buf[(area.x, area.y + row as u16)]
            .set_symbol(symbol)
            .set_style(style);
    }
}

fn detail_title(app: &TuiApp, scroll: u16, visible_lines: usize, total_lines: usize) -> String {
    let label = if app.detail_focused() {
        "Detail [Inspect]"
    } else {
        "Detail [Preview]"
    };

    if visible_lines == 0 || total_lines <= visible_lines {
        return label.to_string();
    }

    let first_line = scroll as usize + 1;
    let last_line = (scroll as usize + visible_lines).min(total_lines);
    format!("{label} {first_line}-{last_line}/{total_lines}")
}

fn render_editor_detail(
    area: Rect,
    buf: &mut ratatui::buffer::Buffer,
    editor: &ProfileEditorState,
) {
    let lines = vec![
        editor_line(
            editor.selected_field == EditorField::Name,
            "name",
            &editor.draft.name,
        ),
        editor_line(
            editor.selected_field == EditorField::SourceRoot,
            "source_root",
            editor.draft.source_root.as_deref().unwrap_or("<default>"),
        ),
        editor_line(
            editor.selected_field == EditorField::Requires,
            "requires",
            &format!("{} entries", editor.draft.requires.len()),
        ),
        editor_line(
            editor.selected_field == EditorField::Rules,
            "rules",
            &format!("{} entries", editor.draft.rules.len()),
        ),
        Line::from(""),
        Line::from("Press Enter to edit the selected field."),
        Line::from("List fields open focused collection editors."),
        Line::from("Press 's' to save or Esc to leave edit mode."),
    ];

    let paragraph = Paragraph::new(Text::from(lines))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Profile Editor"),
        )
        .wrap(Wrap { trim: false });
    paragraph.render(area, buf);
}

pub(super) fn render_show_text(view: ProfileShowView) -> Text<'static> {
    let mut lines = vec![
        Line::from(format!("Profile '{}'", view.profile_name)),
        Line::from(format!("source_root: {}", view.source_root)),
        Line::from(format!(
            "rules={} enabled={} disabled={}",
            view.rule_count, view.enabled_rule_count, view.disabled_rule_count
        )),
    ];

    if view.required_compositions.is_empty() {
        lines.push(Line::from("requires: none"));
    } else {
        lines.push(Line::from("requires:"));
        for requirement in view.required_compositions {
            lines.push(Line::from(vec![
                Span::raw(format!("  {} [", requirement.name)),
                Span::styled(
                    requirement.status.clone(),
                    style_for_requirement_status(&requirement.status),
                ),
                Span::raw(format!(
                    "] {} ({})",
                    requirement.output, requirement.message
                )),
            ]));
        }
    }

    for rule in view.rules {
        lines.push(Line::from(format!(
            "rule {} [{}] {}",
            rule.index,
            rule.mode,
            if rule.enabled { "enabled" } else { "disabled" }
        )));
        lines.push(Line::from(format!("  select {}", rule.select)));
        for destination in rule.destinations {
            lines.push(Line::from(format!("  to {destination}")));
        }
        if !rule.tags.is_empty() {
            lines.push(Line::from(format!("  tags {}", rule.tags.join(","))));
        }
        if let Some(note) = rule.note {
            lines.push(Line::from(format!("  note {note}")));
        }
    }

    Text::from(lines)
}

pub(super) fn render_plan_text(view: ProfileExplainView) -> Text<'static> {
    let mut lines = vec![
        Line::from(format!("Explain '{}'", view.profile_name)),
        Line::from(format!("source_root: {}", view.source_root)),
    ];

    if view.required_compositions.is_empty() {
        lines.push(Line::from("required compositions: none"));
    } else {
        lines.push(Line::from("required compositions:"));
        for requirement in view.required_compositions {
            lines.push(Line::from(vec![
                Span::raw(format!("  {} [", requirement.name)),
                Span::styled(
                    requirement.status.clone(),
                    style_for_requirement_status(&requirement.status),
                ),
                Span::raw(format!(
                    "] {} ({})",
                    requirement.output, requirement.message
                )),
            ]));
        }
    }

    if view.diagnostics.is_empty() {
        lines.push(Line::from("diagnostics: none"));
    } else {
        lines.push(Line::from("diagnostics:"));
        for diagnostic in view.diagnostics {
            lines.push(Line::from(vec![
                Span::raw("  ["),
                Span::styled(diagnostic.code, Style::default().fg(Color::Yellow)),
                Span::raw(format!("] {}", diagnostic.message)),
            ]));
        }
    }

    let mut summary_spans = vec![Span::raw("plan summary: ")];
    for (index, action) in ["create", "update", "remove", "skip", "warning", "danger"]
        .into_iter()
        .enumerate()
    {
        if index > 0 {
            summary_spans.push(Span::raw(" "));
        }
        summary_spans.push(Span::styled(
            format!(
                "{action}={}",
                view.plan_summary.get(action).copied().unwrap_or_default()
            ),
            style_for_action_name(action),
        ));
    }
    lines.push(Line::from(summary_spans));

    if view.plan_items.is_empty() {
        lines.push(Line::from("plan items: none"));
    } else {
        lines.push(Line::from("plan items:"));
        for item in view.plan_items {
            match item.desired_source {
                Some(source) => lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        action_label(&item.action, item.forceable),
                        style_for_action_name(&item.action),
                    ),
                    Span::raw(format!(
                        " {} <- {} ({})",
                        item.target,
                        format_path_under_root(&view.source_root, &source),
                        item.reason
                    )),
                ])),
                None => lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        action_label(&item.action, item.forceable),
                        style_for_action_name(&item.action),
                    ),
                    Span::raw(format!(" {} ({})", item.target, item.reason)),
                ])),
            }
        }
    }

    Text::from(lines)
}

fn format_path_under_root(root: &str, path: &str) -> String {
    let root = Path::new(root);
    let path = Path::new(path);

    match path.strip_prefix(root) {
        Ok(relative) if relative.as_os_str().is_empty() => ".".to_string(),
        Ok(relative) => format!("./{}", relative.display()),
        Err(_) => path.display().to_string(),
    }
}

pub(super) fn render_doctor_text(view: String) -> Text<'static> {
    let has_issue = view.contains("issues for");
    let style = if has_issue {
        Style::default().fg(Color::Red)
    } else {
        Style::default().fg(Color::Green)
    };
    let lines = view
        .lines()
        .map(|line| Line::from(Span::styled(line.to_string(), style)))
        .collect::<Vec<_>>();
    Text::from(lines)
}

fn style_for_requirement_status(status: &str) -> Style {
    match status {
        "ready" => Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
        "stale" => Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
        "missing" => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        _ => Style::default(),
    }
}

fn style_for_action_name(action: &str) -> Style {
    match action {
        "create" | "skip" => Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
        "update" | "remove" | "warning" => Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
        "danger" => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        _ => Style::default(),
    }
}

fn style_for_message(message: &str) -> Style {
    if message.starts_with("Error:") || message.contains("danger") {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    } else if message.contains("Compiled")
        || message.contains("Applied")
        || message.contains("Saved")
        || message.contains("Deleted")
        || message == "Ready"
    {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Yellow)
    }
}

fn action_label(action: &str, forceable: bool) -> String {
    if action == "danger" && forceable {
        format!("{:<7}", "danger*")
    } else {
        format!("{:<7}", action)
    }
}
