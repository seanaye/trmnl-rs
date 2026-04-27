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
    layout::{Constraint, Flex, Layout},
    widgets::Paragraph,
};
use std::path::PathBuf;
use redb::Database;
use std::{ops::Deref, sync::Arc, time::Duration};

use tokio::{sync::oneshot, task::JoinHandle};
use axum::http::header::{CONNECTION, HeaderValue};
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

    let listener = tokio::net::TcpListener::bind("0.0.0.0:2300")
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
    handle: JoinHandle<Result<(), std::io::Error>>,
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
        let markdown_path = PathBuf::from(
            std::env::var("TRMNL_MARKDOWN_PATH").unwrap_or_else(|_| "content.md".to_string()),
        );
        let handle = tokio::task::spawn_blocking(move || {
            BackgroundThread {
                rx,
                wttr_rx,
                markdown_path,
            }
            .run()
        });
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
    markdown_path: PathBuf,
}

impl BackgroundThread {
    #[tracing::instrument(err, skip(self))]
    fn run(self) -> Result<(), std::io::Error> {
        let BackgroundThread {
            mut rx,
            wttr_rx,
            markdown_path,
        } = self;

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

                        // Markdown content in center
                        let markdown_content = std::fs::read_to_string(&markdown_path)
                            .unwrap_or_else(|_| String::from("*No content available*"));
                        let [md_area] = Layout::horizontal([Constraint::Percentage(80)])
                            .flex(Flex::Center)
                            .areas(middle);
                        f.render_widget(Paragraph::new(markdown_content), md_area);

                        // Weather in bottom right (33 chars wide, 7 lines tall)
                        let wttr_text = wttr_rx.borrow();
                        let [_, wttr_area] = Layout::horizontal([
                            Constraint::Fill(1),
                            Constraint::Length(33),
                        ])
                        .areas(bottom);
                        f.render_widget(
                            Paragraph::new(wttr_text.deref().inner.as_str()),
                            wttr_area,
                        );
                    })?;

                    let d = display_clone.data()?;
                    sender.send(Bytes::from(d)).unwrap();
                }
            }
        }
        Ok(())
    }
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
                WttrResponse { inner: String::from("Weather unavailable") }
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
