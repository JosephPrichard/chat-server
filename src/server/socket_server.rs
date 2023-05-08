/*
 * Copyright (c) Joseph Prichard 2022.
 */

use uuid::Uuid;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use axum::extract::{Query, State, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::Json;
use axum::response::{Response};
use axum_sessions::extractors::WritableSession;
use tracing::log::info;
use crate::server::libs::{current_time_secs, error_res, Res};
use crate::server::session::{get_user_or_guest};
use crate::server::state::{AppState, Room, RoomState};
use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, stream::StreamExt};
use tokio::sync::broadcast::error::SendError;
use tokio::sync::{broadcast, Mutex, MutexGuard};
use tracing::log::warn;
use crate::server::socket_handlers::{handle_chat, handle_join, handle_leave, MsgIn, MsgInCode};
use crate::server::session::User;

pub const MAX_ROOM_SIZE: usize = 2;

#[derive(Deserialize, Serialize)]
pub struct RoomId {
    pub room_id: String,
}

pub struct SocketContext {
    pub room: Arc<Room>,
    pub sender: User,
}

impl SocketContext {
    pub async fn broadcast(&self, text: String) -> Result<usize, SendError<String>> {
        self.room.broadcast_tx.send(text)
    }

    pub async fn lock_room(&self) -> MutexGuard<RoomState> {
        self.room.room_state.lock().await
    }

    pub async fn messages(&self) -> Vec<String> {
        return self.lock_room().await.messages.clone()
    }

    pub async fn touch_room(&self) {
        self.lock_room().await.last_action = current_time_secs()
    }

    pub async fn push_message(&self, text: String) {
        self.lock_room().await.messages.push(text)
    }

    pub async fn inc_count(&self) {
        self.lock_room().await.count += 1;
    }

    pub async fn dec_count(&self) {
        self.lock_room().await.count -= 1;
    }

    pub async fn count(&self) -> usize {
        return self.lock_room().await.count;
    }
}

pub async fn handle_create_room(State(state): State<AppState>) -> Json<RoomId> {
    let uuid = Uuid::new_v4();
    // create a new room
    let room = Room {
        broadcast_tx: broadcast::channel(128).0,
        room_state: Mutex::new(RoomState {
            messages: vec![],
            count: 0,
            last_action: current_time_secs(),
        }),
    };
    // insert the room into the map of all rooms
    state.rooms.insert(uuid, Arc::new(room));
    info!("Created a room {}", uuid.to_string());
    Json(RoomId { room_id: uuid.to_string() })
}

// handles a ws upgrade by passing the room to the ws handler
pub async fn handle_ws_upgrade(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(params): Query<RoomId>,
    sess_writer: WritableSession,
) -> Response {
    // get the user session to pass to ws context
    let sender = get_user_or_guest(sess_writer);
    // pass an arc to the room to the websocket handler function
    let room_id = Uuid::parse_str(&params.room_id);
    if room_id.is_err() {
        return error_res(StatusCode::BAD_REQUEST, "Invalid uuid format");
    }
    // get the room, and return an error if none
    let room = state.rooms.get(&room_id.unwrap());
    if room.is_none() {
        return error_res(StatusCode::NOT_FOUND, "No room exists for id");
    }
    // create the socket context
    let context = SocketContext {
        room: room.unwrap().clone(),
        sender: sender.clone(),
    };
    // check if the room is full or not, and if so return an error
    if context.count().await >= MAX_ROOM_SIZE {
        return error_res(StatusCode::FORBIDDEN, "Room is full");
    }
    // do upgrade on the websocket
    info!("Upgrading websocket for user {}", sender);
    ws.on_upgrade(move |socket| handle_socket(socket, context))
}

pub async fn handle_socket(ws: WebSocket, context: SocketContext) {
    let (mut tx, mut rx) = ws.split();
    let shared_context = Arc::new(context);

    // handle user joining the room
    if let Err(e) = handle_join(&mut tx, &shared_context).await {
        warn!("Error occurred in joining: {}", e.to_string());
    };

    // spawn a task to subscribe to the broadcast channel and forward broadcast requests to client
    let room = shared_context.clone().room.clone();
    let mut send_task = tokio::spawn(async move {
        // subscribe to the broadcast channel
        let mut room_rx = room.broadcast_tx.subscribe();
        // take messages off the recv and forward them to the client
        while let Ok(text) = room_rx.recv().await {
            if let Err(e) = tx.send(Message::Text(text)).await {
                warn!("Error occurred in broadcast forwarding: {}", e.to_string())
            }
        }
    });

    // spawn a task to handle each message from the websocket receiver
    let recv_context = shared_context.clone();
    let mut recv_task = tokio::spawn(async move {
        // take messages off the recv and handle them on the server
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
            if let Err(e) = handle_leave(&shared_context.clone()).await {
                warn!("Error occurred in handling leave: {}", e.to_string())
            }
            send_task.abort()
        },
    }
}

async fn handle_socket_recv(text: String, context: &SocketContext) -> Res<()> {
    // deserialize message type, and handle any errors
    let msg: MsgIn = serde_json::from_str(&text)?;
    // call the corresponding message handler
    match msg.msg_code {
        MsgInCode::Chat => handle_chat(text, context).await
    }
}