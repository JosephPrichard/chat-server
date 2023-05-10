/*
 * Copyright (c) Joseph Prichard 2022.
 */

use dashmap::DashMap;
use std::sync::Arc;
use std::time::Duration;
use dashmap::mapref::multiple::{RefMulti};
use tokio::sync::{broadcast, Mutex};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use tokio::time;
use crate::server::libs::current_time_secs;

pub const MIN_AGE: u64 = 3600;

pub type Rooms = Arc<DashMap<Uuid, Arc<Room>>>;

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

pub struct State {
    pub rooms: Rooms,
}

pub type AppState = Arc<State>;

pub fn new_app_state() -> AppState {
    // create the room storage
    let rooms = Arc::new(DashMap::with_capacity_and_shard_amount(1000, 16));
    // start a cronjob to prune expired rooms from the map
    prune_rooms(rooms.clone());
    // create application state
    Arc::new(State { rooms })
}

pub fn prune_rooms(rooms: Rooms) {
    // create a background task to prune out expired room entries (garbage collection)
    tokio::spawn(async move {
        // create an interval to run this operation every hour
        let mut interval = time::interval(Duration::from_secs(60 * 60));
        loop {
            interval.tick().await;
            // iterate through each room to check if it should be pruned
            for e in rooms.iter() {
                let entry: RefMulti<Uuid, Arc<Room>> = e;
                let pair = entry.pair();
                // check if the last action was old enough to expire the entry
                let last_action = pair.1.room_state.lock().await.last_action;
                if current_time_secs() - last_action > MIN_AGE {
                    rooms.remove(pair.0);
                }
            }
        }
    });
}