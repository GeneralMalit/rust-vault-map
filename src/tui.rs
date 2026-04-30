use std::env;
use std::io;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Result};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::{Frame, Terminal};

use crate::scan::{PhaseState, ScanOutcome};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum DashboardAction {
    None,
    Export,
    Edit,
    Quit,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum EditorOs {
    Windows,
    Unix,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct EditorCommand {
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum DashboardFocus {
    Sidebar,
    Content,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum DashboardView {
    Overview,
    BrokenLinks,
    AmbiguousLinks,
    Orphans,
    Stale,
    Hubs,
    Clusters,
    Suggestions,
    Help,
}

impl DashboardView {
    const ALL: [Self; 9] = [
        Self::Overview,
        Self::BrokenLinks,
        Self::AmbiguousLinks,
        Self::Orphans,
        Self::Stale,
        Self::Hubs,
        Self::Clusters,
        Self::Suggestions,
        Self::Help,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::BrokenLinks => "Broken links",
            Self::AmbiguousLinks => "Ambiguous links",
            Self::Orphans => "Orphan notes",
            Self::Stale => "Stale notes",
            Self::Hubs => "Hub notes",
            Self::Clusters => "Clusters",
            Self::Suggestions => "Suggested links",
            Self::Help => "Help",
        }
    }
}

pub struct DashboardApp {
    outcome: ScanOutcome,
    active_view: DashboardView,
    focus: DashboardFocus,
    selected_rows: [usize; 9],
    export_status: Option<String>,
    should_quit: bool,
}

impl DashboardApp {
    pub fn new(outcome: ScanOutcome) -> Self {
        Self {
            outcome,
            active_view: DashboardView::Overview,
            focus: DashboardFocus::Sidebar,
            selected_rows: [0; 9],
            export_status: None,
            should_quit: false,
        }
    }

    pub fn active_view(&self) -> DashboardView {
        self.active_view
    }

    pub fn focus(&self) -> DashboardFocus {
        self.focus
    }

    pub fn set_active_view(&mut self, view: DashboardView) {
        self.active_view = view;
        self.clamp_selected_row();
    }

    pub fn next_view(&mut self) {
        let index = self.view_index();
        self.set_active_view(DashboardView::ALL[(index + 1) % DashboardView::ALL.len()]);
    }

    pub fn previous_view(&mut self) {
        let index = self.view_index();
        let next = if index == 0 {
            DashboardView::ALL.len() - 1
        } else {
            index - 1
        };
        self.set_active_view(DashboardView::ALL[next]);
    }

    pub fn select_next(&mut self) {
        let max = self.active_items().len().saturating_sub(1);
        let index = self.view_index();
        self.selected_rows[index] = (self.selected_rows[index] + 1).min(max);
    }

    pub fn select_previous(&mut self) {
        let index = self.view_index();
        self.selected_rows[index] = self.selected_rows[index].saturating_sub(1);
    }

    pub fn selected_row(&self) -> usize {
        self.selected_rows[self.view_index()]
    }

    pub fn visible_item_range(&self, visible_rows: usize) -> Range<usize> {
        visible_item_range(self.selected_row(), self.active_items().len(), visible_rows)
    }

    pub fn selected_edit_target(&self) -> Option<String> {
        match self.active_view {
            DashboardView::BrokenLinks => self
                .outcome
                .graph
                .broken_links
                .get(self.selected_row())
                .map(|link| link.source_path.clone()),
            DashboardView::AmbiguousLinks => self
                .outcome
                .graph
                .ambiguous_links
                .get(self.selected_row())
                .map(|link| link.source_path.clone()),
            DashboardView::Orphans => self
                .outcome
                .analysis
                .orphan_notes
                .get(self.selected_row())
                .cloned(),
            DashboardView::Stale => self
                .outcome
                .analysis
                .stale_notes
                .get(self.selected_row())
                .map(|note| note.path.clone()),
            DashboardView::Hubs => self
                .outcome
                .analysis
                .hub_notes
                .get(self.selected_row())
                .map(|hub| hub.path.clone()),
            DashboardView::Clusters => self
                .outcome
                .analysis
                .clusters
                .get(self.selected_row())
                .and_then(|cluster| cluster.notes.first().cloned()),
            DashboardView::Suggestions => self
                .outcome
                .analysis
                .suggested_links
                .get(self.selected_row())
                .map(|link| link.source_path.clone()),
            DashboardView::Overview | DashboardView::Help => None,
        }
    }

    pub fn selected_edit_path(&self) -> Option<PathBuf> {
        self.selected_edit_target()
            .map(|target| Path::new(&self.outcome.vault_path).join(target))
    }

    pub fn set_export_status(&mut self, status: String) {
        self.export_status = Some(status);
    }

    pub fn export_status(&self) -> Option<&str> {
        self.export_status.as_deref()
    }

    pub fn request_quit(&mut self) {
        self.should_quit = true;
    }

    pub fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> DashboardAction {
        match (code, modifiers) {
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                self.request_quit();
                DashboardAction::Quit
            }
            (KeyCode::Esc, _) if self.focus == DashboardFocus::Content => {
                self.focus = DashboardFocus::Sidebar;
                DashboardAction::None
            }
            (KeyCode::Char('q') | KeyCode::Esc, _) => {
                self.request_quit();
                DashboardAction::Quit
            }
            (KeyCode::Down, _) if self.focus == DashboardFocus::Sidebar => {
                self.next_view();
                DashboardAction::None
            }
            (KeyCode::Up, _) if self.focus == DashboardFocus::Sidebar => {
                self.previous_view();
                DashboardAction::None
            }
            (KeyCode::Right | KeyCode::Enter, _) if self.focus == DashboardFocus::Sidebar => {
                self.focus = DashboardFocus::Content;
                DashboardAction::None
            }
            (KeyCode::Left, _) if self.focus == DashboardFocus::Content => {
                self.focus = DashboardFocus::Sidebar;
                DashboardAction::None
            }
            (KeyCode::Down, _) if self.focus == DashboardFocus::Content => {
                self.select_next();
                DashboardAction::None
            }
            (KeyCode::Up, _) if self.focus == DashboardFocus::Content => {
                self.select_previous();
                DashboardAction::None
            }
            (KeyCode::Char('h') | KeyCode::F(1), _) => {
                self.set_active_view(DashboardView::Help);
                self.focus = DashboardFocus::Content;
                DashboardAction::None
            }
            (KeyCode::Enter, _) if self.focus == DashboardFocus::Content => DashboardAction::Edit,
            (KeyCode::Char('o'), _) => DashboardAction::Edit,
            (KeyCode::Char('e') | KeyCode::Char('r'), _) => DashboardAction::Export,
            _ => DashboardAction::None,
        }
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) -> DashboardAction {
        match key.kind {
            KeyEventKind::Press | KeyEventKind::Repeat => self.handle_key(key.code, key.modifiers),
            KeyEventKind::Release => DashboardAction::None,
        }
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn outcome(&self) -> &ScanOutcome {
        &self.outcome
    }

    fn view_index(&self) -> usize {
        DashboardView::ALL
            .iter()
            .position(|view| *view == self.active_view)
            .unwrap_or(0)
    }

    fn clamp_selected_row(&mut self) {
        let max = self.active_items().len().saturating_sub(1);
        let index = self.view_index();
        self.selected_rows[index] = self.selected_rows[index].min(max);
    }

    fn active_items(&self) -> Vec<String> {
        match self.active_view {
            DashboardView::Overview | DashboardView::Help => Vec::new(),
            DashboardView::BrokenLinks => self
                .outcome
                .graph
                .broken_links
                .iter()
                .map(|link| format!("{} -> {}", link.source_path, link.target))
                .collect(),
            DashboardView::AmbiguousLinks => self
                .outcome
                .graph
                .ambiguous_links
                .iter()
                .map(|link| format!("{} -> {}", link.source_path, link.target))
                .collect(),
            DashboardView::Orphans => self.outcome.analysis.orphan_notes.clone(),
            DashboardView::Stale => self
                .outcome
                .analysis
                .stale_notes
                .iter()
                .map(|note| note.path.clone())
                .collect(),
            DashboardView::Hubs => self
                .outcome
                .analysis
                .hub_notes
                .iter()
                .map(|hub| format!("{} (in: {}, out: {})", hub.path, hub.inbound, hub.outbound))
                .collect(),
            DashboardView::Clusters => self
                .outcome
                .analysis
                .clusters
                .iter()
                .map(|cluster| format!("{} notes", cluster.notes.len()))
                .collect(),
            DashboardView::Suggestions => self
                .outcome
                .analysis
                .suggested_links
                .iter()
                .map(|link| {
                    format!(
                        "{}: {} -> {}",
                        link.source_path, link.mention, link.target_path
                    )
                })
                .collect(),
        }
    }
}

pub fn run_dashboard(
    outcome: ScanOutcome,
    export_markdown: impl Fn(&ScanOutcome) -> Result<String>,
) -> Result<()> {
    let mut terminal = TerminalSession::enter()?;
    let mut app = DashboardApp::new(outcome);

    loop {
        terminal
            .terminal
            .draw(|frame| render_dashboard(frame, &app))?;

        if app.should_quit() {
            break;
        }

        if let Event::Key(key) = event::read()? {
            match app.handle_key_event(key) {
                DashboardAction::Export => match export_markdown(app.outcome()) {
                    Ok(path) => app.set_export_status(format!("Report written to {path}")),
                    Err(error) => app.set_export_status(format!("Export failed: {error}")),
                },
                DashboardAction::Edit => {
                    let status = match app.selected_edit_path() {
                        Some(path) => match terminal.open_editor(&path) {
                            Ok(()) => format!("Opened {}", path.display()),
                            Err(error) => format!("Edit failed: {error}"),
                        },
                        None => "No Markdown file is selected for editing.".to_string(),
                    };
                    app.set_export_status(status);
                }
                DashboardAction::Quit | DashboardAction::None => {}
            }
        }
    }

    Ok(())
}

struct TerminalSession {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    active: bool,
}

impl TerminalSession {
    fn enter() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        Ok(Self {
            terminal: Terminal::new(backend)?,
            active: true,
        })
    }

    fn suspend(&mut self) -> Result<()> {
        if self.active {
            disable_raw_mode()?;
            execute!(self.terminal.backend_mut(), LeaveAlternateScreen)?;
            self.terminal.show_cursor()?;
            self.active = false;
        }
        Ok(())
    }

    fn resume(&mut self) -> Result<()> {
        if !self.active {
            enable_raw_mode()?;
            execute!(self.terminal.backend_mut(), EnterAlternateScreen)?;
            self.terminal.clear()?;
            self.active = true;
        }
        Ok(())
    }

    fn open_editor(&mut self, path: &Path) -> Result<()> {
        self.suspend()?;
        let result = open_path_in_editor(path);
        let resume_result = self.resume();
        result?;
        resume_result
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        if self.active {
            let _ = disable_raw_mode();
            let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
            let _ = self.terminal.show_cursor();
        }
    }
}

pub fn resolve_editor_command(
    visual: Option<&str>,
    editor: Option<&str>,
    os: EditorOs,
) -> EditorCommand {
    visual
        .filter(|value| !value.trim().is_empty())
        .or_else(|| editor.filter(|value| !value.trim().is_empty()))
        .map(parse_editor_command)
        .unwrap_or_else(|| match os {
            EditorOs::Windows => EditorCommand {
                program: "notepad".to_string(),
                args: Vec::new(),
            },
            EditorOs::Unix => EditorCommand {
                program: "nano".to_string(),
                args: Vec::new(),
            },
        })
}

fn resolve_editor_from_env() -> EditorCommand {
    resolve_editor_command(
        env::var("VISUAL").ok().as_deref(),
        env::var("EDITOR").ok().as_deref(),
        current_os(),
    )
}

fn current_os() -> EditorOs {
    if cfg!(windows) {
        EditorOs::Windows
    } else {
        EditorOs::Unix
    }
}

fn parse_editor_command(value: &str) -> EditorCommand {
    let mut parts = value.split_whitespace();
    let program = parts.next().unwrap_or(value).to_string();
    let args = parts.map(str::to_string).collect();

    EditorCommand { program, args }
}

fn open_path_in_editor(path: &Path) -> Result<()> {
    let editor = resolve_editor_from_env();
    let status = Command::new(&editor.program)
        .args(&editor.args)
        .arg(path)
        .status()?;
    if !status.success() {
        bail!("editor exited with status {status}");
    }
    Ok(())
}

fn render_dashboard(frame: &mut Frame<'_>, app: &DashboardApp) {
    let area = frame.area();
    let shell = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(2),
        ])
        .split(area);

    render_header(frame, shell[0], app);
    render_body(frame, shell[1], app);
    render_footer(frame, shell[2], app);
}

fn render_header(frame: &mut Frame<'_>, area: Rect, app: &DashboardApp) {
    let title = Line::from(vec![
        Span::styled("rust-vault-map", Theme::title()),
        Span::raw("  "),
        Span::styled(&app.outcome.vault_path, Theme::muted()),
    ]);
    frame.render_widget(
        Paragraph::new(title).block(Block::default().borders(Borders::BOTTOM)),
        area,
    );
}

fn render_body(frame: &mut Frame<'_>, area: Rect, app: &DashboardApp) {
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(22), Constraint::Min(20)])
        .split(area);

    render_sidebar(frame, body[0], app);
    match app.active_view {
        DashboardView::Overview => render_overview(frame, body[1], app),
        DashboardView::Help => render_help(frame, body[1], app),
        _ => render_finding_view(frame, body[1], app),
    }
}

fn render_sidebar(frame: &mut Frame<'_>, area: Rect, app: &DashboardApp) {
    let items = DashboardView::ALL
        .iter()
        .map(|view| {
            let style = if *view == app.active_view {
                Theme::selected()
            } else {
                Theme::normal()
            };
            ListItem::new(Line::styled(view.label(), style))
        })
        .collect::<Vec<_>>();

    frame.render_widget(
        List::new(items).block(focused_block(
            "Sections",
            app.focus == DashboardFocus::Sidebar,
        )),
        area,
    );
}

fn render_overview(frame: &mut Frame<'_>, area: Rect, app: &DashboardApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(8)])
        .split(area);

    let metrics = vec![
        Line::from(vec![
            Span::styled("Notes", Theme::muted()),
            Span::raw(format!(" {}", app.outcome.note_count)),
        ]),
        Line::from(vec![
            Span::styled("Folders", Theme::muted()),
            Span::raw(format!(" {}", app.outcome.folder_count)),
        ]),
        Line::from(vec![
            Span::styled("Links", Theme::muted()),
            Span::raw(format!(" {}", app.outcome.wiki_link_count)),
        ]),
        Line::from(vec![
            Span::styled("Broken", Theme::warning()),
            Span::raw(format!(" {}", app.outcome.graph.broken_links.len())),
        ]),
        Line::from(vec![
            Span::styled("Orphans", Theme::danger()),
            Span::raw(format!(" {}", app.outcome.analysis.orphan_notes.len())),
        ]),
    ];
    frame.render_widget(
        Paragraph::new(metrics).block(focused_block(
            "Vault health",
            app.focus == DashboardFocus::Content,
        )),
        chunks[0],
    );

    let mut lines = app
        .outcome
        .phases
        .iter()
        .map(|phase| {
            let marker = match phase.state {
                PhaseState::Complete => Span::styled("✓", Theme::success()),
                PhaseState::Attention => Span::styled("!", Theme::warning()),
            };
            Line::from(vec![
                marker,
                Span::raw(" "),
                Span::styled(phase.phase.label(), Theme::normal()),
                Span::raw("  "),
                Span::styled(&phase.detail, Theme::muted()),
            ])
        })
        .collect::<Vec<_>>();
    lines.push(Line::raw(""));
    lines.push(Line::styled(next_action(app.outcome()), Theme::warning()));
    if let Some(status) = app.export_status() {
        lines.push(Line::raw(""));
        lines.push(Line::styled(status, Theme::success()));
    }

    frame.render_widget(
        Paragraph::new(lines)
            .block(focused_block(
                "Scan flow",
                app.focus == DashboardFocus::Content,
            ))
            .wrap(Wrap { trim: true }),
        chunks[1],
    );
}

fn render_finding_view(frame: &mut Frame<'_>, area: Rect, app: &DashboardApp) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(52), Constraint::Percentage(48)])
        .split(area);

    let items = app.active_items();
    let selected = app.selected_row();
    let list_items = if items.is_empty() {
        vec![ListItem::new(Line::styled(
            "No findings in this section.",
            Theme::muted(),
        ))]
    } else {
        items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let style = if index == selected {
                    Theme::selected()
                } else {
                    Theme::normal()
                };
                ListItem::new(Line::styled(item.as_str(), style))
            })
            .collect()
    };

    let visible_rows = chunks[0].height.saturating_sub(2) as usize;
    let visible_range = app.visible_item_range(visible_rows);
    let mut state = ListState::default()
        .with_offset(visible_range.start)
        .with_selected((!items.is_empty()).then_some(selected));

    frame.render_stateful_widget(
        List::new(list_items)
            .block(focused_block(
                app.active_view.label(),
                app.focus == DashboardFocus::Content,
            ))
            .highlight_style(Theme::selected()),
        chunks[0],
        &mut state,
    );

    let detail = detail_lines(app);
    frame.render_widget(
        Paragraph::new(detail)
            .block(focused_block(
                "Details",
                app.focus == DashboardFocus::Content,
            ))
            .wrap(Wrap { trim: true }),
        chunks[1],
    );
}

fn render_help(frame: &mut Frame<'_>, area: Rect, app: &DashboardApp) {
    let lines = vec![
        Line::styled("Keyboard", Theme::title()),
        Line::raw("Sidebar: Up / Down moves sections"),
        Line::raw("Sidebar: Right / Enter inspects a section"),
        Line::raw("Content: Up / Down moves findings"),
        Line::raw("Content: Left returns to sections"),
        Line::raw("Content: Enter or o opens selected Markdown file"),
        Line::raw("e or r: export Markdown report"),
        Line::raw("q or Esc: quit"),
        Line::raw(""),
        Line::styled("Output modes", Theme::title()),
        Line::raw("TTY scan opens this dashboard."),
        Line::raw("--plain forces deterministic text output."),
        Line::raw("--report writes Markdown and exits."),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .block(focused_block("Help", app.focus == DashboardFocus::Content))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, app: &DashboardApp) {
    let default_status = match app.focus {
        DashboardFocus::Sidebar => {
            "↑/↓ sections · Right/Enter inspect · h help · e export · q quit"
        }
        DashboardFocus::Content => "↑/↓ items · Enter/o edit note · Left back · e export · q quit",
    };
    let status = app.export_status().unwrap_or(default_status);
    frame.render_widget(
        Paragraph::new(Line::styled(status, Theme::muted()))
            .block(Block::default().borders(Borders::TOP)),
        area,
    );
}

fn focused_block(title: &str, focused: bool) -> Block<'_> {
    let style = if focused {
        Theme::title()
    } else {
        Theme::muted()
    };

    Block::default()
        .title(Span::styled(title.to_string(), style))
        .border_style(style)
        .borders(Borders::ALL)
}

fn visible_item_range(selected: usize, total: usize, visible_rows: usize) -> Range<usize> {
    if total == 0 || visible_rows == 0 {
        return 0..0;
    }

    let selected = selected.min(total - 1);
    let visible_rows = visible_rows.min(total);
    let start = selected.saturating_add(1).saturating_sub(visible_rows);
    let end = (start + visible_rows).min(total);
    start..end
}

fn detail_lines(app: &DashboardApp) -> Vec<Line<'static>> {
    match app.active_view {
        DashboardView::BrokenLinks => app
            .outcome
            .graph
            .broken_links
            .get(app.selected_row())
            .map(|link| {
                vec![
                    Line::styled("Broken link", Theme::warning()),
                    Line::raw(format!("Source: {}", link.source_path)),
                    Line::raw(format!("Target: {}", link.target)),
                    Line::raw("Next action: create the target note or update the link text."),
                ]
            })
            .unwrap_or_else(empty_detail),
        DashboardView::AmbiguousLinks => app
            .outcome
            .graph
            .ambiguous_links
            .get(app.selected_row())
            .map(|link| {
                vec![
                    Line::styled("Ambiguous link", Theme::warning()),
                    Line::raw(format!("Source: {}", link.source_path)),
                    Line::raw(format!("Target: {}", link.target)),
                    Line::raw(format!("Candidates: {}", link.candidates.join(", "))),
                    Line::raw("Next action: use a folder-qualified link."),
                ]
            })
            .unwrap_or_else(empty_detail),
        DashboardView::Orphans => app
            .outcome
            .analysis
            .orphan_notes
            .get(app.selected_row())
            .map(|path| {
                vec![
                    Line::styled("Orphan note", Theme::danger()),
                    Line::raw(format!("Path: {path}")),
                    Line::raw("Next action: add meaningful inbound or outbound links."),
                ]
            })
            .unwrap_or_else(empty_detail),
        DashboardView::Stale => app
            .outcome
            .analysis
            .stale_notes
            .get(app.selected_row())
            .map(|note| {
                vec![
                    Line::styled("Stale note", Theme::warning()),
                    Line::raw(format!("Path: {}", note.path)),
                    Line::raw(format!(
                        "Threshold: {} days",
                        app.outcome.stale_threshold_days
                    )),
                    Line::raw("Next action: review whether the note is still useful."),
                ]
            })
            .unwrap_or_else(empty_detail),
        DashboardView::Hubs => app
            .outcome
            .analysis
            .hub_notes
            .get(app.selected_row())
            .map(|hub| {
                vec![
                    Line::styled("Hub note", Theme::title()),
                    Line::raw(format!("Path: {}", hub.path)),
                    Line::raw(format!("Inbound: {}", hub.inbound)),
                    Line::raw(format!("Outbound: {}", hub.outbound)),
                ]
            })
            .unwrap_or_else(empty_detail),
        DashboardView::Clusters => app
            .outcome
            .analysis
            .clusters
            .get(app.selected_row())
            .map(|cluster| {
                vec![
                    Line::styled("Cluster", Theme::title()),
                    Line::raw(format!("Notes: {}", cluster.notes.len())),
                    Line::raw(format!(
                        "Samples: {}",
                        cluster
                            .notes
                            .iter()
                            .take(5)
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(", ")
                    )),
                ]
            })
            .unwrap_or_else(empty_detail),
        DashboardView::Suggestions => app
            .outcome
            .analysis
            .suggested_links
            .get(app.selected_row())
            .map(|link| {
                vec![
                    Line::styled("Suggested link", Theme::title()),
                    Line::raw(format!("Source: {}", link.source_path)),
                    Line::raw(format!("Mention: {}", link.mention)),
                    Line::raw(format!("Target: {}", link.target_path)),
                    Line::raw("Next action: add a wiki link if the mention is intentional."),
                ]
            })
            .unwrap_or_else(empty_detail),
        DashboardView::Overview | DashboardView::Help => empty_detail(),
    }
}

fn empty_detail() -> Vec<Line<'static>> {
    vec![
        Line::styled("Nothing selected", Theme::muted()),
        Line::raw("This section has no findings to inspect."),
    ]
}

fn next_action(outcome: &ScanOutcome) -> &'static str {
    if !outcome.graph.broken_links.is_empty() {
        "Next action: fix broken links first."
    } else if !outcome.graph.ambiguous_links.is_empty() {
        "Next action: resolve ambiguous links."
    } else if !outcome.analysis.orphan_notes.is_empty() {
        "Next action: review orphan notes."
    } else if !outcome.analysis.suggested_links.is_empty() {
        "Next action: review suggested links."
    } else {
        "Next action: vault graph looks tidy."
    }
}

struct Theme;

impl Theme {
    fn title() -> Style {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    }

    fn normal() -> Style {
        Style::default().fg(Color::Gray)
    }

    fn muted() -> Style {
        Style::default().fg(Color::DarkGray)
    }

    fn selected() -> Style {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    }

    fn success() -> Style {
        Style::default().fg(Color::Green)
    }

    fn warning() -> Style {
        Style::default().fg(Color::Yellow)
    }

    fn danger() -> Style {
        Style::default().fg(Color::Red)
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use ratatui::backend::TestBackend;

    use super::*;
    use crate::analysis::{Cluster, HubNote, StaleNote, SuggestedLink, VaultAnalysis};
    use crate::graph::{AmbiguousLink, BrokenLink, GraphNote, VaultGraph};
    use crate::parser::WikiLink;
    use crate::scan::{ScanPhase, ScanProgress};

    #[test]
    fn renders_overview_and_help_views() {
        let mut app = DashboardApp::new(sample_outcome());
        let overview = render_to_text(&app);

        assert!(overview.contains("rust-vault-map"));
        assert!(overview.contains("Vault health"));
        assert!(overview.contains("Scan flow"));
        assert!(overview.contains("Next action: fix broken links first."));

        app.set_active_view(DashboardView::Help);
        app.focus = DashboardFocus::Content;
        let help = render_to_text(&app);

        assert!(help.contains("Keyboard"));
        assert!(help.contains("Enter or o opens selected Markdown file"));
        assert!(help.contains("--plain forces deterministic text output."));
    }

    #[test]
    fn renders_all_finding_detail_views() {
        let cases = [
            (
                DashboardView::BrokenLinks,
                "Broken link",
                "Source: Index.md",
            ),
            (
                DashboardView::AmbiguousLinks,
                "Ambiguous link",
                "Candidates: Areas/Topic.md",
            ),
            (DashboardView::Orphans, "Orphan note", "Path: Orphan.md"),
            (DashboardView::Stale, "Stale note", "Path: Stale.md"),
            (DashboardView::Hubs, "Hub note", "Inbound: 4"),
            (DashboardView::Clusters, "Cluster", "Samples: Cluster/A.md"),
            (
                DashboardView::Suggestions,
                "Suggested link",
                "Mention: Suggestion",
            ),
        ];

        for (view, heading, detail) in cases {
            let mut app = DashboardApp::new(sample_outcome());
            app.set_active_view(view);
            app.focus = DashboardFocus::Content;

            let rendered = render_to_text(&app);

            assert!(rendered.contains(view.label()));
            assert!(rendered.contains(heading));
            assert!(rendered.contains(detail));
        }
    }

    #[test]
    fn renders_empty_finding_detail() {
        let mut outcome = sample_outcome();
        outcome.graph.broken_links.clear();
        let mut app = DashboardApp::new(outcome);
        app.set_active_view(DashboardView::BrokenLinks);
        app.focus = DashboardFocus::Content;

        let rendered = render_to_text(&app);

        assert!(rendered.contains("No findings in this section."));
        assert!(rendered.contains("Nothing selected"));
    }

    #[test]
    fn renders_scrolled_finding_list_with_selected_row_visible() {
        let mut outcome = sample_outcome();
        outcome.analysis.orphan_notes = (0..60)
            .map(|index| format!("Orphan-{index:02}.md"))
            .collect();
        let mut app = DashboardApp::new(outcome);
        app.set_active_view(DashboardView::Orphans);
        app.focus = DashboardFocus::Content;
        for _ in 0..45 {
            app.select_next();
        }

        let rendered = render_to_text(&app);

        assert!(rendered.contains("Orphan-45.md"));
        assert!(!rendered.contains("Orphan-00.md"));
    }

    #[test]
    fn helpers_cover_editor_parsing_and_ranges() {
        let parsed = parse_editor_command("code --wait --reuse-window");

        assert_eq!(parsed.program, "code");
        assert_eq!(parsed.args, ["--wait", "--reuse-window"]);
        let expected_os = if cfg!(windows) {
            EditorOs::Windows
        } else {
            EditorOs::Unix
        };
        assert_eq!(current_os(), expected_os);
        assert_eq!(visible_item_range(0, 0, 4), 0..0);
        assert_eq!(visible_item_range(9, 10, 4), 6..10);
    }

    fn render_to_text(app: &DashboardApp) -> String {
        let backend = TestBackend::new(120, 32);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| render_dashboard(frame, app))
            .expect("draw");

        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<Vec<_>>()
            .join("")
    }

    fn sample_outcome() -> ScanOutcome {
        ScanOutcome {
            vault_path: "D:/Notes/Home".to_string(),
            note_count: 8,
            folder_count: 2,
            wiki_link_count: 5,
            skipped_file_count: 0,
            stale_threshold_days: 90,
            graph: VaultGraph {
                notes: vec![note("Index.md"), note("Rust.md")],
                edges: Vec::new(),
                broken_links: vec![BrokenLink {
                    source_path: "Index.md".to_string(),
                    target: "Missing".to_string(),
                }],
                ambiguous_links: vec![AmbiguousLink {
                    source_path: "Index.md".to_string(),
                    target: "Topic".to_string(),
                    candidates: vec![
                        "Areas/Topic.md".to_string(),
                        "Projects/Topic.md".to_string(),
                    ],
                }],
            },
            analysis: VaultAnalysis {
                orphan_notes: vec!["Orphan.md".to_string()],
                stale_notes: vec![StaleNote {
                    path: "Stale.md".to_string(),
                    modified: SystemTime::UNIX_EPOCH,
                }],
                oldest_note_age_days: Some(100),
                hub_notes: vec![HubNote {
                    path: "Hub.md".to_string(),
                    inbound: 4,
                    outbound: 2,
                }],
                clusters: vec![Cluster {
                    notes: vec!["Cluster/A.md".to_string(), "Cluster/B.md".to_string()],
                }],
                suggested_links: vec![SuggestedLink {
                    source_path: "Index.md".to_string(),
                    target_path: "Suggestion.md".to_string(),
                    mention: "Suggestion".to_string(),
                }],
            },
            phases: vec![
                ScanProgress {
                    phase: ScanPhase::DiscoverNotes,
                    state: PhaseState::Complete,
                    detail: "8 notes".to_string(),
                },
                ScanProgress {
                    phase: ScanPhase::BuildGraph,
                    state: PhaseState::Attention,
                    detail: "1 broken".to_string(),
                },
            ],
        }
    }

    fn note(path: &str) -> GraphNote {
        GraphNote {
            path: path.to_string(),
            title: path.trim_end_matches(".md").to_string(),
            body: String::new(),
            links: vec![WikiLink {
                target: "Rust".to_string(),
                alias: None,
                raw: "[[Rust]]".to_string(),
            }],
            modified: Some(SystemTime::UNIX_EPOCH),
        }
    }
}
