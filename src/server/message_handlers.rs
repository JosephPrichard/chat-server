/*
 * Copyright (c) Joseph Prichard 2022.
 */

use axum::extract::ws::{Message, WebSocket};
use futures::SinkExt;
use futures::stream::SplitSink;
use serde::{Deserialize, Serialize};
use tracing::log::info;
use uuid::Uuid;
use crate::server::libs::Res;
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
pub struct ChatMsgOut {
    pub msg_code: MsgOutCode,
    pub sender_id: Uuid,
    pub text: String,
}

// output message to represent joining or leaving the room
#[derive(Serialize)]
pub struct RoomMsgOut {
    pub msg_code: MsgOutCode,
    pub sender_id: Uuid,
}

// output message to represent joining or leaving the room
#[derive(Serialize)]
pub struct ChatLogMsgOut<'a> {
    pub msg_code: MsgOutCode,
    pub messages: &'a Vec<String>
}

type SingleSender = SplitSink<WebSocket, Message>;

pub async fn handle_chat(text: String, context: &ConnContext) -> Res<()> {
    info!("Handling a chat message: {}", text);
    let msg_in: ChatMsgIn = serde_json::from_str(&text)?;
    // serialize then broadcast chat message
    let msg_out = ChatMsgOut {
        msg_code: MsgOutCode::Chat,
        sender_id: context.sender,
        text: msg_in.text,
    };
    // broadcast the result to all members of the room
    let result_out = serde_json::to_string(&msg_out)?;
    context.broadcast(result_out)?;
    // update the room to not be expired
    context.touch_room().await;
    // add the message to the room
    context.push_message(msg_out.text).await;
    Ok(())
}

pub async fn handle_join(joiner_tx: &mut SingleSender, context: &ConnContext) -> Res<()> {
    info!("Handling a joiner: {}", context.sender);
    // increment count because new joiner
    context.inc_user_count().await;
    // serialize then broadcast chat message
    let join_msg_out = RoomMsgOut {
        msg_code: MsgOutCode::Join,
        sender_id: context.sender
    };
    // broadcast the result to all members of the room
    let join_result_out = serde_json::to_string(&join_msg_out)?;
    context.broadcast(join_result_out)?;
    // get a mutex guard to the room
    let room = context.lock_room().await;
    // serialize the current room state then send to the joiner
    let cl_msg_out = ChatLogMsgOut {
        msg_code: MsgOutCode::ChatLog,
        messages: &room.messages
    };
    let cl_result_out = serde_json::to_string(&cl_msg_out)?;
    joiner_tx.send(Message::Text(cl_result_out)).await?;
    Ok(())
}

pub async fn handle_leave(context: &ConnContext) -> Res<()> {
    info!("Handling a leaver: {}", context.sender);
    // decrement count because leaver
    context.dec_user_count().await;
    // serialize then broadcast chat message
    let msg_out = RoomMsgOut {
        msg_code: MsgOutCode::Leave,
        sender_id: context.sender
    };
    // broadcast the result to all members of the room
    let result_out = serde_json::to_string(&msg_out)?;
    context.broadcast(result_out)?;
    Ok(())
}