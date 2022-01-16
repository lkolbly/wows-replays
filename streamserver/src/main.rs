use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        TypedHeader,
    },
    http::StatusCode,
    response::IntoResponse,
    routing::{get, get_service},
    Router,
};
use serde_derive::{Deserialize, Serialize};
use std::net::SocketAddr;
use tower_http::{
    services::ServeDir,
    trace::{DefaultMakeSpan, TraceLayer},
};
use tracing::*;

mod broker;

#[tokio::main]
async fn main() {
    // Set the RUST_LOG, if it hasn't been explicitly defined
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "example_websockets=debug,tower_http=debug")
    }
    tracing_subscriber::fmt::init();

    // build our application with some routes
    let app = Router::new()
        .fallback(
            get_service(
                ServeDir::new("streamserver/assets").append_index_html_on_directories(true),
            )
            .handle_error(|error: std::io::Error| async move {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Unhandled internal error: {}", error),
                )
            }),
        )
        // routes are matched from bottom to top, so we have to put `nest` at the
        // top since it matches all routes
        .route("/upload", get(upload_handler))
        .route("/subscribe", get(subscribe_handler))
        // logging so we can see whats going on
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        );

    // run it with hyper
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

#[derive(Deserialize)]
enum ControlPacket {
    ReplayMeta(wows_replays::ReplayMeta),
    //ReplayData(Vec<u8>),
    GameOver,
}

async fn upload_handler(
    ws: WebSocketUpgrade,
    user_agent: Option<TypedHeader<headers::UserAgent>>,
) -> impl IntoResponse {
    if let Some(TypedHeader(user_agent)) = user_agent {
        println!("`{}` connected", user_agent.as_str());
    }

    ws.on_upgrade(handle_upload_socket)
}

async fn handle_upload_socket(mut socket: WebSocket) {
    loop {
        if let Some(msg) = socket.recv().await {
            if let Ok(msg) = msg {
                match msg {
                    axum::extract::ws::Message::Text(msg) => {
                        // This is a control message
                        match serde_json::from_str(&msg) {
                            Ok(ControlPacket::ReplayMeta(meta)) => {
                                info!("Got replay meta!");
                            }
                            /*Ok(ControlPacket::ReplayData(data)) => {
                                info!("Got");
                            }*/
                            Ok(ControlPacket::GameOver) => {
                                info!("Got game over!");
                            }
                            Err(e) => {
                                error!("Error {:?} while parsing control message '{}'", e, msg);
                            }
                        }
                    }
                    axum::extract::ws::Message::Binary(msg) => {
                        // This is a raw replay data message
                        info!("Got replay data!");
                    }
                    msg => {
                        error!("Unrecognized upload message {:?}", msg);
                    }
                }
            } else {
                info!("Uploading client disconnected");
                return;
            }
        }
    }

    /*if let Some(msg) = socket.recv().await {
        if let Ok(msg) = msg {
            println!("Client says: {:?}", msg);
        } else {
            println!("client disconnected");
            return;
        }
    }

    loop {
        if socket
            .send(Message::Text(String::from("Hi!")))
            .await
            .is_err()
        {
            println!("client disconnected");
            return;
        }
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }*/
}

async fn subscribe_handler(
    ws: WebSocketUpgrade,
    user_agent: Option<TypedHeader<headers::UserAgent>>,
) -> impl IntoResponse {
    if let Some(TypedHeader(user_agent)) = user_agent {
        println!("`{}` connected", user_agent.as_str());
    }

    ws.on_upgrade(handle_subscribe_socket)
}

async fn handle_subscribe_socket(mut socket: WebSocket) {
    // First, read the username they're subscribing to
    let username = if let Some(msg) = socket.recv().await {
        if let Ok(msg) = msg {
            println!("Client says: {:?}", msg);
            match msg {
                axum::extract::ws::Message::Text(s) => s,
                msg => {
                    error!(
                        "Got unexpected first message {:?} in subscribe connection!",
                        msg
                    );
                    return;
                }
            }
        } else {
            println!("client disconnected");
            return;
        }
    } else {
        println!("Client disconnected");
        return;
    };

    loop {
        if socket
            .send(Message::Text(String::from("Hi!")))
            .await
            .is_err()
        {
            println!("client disconnected");
            return;
        }
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }
}
