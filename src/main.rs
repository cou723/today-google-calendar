mod calendar;
mod event;
mod token;
use std::{env, io, time::Duration};

use anyhow::Result;
use calendar::Calendar;
use chrono::{DateTime, Datelike, Timelike, Utc};
use chrono_tz::Asia::Tokyo;
use chrono_tz::Tz;
use event::{EventModel, EventView};
use oauth2::basic::{BasicErrorResponseType, BasicTokenType};
use oauth2::url::Url;
use oauth2::{
    EmptyExtraTokenFields, EndpointNotSet, EndpointSet, RevocationErrorResponseType,
    StandardErrorResponse, StandardRevocableToken, StandardTokenIntrospectionResponse,
    StandardTokenResponse,
};
use ratatui::crossterm;
use ratatui::layout::Rect;
use ratatui::prelude::CrosstermBackend;
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Terminal;
use reqwest::blocking::Client;
use token::Token;

struct App {
    events: Option<Vec<EventModel>>,
    token: Token,
    fetched_time: DateTime<Tz>,
    calendar_list: Vec<Calendar>,
}

type OAuthClient = oauth2::Client<
    StandardErrorResponse<BasicErrorResponseType>,
    StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>,
    StandardTokenIntrospectionResponse<EmptyExtraTokenFields, BasicTokenType>,
    StandardRevocableToken,
    StandardErrorResponse<RevocationErrorResponseType>,
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointSet,
    EndpointSet,
>;

impl App {
    fn new(
        client_id: String,
        client_secret: String,
        now: DateTime<Tz>,
        calendar_list: Vec<Calendar>,
    ) -> Result<Self> {
        Ok(App {
            events: None,
            token: Token::new(client_id, client_secret)?,
            fetched_time: now,
            calendar_list,
        })
    }

    fn get_utc_date_range_string(date: DateTime<Tz>) -> Result<(String, String)> {
        return Ok((
            date.with_time(chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                .unwrap()
                .to_rfc3339(),
            date.with_time(chrono::NaiveTime::from_hms_opt(23, 59, 59).unwrap())
                .unwrap()
                .to_rfc3339(),
        ));
    }

    fn fetch_date_events(self: &mut Self, date: DateTime<Tz>) -> Result<()> {
        self.fetched_time = date;
        let client = Client::new();

        let (time_min, time_max) = App::get_utc_date_range_string(date)?;

        println!("Fetching events for date: {}", date);
        println!("Time Min (UTC): {}", time_min);
        println!("Time Max (UTC): {}", time_max);

        self.events = Some(
            self.calendar_list
                .iter()
                .flat_map(|calendar| -> Vec<EventModel> {
                    let url = Url::parse(
                        format!(
                            "https://www.googleapis.com/calendar/v3/calendars/{}/events",
                            calendar.id()
                        )
                        .as_str(),
                    )
                    .expect("URL should be valid");

                    use reqwest::StatusCode;

                    let mut response = client
                        .get(url.clone())
                        .query(&[("timeMin", time_min.as_str())])
                        .query(&[("timeMax", time_max.as_str())])
                        .query(&[("orderBy", "startTime")])
                        .query(&[("singleEvents", "true")])
                        .query(&[("access_type", "offline")])
                        .query(&[("prompt", "consent")])
                        .bearer_auth(self.token.access_token.clone())
                        .send()
                        .expect("Request should be sent");

                    if response.status() == StatusCode::UNAUTHORIZED {
                        eprintln!("Access token expired or invalid. Attempting to refresh token...");
                        match self.token.refresh() {
                            Ok(_) => {
                                eprintln!("Token refresh succeeded. Retrying request...");
                                response = client
                                    .get(url)
                                    .query(&[("timeMin", time_min.as_str())])
                                    .query(&[("timeMax", time_max.as_str())])
                                    .query(&[("orderBy", "startTime")])
                                    .query(&[("singleEvents", "true")])
                                    .query(&[("access_type", "offline")])
                                    .query(&[("prompt", "consent")])
                                    .bearer_auth(self.token.access_token.clone())
                                    .send()
                                    .expect("Request should be sent (after refresh)");
                                if !response.status().is_success() {
                                    eprintln!("Request failed after token refresh: {:?}", response.status());
                                    panic!(
                                        "Request failed with text after refresh: {}",
                                        response.text().unwrap_or_default()
                                    );
                                }
                            }
                            Err(e) => {
                                eprintln!("Token refresh failed: {:?}", e);
                                panic!("Token refresh failed: {:?}", e);
                            }
                        }
                    } else if !response.status().is_success() {
                        eprintln!("Error: {:?}", response.status());
                        panic!(
                            "Request failed with text: {}",
                            response.text().unwrap_or_default()
                        );
                    }

                    let response_text = response.text().expect("Response should be text");

                    let response_data = serde_json::from_str::<google_calendar3::api::Events>(
                        response_text.as_str(),
                    )
                    .expect("Response should be deserialized");
                    response_data
                        .items
                        .unwrap_or_default()
                        .iter()
                        .map(|event| EventModel::new(event.clone(), calendar.clone()))
                        .collect()
                })
                .collect::<Vec<EventModel>>(),
        );
        Ok(())
    }

    fn render_ui(
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        events: Vec<EventView>,
        now: DateTime<Tz>,
    ) -> Result<()> {
        terminal.clear()?;
        terminal.draw(|terminal_window| {
            // render events
            let height_unit: u16 = terminal_window.area().height / 48;
            for event in events {
                let size = Rect {
                    x: 1 + terminal_window.area().x,
                    y: event.start * height_unit,
                    width: terminal_window.area().width - 1,
                    height: event.height * height_unit,
                };

                terminal_window.render_widget(
                    Paragraph::new(event.title).block(
                        Block::default()
                            .borders(Borders::NONE)
                            .style(Style::default().bg(event.color)),
                    ),
                    size,
                );
            }
            // render now line
            let now_height = EventView::date_time_to_height(now, &Tokyo);
            let size = Rect {
                x: 0,
                y: now_height,
                width: 1,
                height: 1,
            };
            terminal_window.render_widget(
                Paragraph::new(">").block(
                    Block::default()
                        .borders(Borders::NONE)
                        .style(Style::default().bg(ratatui::style::Color::Yellow)),
                ),
                size,
            );
        })?;
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 環境変数の読み込み
    dotenv::dotenv().ok();

    // ターミナルの初期化
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(
        stdout,
        crossterm::terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture
    )?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // ctrlc ハンドラーの設定
    ctrlc::set_handler(move || {
        println!("終了処理中...");
        if let Err(e) = crossterm::terminal::disable_raw_mode() {
            eprintln!("終了処理中にエラーが発生しました: {:?}", e);
        }
        std::process::exit(0);
    })?;

    // アプリケーションの初期化
    let mut app = App::new(
        env::var("GOOGLE_CLIENT_ID").expect("GOOGLE_CLIENT_ID is not defined in env"),
        env::var("GOOGLE_CLIENT_SECRET").expect("GOOGLE_CLIENT_SECRET is not defined in env"),
        Utc::now().with_timezone(&Tokyo),
        vec![Calendar::Primary, Calendar::Private, Calendar::University],
    )?;

    // 初回の予定取得と表示
    (app.fetch_date_events(Utc::now().with_timezone(&Tokyo))?);

    {
        App::render_ui(
            &mut terminal,
            app.events
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Events are not fetched"))?
                .iter()
                .map(|event| EventView::from_event(event.clone()))
                .filter_map(|event| event.ok())
                .collect(),
            Utc::now().with_timezone(&Tokyo),
        )?;
    }

    // エラーハンドリング付きのメインループ
    run_app(&mut terminal, &mut app)?;

    // エラーを返す
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut last_check = std::time::Instant::now();
    let check_interval = Duration::from_secs(60); // 1分ごとにチェック

    loop {
        // キー入力をポーリング（タイムアウト付き）
        if crossterm::event::poll(Duration::from_millis(100))? {
            if let crossterm::event::Event::Key(key) = crossterm::event::read()? {
                if key.kind == crossterm::event::KeyEventKind::Press
                    && key.code == crossterm::event::KeyCode::Char('q')
                {
                    return Ok(());
                }
            }
        }

        // 現在時刻を確認
        let now = std::time::Instant::now();
        if now.duration_since(last_check) >= check_interval {
            let now_date = Utc::now().with_timezone(&Tokyo);

            //30分ごとにUIを更新
            if now_date.minute() == 0 || now_date.minute() == 30 {
                App::render_ui(
                    terminal,
                    app.events
                        .as_ref()
                        .ok_or_else(|| anyhow::anyhow!("Events are not fetched"))?
                        .iter()
                        .map(|event| EventView::from_event(event.clone()))
                        .filter_map(|event| event.ok())
                        .collect(),
                    Utc::now().with_timezone(&Tokyo),
                )?;
            }

            // 日付が変わった場合はeventを再取得
            if now_date.day() != app.fetched_time.day() {
                app.fetch_date_events(now_date)?;
            }
            last_check = now;
        }
    }
}
