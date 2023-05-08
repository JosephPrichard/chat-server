/*
 * Copyright (c) Joseph Prichard 2022.
 */

use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use uuid::Uuid;
use serde::{Deserialize, Serialize};

pub const MAX_ROOM_SIZE: usize = 2;

pub type Rooms = Arc<DashMap<Uuid, Arc<Room>>>;

pub struct Room {
    pub room_state: Mutex<RoomState>,
    pub broadcast_tx: broadcast::Sender<String>
}

#[derive(Deserialize, Serialize, Clone)]
pub struct RoomState {
    pub messages: Vec<String>,
    pub count: usize,
    pub last_action: u64
}

pub struct State {
    pub rooms: Rooms,
}

pub type AppState = Arc<State>;

pub fn new_app_state() -> AppState {
    // create the room storage
    let rooms = Arc::new(DashMap::new());
    // create a background task to clear out expired room entries

    // create application state
    Arc::new(State {
        rooms: rooms.clone()
    })
}