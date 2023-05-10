/*
 * Copyright (c) Joseph Prichard 2022.
 */

use axum_sessions::extractors::WritableSession;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type User = Uuid;

pub fn set_user(mut sess_writer: WritableSession, sess: &User) {
    sess_writer
        .insert("session", sess)
        .expect("Could not store the session.");
}

pub fn get_user(sess_writer: WritableSession) -> User {
    let fetched_sess: Option<User> = sess_writer.get("session");
    match fetched_sess {
        Some(sess) => sess,
        None => {
            let guest_sess = Uuid::new_v4();
            set_user(sess_writer, &guest_sess);
            guest_sess.clone()
        }
    }
}