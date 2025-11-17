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
    layout::{Constraint, Flex, Layout, Rect},
    style::Color,
    text::Text,
    widgets::{Block, Paragraph},
};
use redb::Database;
use std::sync::Arc;
use tokio::{sync::oneshot, task::JoinHandle};
use tower_http::{services::ServeDir, trace::TraceLayer};
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
            ratatui: Arc::new(RatatuiHandle::new()),
        }),
    )
    .await
    .unwrap()
}

enum Msg {
    PrintOut(tokio::sync::oneshot::Sender<Bytes>),
}

#[derive(Debug)]
struct RatatuiHandle {
    tx: tokio::sync::mpsc::Sender<Msg>,
    handle: JoinHandle<Result<(), std::io::Error>>,
}

impl RatatuiHandle {
    async fn get_bytes(&self) -> Result<Bytes, tokio::sync::oneshot::error::RecvError> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(Msg::PrintOut(tx)).await.unwrap();
        rx.await
    }
    fn new() -> Self {
        let (tx, rx) = tokio::sync::mpsc::channel(10);
        let handle = tokio::task::spawn_blocking(move || BackgroundThread { rx }.run());
        Self { handle, tx }
    }
}

struct BackgroundThread {
    rx: tokio::sync::mpsc::Receiver<Msg>,
}

impl BackgroundThread {
    #[tracing::instrument(err, skip(self))]
    fn run(self) -> Result<(), std::io::Error> {
        let BackgroundThread { mut rx } = self;

        let mut display = BmpWrapper::new_with_scale(800, 480, 2);
        let display_clone = display.clone();
        let backend = EmbeddedBackend::new(&mut display, EmbeddedBackendConfig::default());
        let mut terminal = Terminal::new(backend).unwrap();
        while let Some(msg) = rx.blocking_recv() {
            match msg {
                Msg::PrintOut(sender) => {
                    terminal.draw(|f| {
                        fn center(area: Rect, vertical: Constraint) -> Rect {
                            let [area] =
                                Layout::vertical([vertical]).flex(Flex::Center).areas(area);
                            area
                        }
                        let text = Paragraph::new("Hello world!").centered();
                        let area = center(f.area(), Constraint::Length(1));
                        f.render_widget(text, area);
                    })?;

                    let d = display_clone.data()?;
                    sender.send(Bytes::from(d)).unwrap();
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, FromRef)]
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
        .route("/app.bmp", get(app))
        .with_state(state)
        .layer(TraceLayer::new_for_http())
}
