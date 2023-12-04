use clap::{App, Arg};
use rocket::State;
use rust_embed::RustEmbed;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Mutex;
use tera::{Context, Tera};
use wows_replays::analyzer::decoder::DecodedPacketPayload;
use wows_replays::analyzer::AnalyzerBuilder;
use wows_replays::packet2::Packet;
use wows_replays::parse_scripts;
use wows_replays::ReplayFile;

#[macro_use]
extern crate rocket;

#[macro_use]
extern crate lazy_static;

struct ServerConfig {
    webroot: String,
}

#[derive(Serialize, Clone)]
struct ReplayInfo {
    username: String,
    date: String,
    ship: String,
    map: String,
    version: wows_replays::version::Version,
    hash: String,
    path: std::path::PathBuf,
    victory: Option<bool>,
    num_packets: usize,
    player_team: i64,
}

impl wows_replays::packet2::PacketProcessor for ReplayInfo {
    fn process(&mut self, packet: Packet<'_, '_>) {
        let packet =
            wows_replays::analyzer::decoder::DecodedPacket::from(&self.version, false, &packet);
        match &packet.payload {
            DecodedPacketPayload::OnArenaStateReceived { players, .. } => {
                for player in players.iter() {
                    if player.username == self.username {
                        self.player_team = player.teamid;
                        break;
                    }
                }
            }
            DecodedPacketPayload::BattleEnd { winning_team, .. } => {
                if self.player_team != -1 {
                    self.victory = Some(*winning_team as i64 == self.player_team);
                }
            }
            _ => {}
        }
        self.num_packets += 1;
    }
}

impl ReplayInfo {
    fn new(path: &std::path::PathBuf, hash: String, meta: &wows_replays::ReplayMeta) -> ReplayInfo {
        ReplayInfo {
            username: meta.playerName.clone(),
            date: meta.dateTime.clone(),
            ship: meta.playerVehicle.clone(),
            map: meta.mapDisplayName.clone(),
            hash: hash,
            path: path.clone(),
            version: wows_replays::version::Version::from_client_exe(&meta.clientVersionFromExe),
            victory: None,
            num_packets: 0,
            player_team: -1,
        }
    }

    fn from(replay: &std::path::PathBuf) -> Result<ReplayInfo, wows_replays::ErrorKind> {
        let replay_file = ReplayFile::from_file(replay)?;

        let datafiles = wows_replays::version::EmbeddedDataFiles::new(
            std::path::PathBuf::from("versions"),
            wows_replays::version::Version::from_client_exe(&replay_file.meta.clientVersionFromExe),
        )?;
        let specs = parse_scripts(&datafiles)?;
        let version_parts: Vec<_> = replay_file.meta.clientVersionFromExe.split(",").collect();
        assert!(version_parts.len() == 4);

        // Parse packets
        let mut p = wows_replays::packet2::Parser::new(&specs);

        let mut processor = ReplayInfo::new(
            replay,
            replay.file_name().unwrap().to_str().unwrap().to_string(),
            &replay_file.meta,
        );
        p.parse_packets(&replay_file.packet_data, &mut processor);
        Ok(processor)
    }
}

impl<'a> rocket::request::FromParam<'a> for ReplayInfo {
    type Error = ();

    fn from_param(hash: &'a str) -> Result<Self, Self::Error> {
        let database = DATABASE.lock().unwrap();
        let replay = match database.replays.get(hash) {
            Some(x) => x,
            None => {
                panic!("foo");
            }
        };
        let replay = replay.as_ref().unwrap();
        Ok(replay.clone())
    }
}

struct Database {
    replays: HashMap<String, Result<ReplayInfo, wows_replays::ErrorKind>>,
}

impl Database {
    fn new() -> Self {
        Database {
            replays: HashMap::new(),
        }
    }
}

fn file_watcher(path: String, file_sink: Sender<std::path::PathBuf>) {
    let mut seen = HashSet::new();

    loop {
        for entry in walkdir::WalkDir::new(&path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let hash = format!("{}", entry.path().display());
            if seen.insert(hash.clone()) {
                TOTAL_FILES.fetch_add(1, Ordering::SeqCst);
                file_sink.send(entry.into_path()).unwrap();
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(20));
    }
}

fn replay_updater(file_source: Receiver<std::path::PathBuf>) {
    loop {
        match file_source.recv() {
            Ok(path) => {
                println!("Got {:?}", path);
                let hash = path.file_name().unwrap().to_str().unwrap().to_string();
                let replay_info = ReplayInfo::from(&path);
                let mut database = DATABASE.lock().unwrap();
                database.replays.insert(hash, replay_info);
            }
            Err(e) => {
                println!("Replay error: {:?}", e);
                break;
            }
        }
    }
}

lazy_static! {
    static ref DATABASE: Mutex<Database> = Mutex::new(Database::new());
    static ref TOTAL_FILES: AtomicUsize = AtomicUsize::new(0);
}

#[get("/<name>/<age>")]
fn hello(name: &str, age: u8) -> String {
    let database = DATABASE.lock().unwrap();
    format!(
        "Hello, {} year old named {}! There are {} replays, but {} total files have been found",
        age,
        name,
        database.replays.len(),
        TOTAL_FILES.load(Ordering::Relaxed),
    )
}

#[derive(RustEmbed)]
#[folder = "templates"]
struct Templates;

#[get("/page/<pageid>")]
fn page(pageid: u32, config: &State<ServerConfig>) -> rocket::response::content::Html<String> {
    let mut tera = Tera::default();
    for fname in Templates::iter() {
        let content = Templates::get(&fname).unwrap();
        tera.add_raw_template(&fname, std::str::from_utf8(&content.data).unwrap())
            .unwrap();
    }

    let mut context = Context::new();
    context.insert("root", &config.webroot);

    {
        let database = DATABASE.lock().unwrap();
        let mut games = vec![];
        for (_, replay) in database.replays.iter() {
            if replay.is_ok() {
                games.push(replay.clone().as_ref().ok().clone().unwrap());
            }
        }
        games.sort_by_key(|replay| {
            match chrono::NaiveDateTime::parse_from_str(&replay.date, "%d.%m.%Y %H:%M:%S") {
                Ok(x) => x,
                Err(e) => {
                    println!("Couldn't parse '{}' because {:?}", replay.date, e);
                    chrono::NaiveDateTime::parse_from_str(
                        "05.05.1995 01:02:03",
                        "%d.%m.%Y %H:%M:%S",
                    )
                    .unwrap()
                }
            }
        });
        games.reverse();
        context.insert("games", &games);
    } // Unlock the DB before we render (and potentially panic)

    rocket::response::content::Html(tera.render("page.html.tera", &context).unwrap())
}

struct DownloadResponder {
    path: std::path::PathBuf,
    filename: String,
}

impl<'r> rocket::response::Responder<'r, 'r> for DownloadResponder {
    fn respond_to(self, _: &'r rocket::request::Request<'_>) -> rocket::response::Result<'r> {
        let mut content = vec![];
        let mut f = std::fs::File::open(self.path).unwrap();
        f.read_to_end(&mut content).unwrap();

        rocket::response::Response::build()
            .raw_header(
                "Content-Disposition",
                format!("attachment; filename={}", self.filename),
            )
            .sized_body(None, std::io::Cursor::new(content))
            .ok()
    }
}

#[get("/damage_trails/<replay>")]
fn damage_trails(replay: ReplayInfo) -> (rocket::http::ContentType, Vec<u8>) {
    {
        let replay_file = ReplayFile::from_file(&replay.path).unwrap();

        let datafiles = wows_replays::version::EmbeddedDataFiles::new(
            std::path::PathBuf::from("versions"),
            wows_replays::version::Version::from_client_exe(&replay_file.meta.clientVersionFromExe),
        )
        .unwrap();
        let specs = parse_scripts(&datafiles).unwrap();
        let version_parts: Vec<_> = replay_file.meta.clientVersionFromExe.split(",").collect();
        assert!(version_parts.len() == 4);

        let processor = analysis::damage_trails::DamageTrailsBuilder::new("foo.png");
        let processor = processor.build(&replay_file.meta);

        // Parse packets
        let mut p = wows_replays::packet2::Parser::new(&specs);

        let mut analyzer_set = wows_replays::analyzer::AnalyzerAdapter::new(vec![processor]);
        p.parse_packets(&replay_file.packet_data, &mut analyzer_set)
            .unwrap();
        analyzer_set.finish();
    }

    let mut content = vec![];
    let mut f = std::fs::File::open("foo.png").unwrap();
    f.read_to_end(&mut content).unwrap();
    (
        rocket::http::ContentType::from_extension("png").unwrap(),
        content,
    )
}

#[get("/trails/<replay>")]
fn trails(replay: ReplayInfo) -> (rocket::http::ContentType, Vec<u8>) {
    {
        let replay_file = ReplayFile::from_file(&replay.path).unwrap();

        let datafiles = wows_replays::version::EmbeddedDataFiles::new(
            std::path::PathBuf::from("versions"),
            wows_replays::version::Version::from_client_exe(&replay_file.meta.clientVersionFromExe),
        )
        .unwrap();
        let specs = parse_scripts(&datafiles).unwrap();
        let version_parts: Vec<_> = replay_file.meta.clientVersionFromExe.split(",").collect();
        assert!(version_parts.len() == 4);

        let processor = analysis::trails::TrailsBuilder::new("/tmp/tmp.png");
        let processor = processor.build(&replay_file.meta);

        // Parse packets
        let mut p = wows_replays::packet2::Parser::new(&specs);

        let mut analyzer_set = wows_replays::analyzer::AnalyzerAdapter::new(vec![processor]);
        p.parse_packets(&replay_file.packet_data, &mut analyzer_set)
            .unwrap();
        analyzer_set.finish();
    }

    let mut content = vec![];
    let mut f = std::fs::File::open("/tmp/tmp.png").unwrap();
    f.read_to_end(&mut content).unwrap();
    (
        rocket::http::ContentType::from_extension("png").unwrap(),
        content,
    )
}

#[get("/download/<replay>")]
fn download(replay: ReplayInfo) -> DownloadResponder {
    DownloadResponder {
        path: replay.path.clone(),
        filename: format!(
            "{}-{}-{}-{}-{}.wowsreplay",
            replay.date,
            replay.ship,
            replay.map,
            replay.username,
            &replay.hash[0..10]
        ),
    }
}

struct DecodedResponder {
    filename: String,
    version: wows_replays::version::Version,
    result: String,
}

impl wows_replays::packet2::PacketProcessor for DecodedResponder {
    fn process(&mut self, packet: Packet<'_, '_>) {
        let packet =
            wows_replays::analyzer::decoder::DecodedPacket::from(&self.version, false, &packet);
        let encoded = serde_json::to_string(&packet).unwrap();
        self.result.push_str("\n");
        self.result.push_str(&encoded);
    }
}

impl<'r> rocket::response::Responder<'r, 'r> for DecodedResponder {
    fn respond_to(self, _: &'r rocket::request::Request<'_>) -> rocket::response::Result<'r> {
        rocket::response::Response::build()
            .raw_header(
                "Content-Disposition",
                format!("attachment; filename={}", self.filename),
            )
            .sized_body(None, std::io::Cursor::new(self.result))
            .ok()
    }
}

#[get("/decoded/<replay>")]
fn download_decoded(replay: ReplayInfo) -> DecodedResponder {
    let replay_file = ReplayFile::from_file(&replay.path).unwrap();

    let datafiles = wows_replays::version::EmbeddedDataFiles::new(
        std::path::PathBuf::from("versions"),
        wows_replays::version::Version::from_client_exe(&replay_file.meta.clientVersionFromExe),
    )
    .unwrap();
    let specs = parse_scripts(&datafiles).unwrap();
    let version_parts: Vec<_> = replay_file.meta.clientVersionFromExe.split(",").collect();
    assert!(version_parts.len() == 4);

    let mut processor = DecodedResponder {
        filename: format!(
            "{}-{}-{}-{}-{}.jl",
            replay.date,
            replay.ship,
            replay.map,
            replay.username,
            &replay.hash[0..10]
        ),
        version: wows_replays::version::Version::from_client_exe(
            &replay_file.meta.clientVersionFromExe,
        ),
        result: serde_json::to_string(&replay_file.meta).unwrap(),
    };

    // Parse packets
    let mut p = wows_replays::packet2::Parser::new(&specs);

    p.parse_packets(&replay_file.packet_data, &mut processor)
        .unwrap();
    processor
}

#[launch]
fn rocket() -> _ {
    let matches = App::new("WoWS Replay Server")
        .about("Hosts a webserver for World of Warships replay files")
        .arg(
            Arg::with_name("replays")
                .long("replays")
                .takes_value(true)
                .required(true)
                .help("Path to the replay directory"),
        )
        .arg(
            Arg::with_name("webroot")
                .long("root")
                .takes_value(true)
                .help("Prefix for the webserver path"),
        )
        .get_matches();

    let (file_sink, file_source) = std::sync::mpsc::channel();

    let replays = matches.value_of("replays").unwrap().to_owned();
    std::thread::spawn(move || {
        file_watcher(replays, file_sink);
    });

    std::thread::spawn(move || {
        replay_updater(file_source);
    });

    let config = ServerConfig {
        webroot: matches.value_of("webroot").unwrap_or("/").to_string(),
    };

    rocket::build()
        .mount(
            &config.webroot,
            routes![
                hello,
                page,
                download,
                trails,
                damage_trails,
                download_decoded
            ],
        )
        .manage(config)
}
