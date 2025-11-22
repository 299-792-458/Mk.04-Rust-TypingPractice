use std::io;
use std::time::Duration;

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Gauge, Paragraph, Wrap},
};

const LYRICS: [&str; 4] = [
    "동해 물과 백두산이 마르고 닳도록",
    "하느님이 보우하사 우리나라 만세",
    "무궁화 삼천리 화려 강산",
    "대한 사람 대한으로 길이 보전하세",
];

#[derive(Debug)]
enum StepResult {
    Correct,
    Wrong(char),
    Ignored,
    Victory,
    Restarted,
}

struct Game {
    text_chars: Vec<char>,
    char_meta: Vec<(usize, usize)>,
    total_chars: usize,
    boss_hp: f32,
    boss_damage: f32,
    current_index: usize,
    awaiting_restart: bool,
    message: String,
}

impl Game {
    fn new() -> Self {
        let text_chars: Vec<char> = LYRICS.iter().flat_map(|line| line.chars()).collect();
        let mut char_meta = Vec::with_capacity(text_chars.len());
        for (line_idx, line) in LYRICS.iter().enumerate() {
            for (pos, _) in line.chars().enumerate() {
                char_meta.push((line_idx, pos));
            }
        }

        let total_chars = text_chars.len();
        let boss_damage = 100.0 / total_chars as f32;

        Self {
            text_chars,
            char_meta,
            total_chars,
            boss_hp: 100.0,
            boss_damage,
            current_index: 0,
            awaiting_restart: false,
            message: String::from("가사를 모두 입력해 보스를 처치하세요."),
        }
    }

    fn reset(&mut self) {
        self.boss_hp = 100.0;
        self.current_index = 0;
        self.awaiting_restart = false;
        self.message = String::from("다시 시작했습니다. 계속 입력하세요.");
    }

    fn expected_char(&self) -> Option<char> {
        self.text_chars.get(self.current_index).copied()
    }

    fn process_char(&mut self, ch: char) -> StepResult {
        if self.awaiting_restart {
            if ch == ' ' {
                self.reset();
                return StepResult::Restarted;
            }
            return StepResult::Ignored;
        }

        if ch == '\n' || ch == '\r' {
            return StepResult::Ignored;
        }

        let Some(expected) = self.expected_char() else {
            self.awaiting_restart = true;
            self.message = String::from("승리! 스페이스로 다시 시작합니다.");
            return StepResult::Victory;
        };

        if Self::is_composing_char(ch) {
            return StepResult::Ignored;
        }

        if ch.is_whitespace() && ch != ' ' && expected != ' ' {
            return StepResult::Ignored;
        }

        if ch == expected {
            self.current_index += 1;
            self.boss_hp = (self.boss_hp - self.boss_damage).max(0.0);

            if self.current_index >= self.total_chars {
                self.awaiting_restart = true;
                self.message = String::from("승리! 스페이스로 다시 시작합니다.");
                StepResult::Victory
            } else {
                self.message = String::from("정확!");
                StepResult::Correct
            }
        } else {
            self.message = String::from("틀렸습니다.");
            StepResult::Wrong(ch)
        }
    }

    fn line_state(&self) -> (usize, usize) {
        if self.current_index >= self.total_chars {
            let idx = LYRICS.len() - 1;
            (idx, LYRICS[idx].chars().count())
        } else {
            let (idx, typed_len) = self.char_meta[self.current_index];
            (idx, typed_len)
        }
    }

    fn is_composing_char(ch: char) -> bool {
        let code = ch as u32;
        (0x1100..=0x11FF).contains(&code)
            || (0x3130..=0x318F).contains(&code)
            || (0xA960..=0xA97F).contains(&code)
            || (0xD7B0..=0xD7FF).contains(&code)
    }
}

fn main() -> io::Result<()> {
    let mut terminal = setup_terminal()?;

    let app_result = run_app(&mut terminal);

    restore_terminal()?;

    app_result?;
    println!("게임을 종료합니다.");
    Ok(())
}

fn setup_terminal() -> io::Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, cursor::Hide)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;
    Ok(terminal)
}

fn restore_terminal() -> io::Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, cursor::Show)?;
    Ok(())
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    let mut game = Game::new();
    game.message = String::from("실시간 입력 활성화. 가사를 이어서 입력하세요.");

    let mut wrong_char: Option<char> = None;

    loop {
        terminal.draw(|f| draw_ui(f, &game, wrong_char))?;

        if !event::poll(Duration::from_millis(250))? {
            continue;
        }

        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL)
                    && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('C'))
                {
                    break;
                }

                if key.code == KeyCode::Esc {
                    break;
                }

                if let Some(ch) = key_to_char(&key.code) {
                    match game.process_char(ch) {
                        StepResult::Wrong(c) => {
                            wrong_char = Some(c);
                        }
                        StepResult::Correct | StepResult::Victory | StepResult::Restarted => {
                            wrong_char = None;
                        }
                        StepResult::Ignored => {}
                    }
                }
            }
            Event::Resize(_, _) => {}
            _ => {}
        }
    }

    Ok(())
}

fn key_to_char(code: &KeyCode) -> Option<char> {
    match code {
        KeyCode::Char(c) => Some(*c),
        KeyCode::Enter => Some('\n'),
        KeyCode::Tab => Some('\t'),
        _ => None,
    }
}

fn draw_ui(f: &mut Frame, game: &Game, wrong_char: Option<char>) {
    let area = centered_rect(70, 85, f.size());

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(area);

    draw_header(f, layout[0]);
    draw_stats(f, layout[1], game);
    draw_lyrics(f, layout[2], game, wrong_char);
    draw_messages(f, layout[3], game);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ]
            .as_ref(),
        )
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ]
            .as_ref(),
        )
        .split(popup_layout[1])[1]
}

fn draw_header(f: &mut Frame, area: Rect) {
    let text = vec![Line::from(Span::styled(
        "Mk.04 Rust Typing Practice",
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    ))];

    let para = Paragraph::new(text)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).title("타이틀"))
        .wrap(Wrap { trim: true });
    f.render_widget(para, area);
}

fn draw_stats(f: &mut Frame, area: Rect, game: &Game) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
        .split(area);

    let hp_ratio = (clamp_percent(game.boss_hp) / 100.0).clamp(0.0, 1.0);
    let hp_gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("보스 체력"))
        .gauge_style(Style::default().fg(Color::Green))
        .ratio(hp_ratio as f64)
        .label(format!("{:>5.1}%", clamp_percent(game.boss_hp)));
    f.render_widget(hp_gauge, chunks[0]);

    let progress_percent = if game.total_chars == 0 {
        0.0
    } else {
        (game.current_index as f32 / game.total_chars as f32) * 100.0
    };
    let progress_ratio = (clamp_percent(progress_percent) / 100.0).clamp(0.0, 1.0);
    let progress = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("진행도"))
        .gauge_style(Style::default().fg(Color::Cyan))
        .ratio(progress_ratio as f64)
        .label(format!(
            "{:>5.1}% ({}/{})",
            clamp_percent(progress_percent),
            game.current_index,
            game.total_chars
        ));
    f.render_widget(progress, chunks[1]);
}

fn draw_lyrics(f: &mut Frame, area: Rect, game: &Game, wrong_char: Option<char>) {
    let (line_idx, typed_len) = game.line_state();
    let current_line = LYRICS[line_idx];

    let header_line = Line::from(vec![
        Span::styled(
            format!("현재 줄 {}/{}", line_idx + 1, LYRICS.len()),
            Style::default().fg(Color::Yellow),
        ),
        Span::raw("   "),
        Span::raw(format!(
            "위치 {}/{}",
            typed_len,
            current_line.chars().count()
        )),
    ]);

    let lines = vec![
        header_line,
        styled_line(current_line, typed_len, wrong_char),
    ];

    let para = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("가사 진행"))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: false });
    f.render_widget(para, area);
}

fn draw_messages(f: &mut Frame, area: Rect, game: &Game) {
    let text = vec![Line::from(game.message.as_str())];

    let para = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title("메시지"))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    f.render_widget(para, area);
}

fn clamp_percent(value: f32) -> f32 {
    if value < 0.0 {
        0.0
    } else if value > 100.0 {
        100.0
    } else {
        value
    }
}

fn styled_line(line: &str, typed_len: usize, wrong_char: Option<char>) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();

    for (idx, ch) in line.chars().enumerate() {
        let style = if idx < typed_len {
            Style::default().fg(Color::Green)
        } else if idx == typed_len {
            if wrong_char.is_some() {
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            }
        } else {
            Style::default().fg(Color::DarkGray)
        };

        spans.push(Span::styled(ch.to_string(), style));
    }

    Line::from(spans)
}
