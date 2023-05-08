/*
 * Copyright (c) Joseph Prichard 2022.
 */

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;
use axum::http::{StatusCode};
use axum::Json;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct ErrorRes {
    message: String
}

pub type Shared<T> = Arc<Mutex<T>>;

pub type Res<T> = Result<T, Box<dyn std::error::Error>>;

pub fn current_time() -> Duration {
    SystemTime::now().duration_since(UNIX_EPOCH).expect("Time went backwards???")
}

pub fn current_time_millis() -> u128 {
    current_time().as_millis()
}

pub fn current_time_secs() -> u64 {
    current_time().as_secs()
}

pub fn ok_res<T: Serialize>(body: T) -> Response {
    (StatusCode::OK, Json(body)).into_response()
}

pub fn error_res(code: StatusCode, message: &str) -> Response {
    (code, Json(ErrorRes { message: String::from(message)})).into_response()
}