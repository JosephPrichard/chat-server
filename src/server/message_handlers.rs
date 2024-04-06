/*
 * Copyright (c) Joseph Prichard 2022.
 */

use axum::extract::ws::{Message, WebSocket};
use futures::SinkExt;
use futures::stream::SplitSink;
use serde::{Deserialize, Serialize};
use tracing::log::info;
use uuid::Uuid;
use crate::server::socket_server::ConnContext;

#[derive(Deserialize)]
pub enum MsgInCode {
    Chat,
}

// each input message contains this to determine type
#[derive(Deserialize)]
pub struct MsgIn {
    pub msg_code: MsgInCode,
}

// input message to represent a chat message
#[derive(Deserialize)]
pub struct ChatMsgIn {
    pub text: String,
}

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
pub struct ChatLogMsgOut<'a> {
    pub msg_code: MsgOutCode,
    pub messages: &'a Vec<String>
}

type SingleSender = SplitSink<WebSocket, Message>;

pub async fn handle_chat(text: String, context: &ConnContext) ->  Result<(), Box<dyn std::error::Error>> {
    info!("Handling a chat message: {}", text);
    let msg_in: ChatMsgIn = serde_json::from_str(&text)?;

    let msg_out = ChatMsgOut {
        msg_code: MsgOutCode::Chat,
        sender_id: context.sender,
        text: &msg_in.text,
    };
    let result_out = serde_json::to_string(&msg_out)?;
    context.broadcast(result_out)?;

    let mut room = context.lock_room().await;
    room.touch();
    room.push_message(msg_in.text);

    Ok(())
}

pub async fn handle_join(joiner_tx: &mut SingleSender, context: &ConnContext) -> Result<(), Box<dyn std::error::Error>> {
    info!("Handling a joiner: {}", context.sender);

    let join_msg_out = RoomMsgOut {
        msg_code: MsgOutCode::Join,
        sender_id: context.sender
    };
    let join_result_out = serde_json::to_string(&join_msg_out)?;
    context.broadcast(join_result_out)?;

    let mut room = context.lock_room().await;
    room.user_count += 1;

    let cl_msg_out = ChatLogMsgOut {
        msg_code: MsgOutCode::ChatLog,
        messages: &room.messages
    };
    let cl_result_out = serde_json::to_string(&cl_msg_out)?;
    joiner_tx.send(Message::Text(cl_result_out)).await?;

    Ok(())
}

pub async fn handle_leave(context: &ConnContext) -> Result<(), Box<dyn std::error::Error>> {
    info!("Handling a leaver: {}", context.sender);

    let msg_out = RoomMsgOut {
        msg_code: MsgOutCode::Leave,
        sender_id: context.sender
    };
    let result_out = serde_json::to_string(&msg_out)?;
    context.broadcast(result_out)?;

    context.lock_room().await.user_count -= 1;

    Ok(())
}