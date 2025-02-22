/*
 * Copyright (c) Joseph Prichard 2022.
 */

use dashmap::DashMap;
use std::{sync::Arc, time::SystemTime};
use std::time::{Duration, UNIX_EPOCH};
use dashmap::mapref::multiple::RefMulti;
use tokio::sync::{broadcast, Mutex};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use tokio::time;

pub const MIN_AGE: u64 = 3600;

pub type AppState = Arc<State>;

pub type State = Arc<DashMap<Uuid, Arc<Room>>>;

pub struct Room {
    pub room_state: Mutex<RoomState>,
    pub broadcast_tx: broadcast::Sender<String>
}

#[derive(Deserialize, Serialize, Clone)]
pub struct RoomState {
    pub messages: Vec<String>,
    pub user_count: usize,
    pub last_action: u64
}

impl RoomState {
    pub fn touch(&mut self) {
        self.last_action = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards???")
            .as_secs()
    }

    pub fn push_message(&mut self, text: String) {
        self.messages.push(text)
    }
}

pub fn new_app_state() -> AppState {
    let rooms = Arc::new(DashMap::with_capacity_and_shard_amount(1000, 16));
    prune_rooms(rooms.clone());
    Arc::new(rooms)
}

pub fn prune_rooms(state: State) {
    // create a background task to prune out expired room entries (garbage collection)
    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(60 * 60)); // every hour
        loop {
            interval.tick().await;
            for e in state.iter() {
                let entry: RefMulti<Uuid, Arc<Room>> = e;
                let pair = entry.pair();
                
                let last_action = pair.1.room_state.lock().await.last_action;

                let current_time_secs = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("Time went backwards???")
                    .as_secs();
                if current_time_secs - last_action > MIN_AGE {
                    state.remove(pair.0);
                }
            }
        }
    });
}