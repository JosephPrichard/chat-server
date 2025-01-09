/*
 * Copyright (c) Joseph Prichard 2022.
 */

use std::env;
use axum::Router;
use axum::routing::{get, post};
use axum_sessions::{async_session, SessionLayer};
use dotenv::dotenv;
use crate::server::websocket::{handle_create_room, handle_ws_upgrade};
use crate::server::state::new_app_state;

mod server;

#[tokio::main]
async fn main() {
    let rooms = new_app_state();

    dotenv().ok();
    let session_secret = env::var("SESSION_SECRET").unwrap();

    tracing_subscriber::fmt::init();

    println!("Started the server on port 8080...");

    let store = async_session::MemoryStore::new();
    let session_layer = SessionLayer::new(store, session_secret.as_bytes());

    let app = Router::new()
        .route("/rooms", post(handle_create_room))
        .route("/rooms/socket", get(handle_ws_upgrade))
        .with_state(rooms.clone())
        .layer(session_layer);

    axum::Server::bind(&"0.0.0.0:8080".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
