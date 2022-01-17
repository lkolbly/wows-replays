use serde_derive::Serialize;
use websocket::ClientBuilder;

// TODO: Single-source this with streamserver
#[derive(Serialize)]
enum ControlPacket {
    ReplayMeta { username: String, version: String },
    //ReplayData(Vec<u8>),
    GameOver,
}

fn main() {
    println!("Hello, world!");

    let mut client = ClientBuilder::new("ws://127.0.0.1:3000/upload")
        .unwrap()
        .connect_insecure()
        .unwrap();

    // Read a replay to upload
    /*let mut contents = vec![];
    let f = std::fs::File::open("./test/replays/version-3747819.wowsreplay").unwrap();
    f.read_to_end(&mut contents).unwrap();*/

    let replayfile = wows_replays::ReplayFile::from_file(&std::path::PathBuf::from(
        "./test/replays/version-3747819.wowsreplay",
    ))
    .unwrap();

    println!("{:?}", replayfile.meta);

    client
        .send_message(&websocket::OwnedMessage::Text(
            serde_json::to_string(&ControlPacket::ReplayMeta {
                username: replayfile.meta.playerName.clone(),
                version: replayfile.meta.clientVersionFromExe.clone(),
            })
            .unwrap(),
        ))
        .unwrap();

    // Upload 870 bytes ten times per second to roughly approximate a real upload
    let chunk_size = 870;
    let mut offset = 0;
    while offset < replayfile.packet_data.len() {
        let end = if offset + chunk_size > replayfile.packet_data.len() {
            replayfile.packet_data.len()
        } else {
            offset + chunk_size
        };
        let to_send = &replayfile.packet_data[offset..end];
        client
            .send_message(&websocket::OwnedMessage::Binary(to_send.to_owned()))
            .unwrap();
        offset += chunk_size;
        std::thread::sleep_ms(100);
    }

    client
        .send_message(&websocket::OwnedMessage::Text(
            serde_json::to_string(&ControlPacket::GameOver).unwrap(),
        ))
        .unwrap();
}
