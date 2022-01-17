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
        std::env::set_var("RUST_LOG", "streamserver=debug,tower_http=debug")
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
    ReplayMeta { username: String, version: String },
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
    let mut publisher = None;
    let mut replay_buffer = vec![];
    let mut datafiles = None;
    let mut specs = None;
    let mut parser = None;
    loop {
        if let Some(msg) = socket.recv().await {
            if let Ok(msg) = msg {
                match msg {
                    axum::extract::ws::Message::Text(msg) => {
                        // This is a control message
                        match serde_json::from_str(&msg) {
                            Ok(ControlPacket::ReplayMeta { username, version }) => {
                                info!("Got replay meta! username={} version={}", username, version);

                                parser = None;

                                datafiles = Some(
                                    wows_replays::version::Datafiles::new(
                                        std::path::PathBuf::from("versions"),
                                        wows_replays::version::Version::from_client_exe(&version),
                                    )
                                    .unwrap(),
                                );
                                specs = Some(
                                    wows_replays::parse_scripts(datafiles.as_ref().unwrap())
                                        .unwrap(),
                                );

                                // Safety: Parser should be set to None above, before specs is
                                // overwritten. Thus, parser should never exist when specs doesn't
                                // exist. Additionally, specs is a Vec<_>, so the contents are on
                                // the heap. So even if this stack is moved, the entity specs
                                // that parser references will stay at the same address until
                                // they're deleted.
                                let specs: &'static Vec<wows_replays::rpc::entitydefs::EntitySpec> =
                                    unsafe { std::mem::transmute(specs.as_ref().unwrap()) };
                                parser = Some(wows_replays::packet2::Parser::new(specs));

                                let mut broker = crate::broker::BrokerProxy::get();
                                publisher = Some(broker.publish().await);
                                let mut publisher = publisher.as_mut().unwrap();
                                publisher.set_username(&username).await;
                                replay_buffer = vec![];
                            }
                            Ok(ControlPacket::GameOver) => {
                                info!("Got game over!");
                            }
                            Err(e) => {
                                error!("Error {:?} while parsing control message '{}'", e, msg);
                            }
                        }
                    }
                    axum::extract::ws::Message::Binary(mut msg) => {
                        // This is a raw replay data message
                        let msg_len = msg.len();
                        let previous_buf_len = replay_buffer.len();
                        replay_buffer.append(&mut msg);
                        let mut npackets = 0;
                        loop {
                            let (advance, packet) = parser
                                .as_mut()
                                .unwrap()
                                .parse_next_packet(&replay_buffer[..])
                                .unwrap();
                            if packet.is_none() {
                                break;
                            }
                            let packet = packet.unwrap();
                            //debug!("Received packet {:?}", packet);

                            let packet = serde_json::to_string(&packet).unwrap();
                            publisher
                                .as_mut()
                                .unwrap()
                                .upload(packet.into_bytes())
                                .await;

                            replay_buffer.drain(..advance);
                            npackets += 1;
                        }
                        debug!(
                            "Got {} bytes of replay data, buffer has {} bytes, npackets={}",
                            msg_len, previous_buf_len, npackets,
                        );
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

    let mut b = crate::broker::BrokerProxy::get();
    let mut subscriber = b.subscribe(&username).await;

    loop {
        let packet = subscriber.recv().await.unwrap();
        if socket
            .send(Message::Text(
                std::str::from_utf8(&packet).unwrap().to_owned(),
            ))
            .await
            .is_err()
        {
            info!("Subscriber to '{}' disconnected", username);
            return;
        }
    }
}
