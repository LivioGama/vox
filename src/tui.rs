//! Interactive TUI for human users to configure vox.
//!
//! Launched via `vox setup`. Provides a menu to select backend, voice, language,
//! style, STT settings, and test speech in real-time. AI agents use CLI flags instead.

use std::io::{self, Stdout};

use anyhow::{Context, Result};
use crossterm::ExecutableCommand;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::prelude::*;
use ratatui::widgets::*;

use crate::backend::{self, SpeakOptions};
use crate::config;
use crate::db;
use crate::tui_stt;

/// All screens in the TUI.
#[derive(Clone, Copy, PartialEq)]
enum Screen {
    Backend,
    Voice,
    Language,
    Style,
    Volume,
    SttThreshold,
    SttSilence,
    SttToggles,
    Test,
}

const VOLUME_PRESETS: &[&str] = &["0.5", "0.75", "1.0", "1.25", "1.5", "2.0", "3.0"];

struct App {
    screen: Screen,
    backends: Vec<&'static str>,
    backend_idx: usize,
    voices: Vec<String>,
    voice_idx: usize,
    languages: Vec<&'static str>,
    lang_idx: usize,
    styles: Vec<&'static str>,
    style_idx: usize,
    volume_idx: usize,
    stt_threshold_idx: usize,
    stt_silence_idx: usize,
    stt_toggle_idx: usize,
    stt_trim_silence: bool,
    stt_auto_enter: bool,
    status: String,
    should_quit: bool,
}

impl App {
    fn new() -> Result<Self> {
        let conn = db::open()?;
        let prefs = db::get_preferences(&conn)?;

        #[cfg(target_os = "macos")]
        let backends = vec![
            "say          \u{2605}\u{2605}\u{2605} quality  \u{26a1} 3s",
            "piper        \u{2605}\u{2605}  quality  \u{26a1} <1s  [Rust]",
            "qwen-native  \u{2605}\u{2605}\u{2605}\u{2605} quality  \u{26a1} 12s  [Rust+Metal]",
            "voxtream     \u{2605}\u{2605}\u{2605}\u{2605}\u{2605} quality  \u{26a1} 170ms [CUDA]",
            "qwen         \u{2605}\u{2605}\u{2605}\u{2605} quality  \u{26a1} 2s   [Python+MLX]",
        ];
        #[cfg(not(target_os = "macos"))]
        let backends = vec![
            "piper        \u{2605}\u{2605}  quality  \u{26a1} <1s  [Rust]",
            "qwen-native  \u{2605}\u{2605}\u{2605}\u{2605} quality  \u{26a1} 3s   [Rust+CUDA]",
            "voxtream     \u{2605}\u{2605}\u{2605}\u{2605}\u{2605} quality  \u{26a1} 170ms [CUDA]",
        ];

        let current_backend = prefs.backend.as_deref().unwrap_or(config::DEFAULT_BACKEND);
        let backend_idx = backends
            .iter()
            .position(|b| b.split_whitespace().next() == Some(current_backend))
            .unwrap_or(0);

        let languages: Vec<&str> = config::SUPPORTED_LANGS.to_vec();
        let lang_idx = prefs
            .lang
            .as_deref()
            .and_then(|l| languages.iter().position(|x| *x == l))
            .unwrap_or(0);

        let styles = vec![
            "(default)",
            "calm",
            "energetic",
            "warm",
            "authoritative",
            "cheerful",
            "serious",
        ];
        let style_idx = prefs
            .style
            .as_deref()
            .and_then(|s| styles.iter().position(|x| *x == s))
            .unwrap_or(0);

        let voices = Self::load_voices(backends[backend_idx]);

        let voice_idx = prefs
            .voice
            .as_deref()
            .and_then(|v| voices.iter().position(|x| x == v))
            .unwrap_or(0);

        let volume_idx = VOLUME_PRESETS.iter().position(|x| *x == "1.0").unwrap_or(2);
        let stt_threshold_idx = prefs
            .stt_threshold
            .and_then(|v| {
                tui_stt::THRESHOLD_PRESETS
                    .iter()
                    .position(|x| x.parse::<f64>().ok() == Some(v))
            })
            .unwrap_or(2);
        let stt_silence_idx = prefs
            .stt_silence
            .and_then(|v| {
                tui_stt::SILENCE_PRESETS
                    .iter()
                    .position(|x| x.parse::<f64>().ok() == Some(v))
            })
            .unwrap_or(2);
        let stt_trim_silence = prefs.stt_trim_silence.unwrap_or(true);
        let stt_auto_enter = prefs.stt_auto_enter.unwrap_or(false);

        Ok(Self {
            screen: Screen::Backend,
            backends,
            backend_idx,
            voices,
            voice_idx,
            languages,
            lang_idx,
            styles,
            style_idx,
            volume_idx,
            stt_threshold_idx,
            stt_silence_idx,
            stt_toggle_idx: 0,
            stt_trim_silence,
            stt_auto_enter,
            status: "Arrow keys to navigate, Enter to select, Tab to switch section, T to test, S to save, Q to quit  |  Enter on toggles to switch".into(),
            should_quit: false,
        })
    }

    fn load_voices(backend_name: &str) -> Vec<String> {
        backend::get_backend(backend_name)
            .and_then(|b| b.list_voices())
            .unwrap_or_else(|_| vec!["(default)".into()])
    }

    fn selected_backend(&self) -> &str {
        self.backends[self.backend_idx]
            .split_whitespace()
            .next()
            .unwrap_or("say")
    }

    fn selected_lang(&self) -> &str {
        self.languages[self.lang_idx]
    }

    fn selected_voice(&self) -> Option<&str> {
        let v = self.voices.get(self.voice_idx).map(|s| s.as_str())?;
        if v.starts_with('(') { None } else { Some(v) }
    }

    fn selected_style(&self) -> Option<&str> {
        let s = self.styles[self.style_idx];
        if s == "(default)" { None } else { Some(s) }
    }

    fn selected_volume(&self) -> f32 {
        VOLUME_PRESETS[self.volume_idx].parse().unwrap_or(1.0)
    }

    fn current_list_len(&self) -> usize {
        match self.screen {
            Screen::Backend => self.backends.len(),
            Screen::Voice => self.voices.len(),
            Screen::Language => self.languages.len(),
            Screen::Style => self.styles.len(),
            Screen::Volume => VOLUME_PRESETS.len(),
            Screen::SttThreshold => tui_stt::THRESHOLD_PRESETS.len(),
            Screen::SttSilence => tui_stt::SILENCE_PRESETS.len(),
            Screen::SttToggles => tui_stt::TOGGLE_COUNT,
            Screen::Test => 2,
        }
    }

    fn current_idx(&self) -> usize {
        match self.screen {
            Screen::Backend => self.backend_idx,
            Screen::Voice => self.voice_idx,
            Screen::Language => self.lang_idx,
            Screen::Style => self.style_idx,
            Screen::Volume => self.volume_idx,
            Screen::SttThreshold => self.stt_threshold_idx,
            Screen::SttSilence => self.stt_silence_idx,
            Screen::SttToggles => self.stt_toggle_idx,
            Screen::Test => 0,
        }
    }

    fn set_idx(&mut self, idx: usize) {
        match self.screen {
            Screen::Backend => {
                self.backend_idx = idx;
                let name = self.backends[idx]
                    .split_whitespace()
                    .next()
                    .unwrap_or("say");
                self.voices = Self::load_voices(name);
                self.voice_idx = 0;
            }
            Screen::Voice => self.voice_idx = idx,
            Screen::Language => self.lang_idx = idx,
            Screen::Style => self.style_idx = idx,
            Screen::Volume => self.volume_idx = idx,
            Screen::SttThreshold => self.stt_threshold_idx = idx,
            Screen::SttSilence => self.stt_silence_idx = idx,
            Screen::SttToggles => self.stt_toggle_idx = idx,
            Screen::Test => {}
        }
    }

    fn move_up(&mut self) {
        let idx = self.current_idx();
        if idx > 0 {
            self.set_idx(idx - 1);
        }
    }

    fn move_down(&mut self) {
        let idx = self.current_idx();
        let max = self.current_list_len();
        if idx + 1 < max {
            self.set_idx(idx + 1);
        }
    }

    fn next_screen(&mut self) {
        self.screen = match self.screen {
            Screen::Backend => Screen::Voice,
            Screen::Voice => Screen::Language,
            Screen::Language => Screen::Style,
            Screen::Style => Screen::Volume,
            Screen::Volume => Screen::SttThreshold,
            Screen::SttThreshold => Screen::SttSilence,
            Screen::SttSilence => Screen::SttToggles,
            Screen::SttToggles => Screen::Test,
            Screen::Test => Screen::Backend,
        };
    }

    fn prev_screen(&mut self) {
        self.screen = match self.screen {
            Screen::Backend => Screen::Test,
            Screen::Voice => Screen::Backend,
            Screen::Language => Screen::Voice,
            Screen::Style => Screen::Language,
            Screen::Volume => Screen::Style,
            Screen::SttThreshold => Screen::Volume,
            Screen::SttSilence => Screen::SttThreshold,
            Screen::SttToggles => Screen::SttSilence,
            Screen::Test => Screen::SttToggles,
        };
    }

    fn test_speak(&mut self) {
        self.status = format!("Speaking with {} ...", self.selected_backend());
        let opts = SpeakOptions {
            voice: self.selected_voice().map(String::from),
            lang: Some(self.selected_lang().to_string()),
            style: self.selected_style().map(String::from),
            volume: self.selected_volume(),
            ..Default::default()
        };
        let text = match self.selected_lang() {
            "fr" => "Bonjour, ceci est un test de synthese vocale.",
            "es" => "Hola, esta es una prueba de sintesis de voz.",
            "de" => "Hallo, dies ist ein Test der Sprachsynthese.",
            "ja" => "こんにちは、これは音声合成のテストです。",
            "zh" => "你好，这是语音合成测试。",
            _ => "Hello, this is a voice synthesis test.",
        };
        match backend::get_backend(self.selected_backend()) {
            Ok(b) => match b.speak(text, &opts) {
                Ok(()) => self.status = "Test complete.".into(),
                Err(e) => self.status = format!("Error: {e}"),
            },
            Err(e) => self.status = format!("Backend error: {e}"),
        }
    }

    fn save(&mut self) -> Result<()> {
        let conn = db::open()?;
        db::set_preference(&conn, "backend", self.selected_backend())?;
        db::set_preference(&conn, "lang", self.selected_lang())?;
        if let Some(v) = self.selected_voice() {
            db::set_preference(&conn, "voice", v)?;
        }
        if let Some(s) = self.selected_style() {
            db::set_preference(&conn, "style", s)?;
        }
        db::set_preference(
            &conn,
            "stt_threshold",
            tui_stt::THRESHOLD_PRESETS[self.stt_threshold_idx],
        )?;
        db::set_preference(
            &conn,
            "stt_silence",
            tui_stt::SILENCE_PRESETS[self.stt_silence_idx],
        )?;
        db::set_preference(
            &conn,
            "stt_trim_silence",
            if self.stt_trim_silence { "1" } else { "0" },
        )?;
        db::set_preference(
            &conn,
            "stt_auto_enter",
            if self.stt_auto_enter { "1" } else { "0" },
        )?;
        self.status = "Preferences saved.".into();
        Ok(())
    }
}

fn render_list<'a>(title: &'a str, items: &[&str], selected: usize, active: bool) -> List<'a> {
    let items: Vec<ListItem> = items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let marker = if i == selected { "> " } else { "  " };
            let style = if i == selected && active {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else if i == selected {
                Style::default().fg(Color::White)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            ListItem::new(format!("{marker}{item}")).style(style)
        })
        .collect();

    let border_style = if active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    List::new(items).block(
        Block::bordered()
            .title(format!(" {title} "))
            .border_style(border_style),
    )
}

fn draw(frame: &mut Frame, app: &App) {
    let outer = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(3),
    ])
    .split(frame.area());

    frame.render_widget(
        Paragraph::new(" vox setup — interactive voice configuration").style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        outer[0],
    );

    let rows =
        Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]).split(outer[1]);

    let top_cols = Layout::horizontal([
        Constraint::Percentage(18),
        Constraint::Percentage(22),
        Constraint::Percentage(12),
        Constraint::Percentage(16),
        Constraint::Percentage(12),
        Constraint::Percentage(20),
    ])
    .split(rows[0]);

    let bottom_cols = Layout::horizontal([
        Constraint::Percentage(18),
        Constraint::Percentage(15),
        Constraint::Percentage(15),
        Constraint::Percentage(20),
        Constraint::Percentage(32),
    ])
    .split(rows[1]);

    let backend_items: Vec<&str> = app.backends.to_vec();
    frame.render_widget(
        render_list(
            "Backend",
            &backend_items,
            app.backend_idx,
            app.screen == Screen::Backend,
        ),
        top_cols[0],
    );

    let voice_items: Vec<&str> = app.voices.iter().map(|s| s.as_str()).collect();
    frame.render_widget(
        render_list(
            "Voice",
            &voice_items,
            app.voice_idx,
            app.screen == Screen::Voice,
        ),
        top_cols[1],
    );

    frame.render_widget(
        render_list(
            "Language",
            &app.languages,
            app.lang_idx,
            app.screen == Screen::Language,
        ),
        top_cols[2],
    );

    frame.render_widget(
        render_list(
            "Style",
            &app.styles,
            app.style_idx,
            app.screen == Screen::Style,
        ),
        top_cols[3],
    );

    let volume_items: Vec<&str> = VOLUME_PRESETS.to_vec();
    frame.render_widget(
        render_list(
            "Volume",
            &volume_items,
            app.volume_idx,
            app.screen == Screen::Volume,
        ),
        top_cols[4],
    );

    let mut summary = vec![
        Line::from(vec![
            Span::styled("Backend: ", Style::default().fg(Color::DarkGray)),
            Span::styled(app.selected_backend(), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Voice:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                app.selected_voice().unwrap_or("(default)"),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("Lang:    ", Style::default().fg(Color::DarkGray)),
            Span::styled(app.selected_lang(), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Style:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                app.selected_style().unwrap_or("(default)"),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("Volume:  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{}x", VOLUME_PRESETS[app.volume_idx]),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "── STT ──",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    summary.extend(tui_stt::summary_lines(
        app.stt_threshold_idx,
        app.stt_silence_idx,
        app.stt_trim_silence,
        app.stt_auto_enter,
    ));
    summary.extend([
        Line::from(""),
        Line::from(Span::styled(
            "[T] Test  [S] Save  [Q] Quit",
            Style::default().fg(Color::Green),
        )),
    ]);

    let active_test = app.screen == Screen::Test;
    let border_style = if active_test {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    frame.render_widget(
        Paragraph::new(summary).block(
            Block::bordered()
                .title(" Config ")
                .border_style(border_style),
        ),
        top_cols[5],
    );

    frame.render_widget(
        Block::bordered()
            .title(" STT Settings ")
            .border_style(Style::default().fg(Color::DarkGray)),
        bottom_cols[0],
    );

    frame.render_widget(
        tui_stt::render_threshold_list(app.stt_threshold_idx, app.screen == Screen::SttThreshold),
        bottom_cols[1],
    );

    frame.render_widget(
        tui_stt::render_silence_list(app.stt_silence_idx, app.screen == Screen::SttSilence),
        bottom_cols[2],
    );

    frame.render_widget(
        tui_stt::render_toggle_list(
            app.stt_toggle_idx,
            app.screen == Screen::SttToggles,
            app.stt_trim_silence,
            app.stt_auto_enter,
        ),
        bottom_cols[3],
    );

    frame.render_widget(
        Paragraph::new(tui_stt::summary_lines(
            app.stt_threshold_idx,
            app.stt_silence_idx,
            app.stt_trim_silence,
            app.stt_auto_enter,
        ))
        .block(
            Block::bordered()
                .title(" STT Summary ")
                .border_style(Style::default().fg(Color::DarkGray)),
        ),
        bottom_cols[4],
    );

    frame.render_widget(
        Paragraph::new(app.status.as_str()).block(
            Block::bordered()
                .title(" Status ")
                .border_style(Style::default().fg(Color::DarkGray)),
        ),
        outer[2],
    );
}

pub fn run() -> Result<()> {
    let mut app = App::new()?;

    terminal::enable_raw_mode().context("failed to enable raw mode")?;
    io::stdout()
        .execute(EnterAlternateScreen)
        .context("failed to enter alternate screen")?;

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend).context("failed to create terminal")?;

    let result = run_loop(&mut terminal, &mut app);

    terminal::disable_raw_mode().ok();
    io::stdout().execute(LeaveAlternateScreen).ok();

    result
}

fn run_loop(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|f| draw(f, app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => {
                    app.should_quit = true;
                }
                KeyCode::Up | KeyCode::Char('k') => app.move_up(),
                KeyCode::Down | KeyCode::Char('j') => app.move_down(),
                KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => app.next_screen(),
                KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => app.prev_screen(),
                KeyCode::Char('t') | KeyCode::Enter if app.screen == Screen::Test => {
                    app.test_speak();
                }
                code if app.screen == Screen::SttToggles
                    && tui_stt::handle_toggle_key(
                        code,
                        app.stt_toggle_idx,
                        &mut app.stt_trim_silence,
                        &mut app.stt_auto_enter,
                    ) => {}
                KeyCode::Char('t') => app.test_speak(),
                KeyCode::Char('s') => {
                    if let Err(e) = app.save() {
                        app.status = format!("Save error: {e}");
                    }
                }
                KeyCode::Enter => app.next_screen(),
                _ => {}
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}
