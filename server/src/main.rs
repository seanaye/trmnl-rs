use crate::{
    display::{app, display},
    domain::models::render::BmpWrapper,
    image::image_handler,
    log::log_handler,
    setup::setup_handler,
};
use axum::{
    Router,
    extract::FromRef,
    routing::{get, post},
};
use bytes::Bytes;
use mousefood::{EmbeddedBackend, EmbeddedBackendConfig};
use ratatui::{
    Terminal,
    layout::{Constraint, Layout},
    widgets::{Block, Padding, Paragraph, Wrap},
};

use redb::Database;
use std::{
    net::{Ipv4Addr, SocketAddrV4},
    ops::Deref,
    sync::Arc,
    time::Duration,
};

use axum::http::header::{CONNECTION, HeaderValue};
use todo_parser_rs::TaskBuf;
use tokio::{sync::oneshot, task::JoinHandle};
use tower_http::{services::ServeDir, set_header::SetResponseHeaderLayer, trace::TraceLayer};
use tracing_subscriber::EnvFilter;

mod display;
mod domain;
mod extractor;
mod image;
mod log;
mod setup;
mod tables;
mod trace_err;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_ansi(true)
        .with_env_filter(EnvFilter::from_default_env())
        .with_line_number(true)
        .pretty()
        .init();

    let ip = SocketAddrV4::new(
        Ipv4Addr::new(0, 0, 0, 0),
        std::env::var("PORT")
            .map(|x| {
                x.as_str()
                    .parse::<u16>()
                    .expect("Received a non integer port number via env var")
            })
            .unwrap_or(2300),
    );
    let listener = tokio::net::TcpListener::bind(ip)
        .await
        .expect("failed to bind listener");
    tracing::info!("listening on {}", listener.local_addr().unwrap());

    let db = Database::create("data.redb").unwrap();

    axum::serve(
        listener,
        router(State {
            db: Arc::new(db),
            ratatui: Arc::new(RatatuiHandle::new().await),
        }),
    )
    .await
    .unwrap()
}

enum Msg {
    PrintOut(tokio::sync::oneshot::Sender<Bytes>),
}

struct RatatuiHandle {
    tx: tokio::sync::mpsc::Sender<Msg>,
    handle: JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>>,
    wttr_poller: WttrPoller,
}

impl RatatuiHandle {
    async fn get_bytes(&self) -> Result<Bytes, tokio::sync::oneshot::error::RecvError> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(Msg::PrintOut(tx)).await.unwrap();
        rx.await
    }
    async fn new() -> Self {
        let (tx, rx) = tokio::sync::mpsc::channel(10);
        let wttr_poller = WttrPoller::new(Duration::from_secs(3600)).await;
        let wttr_rx = wttr_poller.subscribe();
        let handle = tokio::task::spawn_blocking(move || BackgroundThread { rx, wttr_rx }.run());
        Self {
            handle,
            tx,
            wttr_poller,
        }
    }
}

struct BackgroundThread {
    rx: tokio::sync::mpsc::Receiver<Msg>,
    wttr_rx: tokio::sync::watch::Receiver<WttrResponse>,
}

impl BackgroundThread {
    #[tracing::instrument(err, skip(self))]
    fn run(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let BackgroundThread { mut rx, wttr_rx } = self;

        let mut display = BmpWrapper::new_with_scale(800, 480, 3);
        let display_clone = display.clone();
        let backend = EmbeddedBackend::new(&mut display, EmbeddedBackendConfig::default());
        let mut terminal = Terminal::new(backend).unwrap();

        while let Some(msg) = rx.blocking_recv() {
            match msg {
                Msg::PrintOut(sender) => {
                    terminal.draw(|f| {
                        // Main vertical layout: top bar, middle content, bottom bar
                        let [top, middle, bottom] = Layout::vertical([
                            Constraint::Length(2),
                            Constraint::Fill(1),
                            Constraint::Length(7),
                        ])
                        .areas(f.area());

                        // Date/time at top center
                        let now = chrono::Local::now();
                        let s = now.format("%A %B %e %H:%M").to_string();
                        f.render_widget(Paragraph::new(s).centered(), top);

                        // Top 5 todos in center
                        let todos = top_todos(now.date_naive(), 5);
                        let todo_text = todos
                            .iter()
                            .map(|t| {
                                let boxed = if t.completed { "[x]" } else { "[ ]" };
                                // Hide the @home context tag from the rendered text
                                let description = t
                                    .description
                                    .split_whitespace()
                                    .filter(|word| *word != "@home")
                                    .collect::<Vec<_>>()
                                    .join(" ");
                                match t.priority {
                                    Some(p) => format!("{boxed} ({p}) {description}"),
                                    None => format!("{boxed} {description}"),
                                }
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                        f.render_widget(
                            Paragraph::new(todo_text)
                                .wrap(Wrap { trim: false })
                                .block(Block::new().padding(Padding::horizontal(1))),
                            middle,
                        );

                        // Weather anchored to the bottom-right corner:
                        // dedent/trim the wttr art and size the area to fit it exactly.
                        let wttr_text = wttr_rx.borrow();
                        let raw = wttr_text.deref().inner.trim_end_matches(['\n', ' ']);
                        let indent = raw
                            .lines()
                            .filter(|l| !l.trim().is_empty())
                            .map(|l| l.len() - l.trim_start().len())
                            .min()
                            .unwrap_or(0);
                        let lines: Vec<&str> = raw
                            .lines()
                            .map(|l| l.get(indent..).unwrap_or("").trim_end())
                            .collect();
                        let width = lines
                            .iter()
                            .map(|l| l.chars().count())
                            .max()
                            .unwrap_or(0) as u16;
                        let height = lines.len() as u16;
                        let [_, wttr_col] =
                            Layout::horizontal([Constraint::Fill(1), Constraint::Length(width)])
                                .areas(bottom);
                        let [_, wttr_area] =
                            Layout::vertical([Constraint::Fill(1), Constraint::Length(height)])
                                .areas(wttr_col);
                        f.render_widget(Paragraph::new(lines.join("\n")), wttr_area);
                    })?;

                    let d = display_clone.data()?;
                    sender.send(Bytes::from(d)).unwrap();
                }
            }
        }
        Ok(())
    }
}

/// Reads `~/todo.txt` and returns the top `limit` tasks tagged `@home`.
///
/// Incomplete tasks come first: those due within 3 days of `today` lead
/// (soonest due date first), then the rest ordered by priority (A highest,
/// no priority last). Completed tasks sort last, so they only appear when
/// there are fewer than `limit` open tasks.
fn top_todos(today: chrono::NaiveDate, limit: usize) -> Vec<TaskBuf> {
    let path = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default()).join("todo.txt");
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("failed to read {}: {e}", path.display());
            return Vec::new();
        }
    };
    let cutoff = today + chrono::Days::new(3);
    let mut tasks: Vec<TaskBuf> = content
        .lines()
        .filter_map(|line| line.parse::<TaskBuf>().ok())
        .filter(|t| t.contexts.iter().any(|c| c == "home"))
        .collect();
    tasks.sort_by_key(|t| {
        let due_soon = t
            .due_date()
            .and_then(|d| {
                chrono::NaiveDate::from_ymd_opt(d.year() as i32, d.month() as u32, d.day() as u32)
            })
            .filter(|d| *d < cutoff);
        (
            t.completed,
            due_soon.is_none(),
            due_soon,
            t.priority.is_none(),
            t.priority,
        )
    });
    tasks.truncate(limit);
    tasks
}

#[derive(Clone, FromRef)]
struct State {
    db: Arc<Database>,
    ratatui: Arc<RatatuiHandle>,
}

fn router(state: State) -> Router {
    Router::new()
        .nest_service("/assets", ServeDir::new("assets"))
        .nest(
            "/api",
            Router::new()
                .route("/setup", get(setup_handler))
                .route("/log", post(log_handler))
                .route("/display", get(display)),
        )
        .route("/app/{filename}", get(app))
        .with_state(state)
        .layer(TraceLayer::new_for_http())
        .layer(SetResponseHeaderLayer::overriding(
            CONNECTION,
            HeaderValue::from_static("close"),
        ))
}

#[derive(Clone)]
struct WttrPoller {
    rx: tokio::sync::watch::Receiver<WttrResponse>,
    bg_handle: Arc<JoinHandle<()>>,
}

struct WttrClient {
    client: reqwest::Client,
}

impl WttrClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    #[tracing::instrument(ret, err, skip(self))]
    async fn get_weather(&self) -> Result<WttrResponse, reqwest::Error> {
        let res = self
            .client
            .get("https://wttr.in/?0TQ")
            .header("User-Agent", "curl/7.64.1")
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        Ok(WttrResponse { inner: res })
    }
}

impl WttrPoller {
    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<WttrResponse> {
        self.rx.clone()
    }

    pub async fn new(poll_duration: Duration) -> Self {
        let client = WttrClient::new();
        let res = match client.get_weather().await {
            Ok(res) => res,
            Err(e) => {
                tracing::warn!("initial weather fetch failed, will retry: {e}");
                WttrResponse {
                    inner: String::from("Weather unavailable"),
                }
            }
        };

        let (tx, rx) = tokio::sync::watch::channel(res);
        let bg_handle = tokio::task::spawn(async move {
            loop {
                tokio::time::sleep(poll_duration).await;
                let Ok(res) = client.get_weather().await else {
                    continue;
                };
                let _ = tx.send(res);
            }
        });

        WttrPoller {
            rx,
            bg_handle: Arc::new(bg_handle),
        }
    }
}

#[derive(Debug)]
struct WttrResponse {
    inner: String,
}
