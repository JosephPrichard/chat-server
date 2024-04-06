/*
 * Copyright (c) Joseph Prichard 2022.
 */

use uuid::Uuid;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use axum::extract::{Query, State, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::Json;
use axum::response::{IntoResponse, Response};
use axum_sessions::extractors::WritableSession;
use tracing::log::info;
use crate::server::session::get_user;
use crate::server::state::{AppState, Room, RoomState};
use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, stream::StreamExt};
use tokio::sync::broadcast::error::SendError;
use tokio::sync::{broadcast, Mutex, MutexGuard};
use tracing::log::warn;
use crate::server::message_handlers::{handle_chat, handle_join, handle_leave, MsgIn, MsgInCode};
use crate::server::session::User;

pub const MAX_ROOM_SIZE: usize = 12;

#[derive(Deserialize, Serialize)]
pub struct RoomId {
    pub room_id: String,
}

#[derive(Clone)]
pub struct ConnContext {
    pub room: Arc<Room>,
    pub sender: User,
}

impl ConnContext {
    pub fn broadcast(&self, text: String) -> Result<usize, SendError<String>> {
        self.room.broadcast_tx.send(text)
    }

    pub async fn lock_room(&self) -> MutexGuard<RoomState> {
        self.room.room_state.lock().await
    }
}

#[derive(Deserialize, Serialize)]
pub struct ErrorRes {
    message: String
}

pub fn error_res(code: StatusCode, message: &str) -> Response {
    (code, Json(ErrorRes { message: String::from(message)})).into_response()
}

pub async fn handle_create_room(State(state): State<AppState>) -> Json<RoomId> {
    let uuid = Uuid::new_v4();
    let current_time_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards???")
        .as_secs();

    state.rooms.insert(uuid, Arc::new(Room {
        broadcast_tx: broadcast::channel(128).0,
        room_state: Mutex::new(RoomState {
            messages: vec![],
            user_count: 0,
            last_action: current_time_secs,
        }),
    }));

    info!("Created a room {}", uuid.to_string());
    Json(RoomId { room_id: uuid.to_string() })
}

pub async fn handle_ws_upgrade(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(params): Query<RoomId>,
    sess_writer: WritableSession,
) -> Response {
    match Uuid::parse_str(&params.room_id) {
        Err(_) => error_res(StatusCode::BAD_REQUEST, "Invalid uuid format"),
        Ok(room_id) => match state.rooms.get(&room_id) {
            None => error_res(StatusCode::NOT_FOUND, "No room exists for id"),
            Some(room) => {
                let sender = get_user(sess_writer);
                let context = ConnContext {
                    room: room.clone(),
                    sender,
                };
                if context.lock_room().await.user_count >= MAX_ROOM_SIZE {
                    return error_res(StatusCode::FORBIDDEN, "Room is full");
                }
                info!("Upgrading websocket for user {}", sender);
                ws.on_upgrade(move |socket| handle_socket(socket, context))
            }
        }
    }    
}

pub async fn handle_socket(ws: WebSocket, context: ConnContext) {
    let (mut tx, mut rx) = ws.split();

    if let Err(e) = handle_join(&mut tx, &context).await {
        warn!("Error occurred in joining: {}", e.to_string());
    };

    // spawn a task to subscribe to the broadcast channel and forward broadcast requests to client
    let room = context.room.clone();
    let mut send_task = tokio::spawn(async move {
        let mut room_rx = room.broadcast_tx.subscribe();

        while let Ok(text) = room_rx.recv().await {
            if let Err(e) = tx.send(Message::Text(text)).await {
                warn!("Error occurred in broadcast forwarding: {}", e.to_string())
            }
        }
    });

    // spawn a task to handle each message from the websocket receiver
    let recv_context = context.clone();
    let mut recv_task = tokio::spawn(async move {

        while let Some(Ok(Message::Text(text))) = rx.next().await {
            if let Err(e) = handle_socket_recv(text, &recv_context).await {
                warn!("Error occurred in recv: {}", e.to_string())
            }
        }
    });

    // if any one of the tasks exit, abort the other
    tokio::select! {
        _ = (&mut send_task) => {
            recv_task.abort()
        },
        _ = (&mut recv_task) => {
            // if the receiver aborts, broadcast using sender that they left
            if let Err(e) = handle_leave(&context).await {
                warn!("Error occurred in handling leave: {}", e.to_string())
            }
            send_task.abort()
        },
    }
}

async fn handle_socket_recv(text: String, context: &ConnContext) -> Result<(), Box<dyn std::error::Error>> {
    let msg: MsgIn = serde_json::from_str(&text)?;
    match msg.msg_code {
        MsgInCode::Chat => handle_chat(text, context).await
    }
}