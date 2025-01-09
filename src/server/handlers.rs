/*
 * Copyright (c) Joseph Prichard 2022.
 */

use std::fmt::Display;

use axum::extract::ws::{Message, WebSocket};
use futures::SinkExt;
use futures::stream::SplitSink;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast::error::SendError;
use tracing::log::info;
use uuid::Uuid;
use crate::server::websocket::ConnContext;

#[derive(Deserialize)]
pub enum MsgInCode {
    Chat,
    // ... add more message codes
}

// each input message contains this to determine type
#[derive(Deserialize)]
pub struct MsgIn {
    pub msg_code: MsgInCode,
    // ... add more message codes here
}

// input message to represent a chat message
#[derive(Deserialize)]
pub struct ChatMsgIn {
    pub text: String,
}
// .. add more message data structs here

#[derive(Serialize)]
pub enum MsgOutCode {
    Chat,
    Join,
    Leave,
    ChatLog
}

// output message to represent a chat message
#[derive(Serialize)]
pub struct ChatMsgOut<'a> {
    pub msg_code: MsgOutCode,
    pub sender_id: Uuid,
    pub text: &'a String,
}

// output message to represent joining or leaving the room
#[derive(Serialize)]
pub struct RoomMsgOut {
    pub msg_code: MsgOutCode,
    pub sender_id: Uuid,
}

// output message to represent the chat log state
#[derive(Serialize)]
pub struct LogMsgOut<'a> {
    pub msg_code: MsgOutCode,
    pub messages: &'a Vec<String>
}

pub enum HandlerError {
    Serde(serde_json::Error),
    Broadcast(SendError<String>),
    Send(axum::Error),
}

impl Display for HandlerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HandlerError::Serde(e) => write!(f, "Serde Error: {}", e),
            HandlerError::Broadcast(e) => write!(f, "Broadcast Error: {}", e),
            HandlerError::Send(e) => write!(f, "Send Error: {}", e),
        }
    }
}

type SingleSender = SplitSink<WebSocket, Message>;

pub async fn handle_chat(text: String, context: &ConnContext) ->  Result<(), HandlerError> {
    info!("Handling a chat message: {}", text);
    let msg_in: ChatMsgIn = serde_json::from_str(&text).map_err(HandlerError::Serde)?;

    let msg_out = ChatMsgOut {
        msg_code: MsgOutCode::Chat,
        sender_id: context.sender,
        text: &msg_in.text,
    };
    let result_out = serde_json::to_string(&msg_out).map_err(HandlerError::Serde)?;
    context.broadcast(result_out).map_err(HandlerError::Broadcast)?;

    let mut room = context.lock_room().await;
    room.touch();
    room.push_message(msg_in.text);

    Ok(())
}

pub async fn handle_join(joiner_tx: &mut SingleSender, context: &ConnContext) -> Result<(), HandlerError> {
    info!("Handling a joiner: {}", context.sender);

    let join_msg_out = RoomMsgOut {
        msg_code: MsgOutCode::Join,
        sender_id: context.sender
    };
    let join_result_out = serde_json::to_string(&join_msg_out).map_err(HandlerError::Serde)?;
    context.broadcast(join_result_out).map_err(HandlerError::Broadcast)?;

    let mut room = context.lock_room().await;
    room.user_count += 1;

    let log_msg_out = LogMsgOut {
        msg_code: MsgOutCode::ChatLog,
        messages: &room.messages
    };
    let log_result_out = serde_json::to_string(&log_msg_out).map_err(HandlerError::Serde)?;
    joiner_tx.send(Message::Text(log_result_out)).await.map_err(HandlerError::Send)?;

    Ok(())
}

pub async fn handle_leave(context: &ConnContext) -> Result<(), HandlerError> {
    info!("Handling a leaver: {}", context.sender);

    let msg_out = RoomMsgOut {
        msg_code: MsgOutCode::Leave,
        sender_id: context.sender
    };
    let result_out = serde_json::to_string(&msg_out).map_err(HandlerError::Serde)?;
    context.broadcast(result_out).map_err(HandlerError::Broadcast)?;

    context.lock_room().await.user_count -= 1;

    Ok(())
}