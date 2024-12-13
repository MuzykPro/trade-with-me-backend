use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use std::sync::Arc;

use tokio::sync::mpsc;
use uuid::Uuid;

use crate::routes::{SessionId, SharedSessions};

pub async fn handle_socket(
    socket: WebSocket,
    session_id: SessionId,
    sessions: Arc<SharedSessions>,
) {
    let connection_id = Uuid::new_v4();

    let (tx, mut rx) = mpsc::channel(32);

    sessions.add_client(session_id, connection_id, tx);

    let (mut ws_sink, mut ws_stream) = socket.split();

    let write_handle = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_sink.send(Message::Text(msg)).await.is_err() {
                // If send fails, client disconnected
                break;
            }
        }
    });

    let read_handle = tokio::spawn({
        let sessions = Arc::clone(&sessions);
        async move {
            while let Some(Ok(msg)) = ws_stream.next().await {
                match msg {
                    Message::Text(text) => {
                        println!("Received from client {}: {}", connection_id, text);
                        sessions.broadcast(&session_id, &format!("Echo: {}", text));
                    }
                    Message::Close(_frame) => {
                        println!(
                            "Client {} disconnected from session {}",
                            connection_id, session_id
                        );
                        break;
                    }
                    _ => {}
                }
            }
        }
    });

    let _ = tokio::join!(write_handle, read_handle);

    sessions.remove_client(&session_id, &connection_id);
    println!(
        "Removed client {} from session {}",
        connection_id, session_id
    );
}
