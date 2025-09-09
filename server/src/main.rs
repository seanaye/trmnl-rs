use std::sync::Arc;

use axum::{
    Router,
    routing::{get, post},
};
use redb::Database;

use crate::{log::log_handler, setup::setup_handler};

mod log;
mod setup;
mod trace_err;

#[tokio::main]
async fn main() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:2443")
        .await
        .expect("failed to bind listener");
    println!("listening on {}", listener.local_addr().unwrap());

    let db = Database::create("data.redb").unwrap();

    axum::serve(listener, router(Arc::new(db))).await.unwrap()
}

fn router(db: Arc<Database>) -> Router {
    Router::new().nest(
        "/api",
        Router::new()
            .route("/setup", get(setup_handler))
            .route("/log", post(log_handler))
            .with_state(db),
    )
}
