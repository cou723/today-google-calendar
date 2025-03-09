mod calendar;
mod event;
mod token;
use std::{env, io, time::Duration};

use anyhow::Result;
use calendar::Calendar;
use chrono::{DateTime, FixedOffset, NaiveDate, Utc};
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
    current_date: NaiveDate,
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
    fn new(client_id: String, client_secret: String) -> Result<Self> {
        let jst = FixedOffset::east_opt(9 * 3600).expect("Failed to create JST offset");
        let now_jst: DateTime<FixedOffset> = Utc::now().with_timezone(&jst);
        let today_jst = now_jst.date_naive();

        Ok(App {
            events: None,
            token: Token::new(client_id, client_secret)?,
            current_date: today_jst,
        })
    }

    fn get_date_range(date: NaiveDate) -> Result<(String, String)> {
        let jst = FixedOffset::east_opt(9 * 3600).expect("Failed to create JST offset");

        let time_min_jst = match date
            .and_hms_opt(0, 0, 0)
            .expect("Failed to create time_min_jst")
            .and_local_timezone(jst)
        {
            chrono::offset::LocalResult::Ambiguous(x, _) => x,
            chrono::offset::LocalResult::Single(x) => x,
            chrono::offset::LocalResult::None => panic!("Failed to create time_min_jst"),
        };

        let time_min_utc = time_min_jst.with_timezone(&Utc);
        let time_min = time_min_utc.to_rfc3339();

        let time_max_jst = match date
            .and_hms_opt(23, 59, 59)
            .expect("Failed to create time_max_jst")
            .and_local_timezone(jst)
        {
            chrono::offset::LocalResult::Ambiguous(x, _) => x,
            chrono::offset::LocalResult::Single(x) => x,
            chrono::offset::LocalResult::None => panic!("Failed to create time_max_jst"),
        };

        let time_max_utc = time_max_jst.with_timezone(&Utc);
        let time_max = time_max_utc.to_rfc3339();
        return Ok((time_min, time_max));
    }

    fn fetch_date_events(
        self: &Self,
        date: NaiveDate,
        display_calendar_list: Vec<Calendar>,
    ) -> Result<Vec<EventModel>> {
        let client = Client::new();

        let (time_min, time_max) = App::get_date_range(date)?;

        println!("Fetching events for date: {}", date);
        println!("Time Min (UTC): {}", time_min);
        println!("Time Max (UTC): {}", time_max);

        Ok(display_calendar_list
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

                let response = client
                    .get(url)
                    .query(&[("timeMin", time_min.as_str())])
                    .query(&[("timeMax", time_max.as_str())])
                    .query(&[("orderBy", "startTime")])
                    .query(&[("singleEvents", "true")])
                    .query(&[("access_type", "offline")])
                    .query(&[("prompt", "consent")])
                    .bearer_auth(self.token.access_token.clone())
                    .send()
                    .expect("Request should be sent");

                if !response.status().is_success() {
                    eprintln!("Error: {:?}", response.status());
                    panic!(
                        "Request failed with text: {}",
                        response.text().unwrap_or_default()
                    );
                }

                let response_text = response.text().expect("Response should be text");

                let response_data =
                    serde_json::from_str::<google_calendar3::api::Events>(response_text.as_str())
                        .expect("Response should be deserialized");
                response_data
                    .items
                    .unwrap_or_default()
                    .iter()
                    .map(|event| EventModel::new(event.clone(), calendar.clone()))
                    .collect()
            })
            .collect::<Vec<EventModel>>())
    }

    fn fetch_today_events(
        self: &Self,
        display_calendar_list: Vec<Calendar>,
    ) -> Result<Vec<EventModel>> {
        let jst = FixedOffset::east_opt(9 * 3600).expect("Failed to create JST offset");
        let now_jst: DateTime<FixedOffset> = Utc::now().with_timezone(&jst);
        let today_jst = now_jst.date_naive();

        self.fetch_date_events(today_jst, display_calendar_list)
    }

    fn render_ui(
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        events: Vec<EventView>,
    ) -> Result<()> {
        terminal.clear()?;
        terminal.draw(|terminal_window| {
            let height_unit = terminal_window.area().height as f64 / 24 as f64;
            for event in events {
                let size = Rect {
                    x: terminal_window.area().x,
                    y: (event.start as f64 * height_unit) as u16,
                    width: terminal_window.area().width,
                    height: (event.height * height_unit) as u16,
                };

                terminal_window.render_widget(
                    Paragraph::new(event.title).block(
                        Block::default()
                            .borders(Borders::ALL)
                            .style(Style::default().bg(event.color)),
                    ),
                    size,
                );
            }
        })?;
        Ok(())
    }

    // 日付が変わったかどうかをチェックし、変わっていたら予定を更新する
    fn check_and_update_date(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        calendars: &[Calendar],
    ) -> Result<bool> {
        let jst = FixedOffset::east_opt(9 * 3600).expect("Failed to create JST offset");
        let now_jst: DateTime<FixedOffset> = Utc::now().with_timezone(&jst);
        let today_jst = now_jst.date_naive();

        if today_jst != self.current_date {
            println!("日付が変わりました: {} -> {}", self.current_date, today_jst);
            self.current_date = today_jst;
            self.events = Some(self.fetch_date_events(today_jst, calendars.to_vec())?);

            App::render_ui(
                terminal,
                self.events
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Events are not fetched"))?
                    .iter()
                    .map(|event| EventView::from_event(event.clone()))
                    .filter_map(|event| event.ok())
                    .collect(),
            )?;

            return Ok(true);
        }

        Ok(false)
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
    )?;

    // 表示するカレンダーのリスト
    let calendars = vec![Calendar::Primary, Calendar::Private, Calendar::University];

    // 初回の予定取得と表示
    app.events = Some(app.fetch_today_events(calendars.clone())?);

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
        )?;
    }

    // エラーハンドリング付きのメインループ
    run_app(&mut terminal, &mut app, &calendars)?;

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
    calendars: &[Calendar],
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
            // 日付が変わったかチェック
            app.check_and_update_date(terminal, calendars)?;
            last_check = now;
        }
    }
}
