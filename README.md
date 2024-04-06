# ChatServer
A simple web server to handle chat rooms with websockets. 

The web server uses Axum's websocket libraries to communicate over websockets. Since Axum uses Tokio as an async runtime, the server is able to handle concurrent web socket connections.

## Usage

Create a `.env` file in the root directory and set a session token with minimum 64 bytes
```
SESSION_SECRET=<any-string-64-chars-long>
```

Run the server using `cargo run`
```
$ cargo run
Started the server on port 8080...
```

## API

`POST /rooms` => `{ "room_id": "<string>", }`

Create a new room and retrieve the `room_id` of the newly created room in the response

`GET /rooms/socket?room_id=<string>` => `<websocket-conn>`

Establish a websocket connection with a room on the server and send any of the following messages

`{
    "msg_code": "Chat",
    "text": "<string>"
}`

The client can expected to receive any of the following messages

`
{
    "msg_code": "ChatLog",
    "messages": [
        "<string>"
    ]
}
`

`{
    "msg_code": "Chat",
    "sender_id": "<string>",
    "text": "<string>"
}`

`{
    "msg_code": "Join",
    "sender_id": "<string>"
}`

`{
    "msg_code": "Leave",
    "sender_id": "<string>"
}`

## Implementation
Each chat room is stored in a DashMap (concurrency safe sharded hash map) including a chat log, and the last time the room was accessed. Since multiple connections can access a room at the same time, the rooms themseleves are wrapped inside an arc mutex and accessing room state involves locking the mutex. 

Each room contains a broadcast channel that is used to broadcast messages to all connections. The server uses a background task to prune expired entries from the dashmap every hour. 

The background task locks each individual room entry but never the entire DashMap, ensuring the rooms stay available to provide high performance.