use clap::{App, Arg, SubCommand};
use std::collections::HashMap;
use std::io::Write;

use wows_replays::{parse_scripts, ErrorKind, ReplayFile};

mod built_info {
    // The file has been placed there by the build script.
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

struct InvestigativePrinter {
    filter_packet: Option<u32>,
    filter_method: Option<String>,
    timestamp: Option<f32>,
    entity_id: Option<u32>,
    meta: bool,
    version: wows_replays::version::Version,
}

impl wows_replays::analyzer::Analyzer for InvestigativePrinter {
    fn finish(&self) {}

    fn process(&mut self, packet: &wows_replays::packet2::Packet<'_, '_>) {
        let decoded =
            wows_replays::analyzer::decoder::DecodedPacket::from(&self.version, true, packet);

        if self.meta {
            match &decoded.payload {
                wows_replays::analyzer::decoder::DecodedPacketPayload::OnArenaStateReceived {
                    players,
                    ..
                } => {
                    for player in players.iter() {
                        println!(
                            "{} {}/{} ({:x?}/{:x?})",
                            player.name,
                            player.shipId,
                            player.avatarId,
                            (player.shipId as u32).to_le_bytes(),
                            (player.avatarId as u32).to_le_bytes()
                        );
                    }
                }
                _ => {
                    // Nop
                }
            }
        }

        if let Some(n) = self.filter_packet {
            if n != decoded.packet_type {
                return;
            }
        }
        if let Some(s) = self.filter_method.as_ref() {
            match &packet.payload {
                wows_replays::packet2::PacketType::EntityMethod(method) => {
                    if method.method != s {
                        return;
                    }
                    if let Some(eid) = self.entity_id {
                        if method.entity_id != eid {
                            return;
                        }
                    }
                }
                _ => {
                    return;
                }
            }
        }
        if let Some(t) = self.timestamp {
            let clock = (decoded.clock + t) as u32;
            let s = clock % 60;
            let clock = (clock - s) / 60;
            let m = clock % 60;
            let clock = (clock - m) / 60;
            let h = clock;
            let encoded = if self.filter_method.is_some() {
                match &packet.payload {
                    wows_replays::packet2::PacketType::EntityMethod(method) => {
                        serde_json::to_string(&method).unwrap()
                    }
                    _ => panic!(),
                }
            } else if self.filter_packet.is_some() {
                match &packet.payload {
                    wows_replays::packet2::PacketType::Unknown(x) => {
                        let v: Vec<_> = x.iter().map(|n| format!("{:02x}", n)).collect();
                        format!("0x[{}]", v.join(","))
                    }
                    _ => serde_json::to_string(&packet).unwrap(),
                }
            } else {
                serde_json::to_string(&decoded).unwrap()
            };
            println!("{:02}:{:02}:{:02}: {}", h, m, s, encoded);
        } else {
            let encoded = serde_json::to_string(&decoded).unwrap();
            println!("{}", &encoded);
        }
    }
}

pub struct InvestigativeBuilder {
    no_meta: bool,
    filter_packet: Option<String>,
    filter_method: Option<String>,
    timestamp: Option<String>,
    entity_id: Option<String>,
}

impl wows_replays::analyzer::AnalyzerBuilder for InvestigativeBuilder {
    fn build(&self, meta: &wows_replays::ReplayMeta) -> Box<dyn wows_replays::analyzer::Analyzer> {
        let version = wows_replays::version::Version::from_client_exe(&meta.clientVersionFromExe);
        let decoder = InvestigativePrinter {
            version: version,
            filter_packet: self
                .filter_packet
                .as_ref()
                .map(|s| parse_int::parse::<u32>(s).unwrap()),
            filter_method: self.filter_method.clone(),
            timestamp: self.timestamp.as_ref().map(|s| {
                let ts_parts: Vec<_> = s.split("+").collect();
                let offset = ts_parts[1].parse::<u32>().unwrap();
                let parts: Vec<_> = ts_parts[0].split(":").collect();
                if parts.len() == 3 {
                    let h = parts[0].parse::<u32>().unwrap();
                    let m = parts[1].parse::<u32>().unwrap();
                    let s = parts[2].parse::<u32>().unwrap();
                    (h * 3600 + m * 60 + s) as f32 - offset as f32
                } else {
                    panic!("Expected hh:mm:ss+offset as timestamp");
                }
            }),
            entity_id: self
                .entity_id
                .as_ref()
                .map(|s| parse_int::parse(s).unwrap()),
            meta: !self.no_meta,
        };
        if !self.no_meta {
            println!("{}", &serde_json::to_string(&meta).unwrap());
        }
        Box::new(decoder)
    }
}

fn parse_replay<P: wows_replays::analyzer::AnalyzerBuilder>(
    replay: &std::path::PathBuf,
    processor: P,
) -> Result<(), wows_replays::ErrorKind> {
    let replay_file = ReplayFile::from_file(replay)?;

    //let mut file = std::fs::File::create("foo.bin").unwrap();
    //file.write_all(&replay_file.packet_data).unwrap();

    let datafiles = wows_replays::version::Datafiles::new(
        std::path::PathBuf::from("versions"),
        wows_replays::version::Version::from_client_exe(&replay_file.meta.clientVersionFromExe),
    )?;
    let specs = parse_scripts(&datafiles)?;

    let version_parts: Vec<_> = replay_file.meta.clientVersionFromExe.split(",").collect();
    assert!(version_parts.len() == 4);

    let processor = processor.build(&replay_file.meta);

    // Parse packets
    let mut p = wows_replays::packet2::Parser::new(&specs);
    let mut analyzer_set = wows_replays::analyzer::AnalyzerAdapter::new(vec![processor]);
    match p.parse_packets::<wows_replays::analyzer::AnalyzerAdapter>(
        &replay_file.packet_data,
        &mut analyzer_set,
    ) {
        Ok(()) => {
            analyzer_set.finish();
            Ok(())
        }
        Err(e) => Err(e),
    }
}

fn truncate_string(s: &str, length: usize) -> &str {
    match s.char_indices().nth(length) {
        None => s,
        Some((idx, _)) => &s[..idx],
    }
}

fn printspecs(specs: &Vec<wows_replays::rpc::entitydefs::EntitySpec>) {
    println!("Have {} entities", specs.len());
    for entity in specs.iter() {
        println!();
        println!(
            "{} has {} properties ({} internal) and {}/{}/{} base/cell/client methods",
            entity.name,
            entity.properties.len(),
            entity.internal_properties.len(),
            entity.base_methods.len(),
            entity.cell_methods.len(),
            entity.client_methods.len()
        );

        println!("Properties:");
        for (i, property) in entity.properties.iter().enumerate() {
            println!(
                " - {}: {} flag={:?} type={:?}",
                i, property.name, property.flags, property.prop_type
            );
        }
        println!("Internal properties:");
        for (i, property) in entity.internal_properties.iter().enumerate() {
            println!(" - {}: {} type={:?}", i, property.name, property.prop_type);
        }
        println!("Client methods:");
        for (i, method) in entity.client_methods.iter().enumerate() {
            println!(" - {}: {}", i, method.name);
            for arg in method.args.iter() {
                println!("      - {:?}", arg);
            }
        }
    }
}

enum SurveyResult {
    /// npackets, ninvalid
    Success((String, String, usize, usize, Vec<String>)),
    UnsupportedVersion(String),
    ParseFailure(String),
}

struct SurveyResults {
    version_failures: usize,
    parse_failures: usize,
    successes: usize,
    successes_with_invalids: usize,
    total: usize,
    invalid_versions: HashMap<String, usize>,
    audits: HashMap<String, (String, Vec<String>)>,
}

impl SurveyResults {
    fn empty() -> Self {
        Self {
            version_failures: 0,
            parse_failures: 0,
            successes: 0,
            successes_with_invalids: 0,
            total: 0,
            invalid_versions: HashMap::new(),
            audits: HashMap::new(),
        }
    }

    fn add(&mut self, result: SurveyResult) {
        self.total += 1;
        match result {
            SurveyResult::Success((hash, datetime, _npacks, ninvalid, audits)) => {
                self.successes += 1;
                if ninvalid > 0 {
                    self.successes_with_invalids += 1;
                }
                if audits.len() > 0 {
                    self.audits.insert(hash, (datetime, audits));
                }
            }
            SurveyResult::UnsupportedVersion(version) => {
                self.version_failures += 1;
                if !self.invalid_versions.contains_key(&version) {
                    self.invalid_versions.insert(version.clone(), 0);
                }
                *self.invalid_versions.get_mut(&version).unwrap() += 1;
            }
            SurveyResult::ParseFailure(_error) => {
                self.parse_failures += 1;
            }
        }
    }

    fn print(&self) {
        let mut audits: Vec<_> = self.audits.iter().collect();
        audits.sort_by_key(|(_, (tm, _))| {
            chrono::NaiveDateTime::parse_from_str(tm, "%d.%m.%Y %H:%M:%S").unwrap()
        });
        for (k, (tm, v)) in audits.iter() {
            println!();
            println!(
                "{} ({}) has {} audits:",
                truncate_string(k, 20),
                tm,
                v.len()
            );
            let mut cnt = 0;
            for audit in v.iter() {
                if cnt >= 10 {
                    println!("...truncating");
                    break;
                }
                println!(" - {}", audit);
                cnt += 1;
            }
        }
        println!();
        println!("Found {} replay files", self.total);
        println!(
            "- {} ({:.0}%) were parsed",
            self.successes,
            100. * self.successes as f64 / self.total as f64
        );
        println!(
            "  - Of which {} ({:.0}%) contained invalid packets",
            self.successes_with_invalids,
            100. * self.successes_with_invalids as f64 / self.successes as f64
        );
        println!(
            "- {} ({:.0}%) had a parse error",
            self.parse_failures,
            100. * self.parse_failures as f64 / self.total as f64
        );
        println!(
            "- {} ({:.0}%) are an unrecognized version",
            self.version_failures,
            100. * self.version_failures as f64 / self.total as f64
        );
        if self.invalid_versions.len() > 0 {
            for (k, v) in self.invalid_versions.iter() {
                println!("  - Version {} appeared {} times", k, v);
            }
        }
    }
}

fn survey_file(skip_decode: bool, replay: std::path::PathBuf) -> SurveyResult {
    let filename = replay.file_name().unwrap().to_str().unwrap();
    let filename = filename.to_string();

    print!("Parsing {}: ", truncate_string(&filename, 20));
    std::io::stdout().flush().unwrap();

    let survey_stats = std::rc::Rc::new(std::cell::RefCell::new(
        wows_replays::analyzer::survey::SurveyStats::new(),
    ));
    let survey =
        wows_replays::analyzer::survey::SurveyBuilder::new(survey_stats.clone(), skip_decode);
    match parse_replay(&std::path::PathBuf::from(replay), survey) {
        Ok(_) => {
            let stats = survey_stats.borrow();
            if stats.invalid_packets > 0 {
                println!(
                    "OK ({} packets, {} invalid)",
                    stats.total_packets, stats.invalid_packets
                );
            } else {
                println!("OK ({} packets)", stats.total_packets);
            }
            SurveyResult::Success((
                filename.to_string(),
                stats.date_time.clone(),
                stats.total_packets,
                stats.invalid_packets,
                stats.audits.clone(),
            ))
        }
        Err(ErrorKind::DatafileNotFound { version, .. }) => {
            println!("Unsupported version {}", version.to_path());
            SurveyResult::UnsupportedVersion(version.to_path())
        }
        Err(ErrorKind::UnsupportedReplayVersion(n)) => {
            println!("Unsupported version {}", n);
            SurveyResult::UnsupportedVersion(n)
        }
        Err(e) => {
            println!("Parse error: {:?}", e);
            SurveyResult::ParseFailure(format!("{:?}", e))
        }
    }
}

fn main() {
    let replay_arg = Arg::with_name("REPLAY")
        .help("The replay file to use")
        .required(true)
        .index(1);
    let matches = App::new("World of Warships Replay Parser Utility")
        .version(built_info::GIT_VERSION.unwrap_or("undefined"))
        .author("Lane Kolbly <lane@rscheme.org>")
        .about("Parses & processes World of Warships replay files")
        .subcommand(
            SubCommand::with_name("survey")
                .about("Runs the parser against a directory of replays to validate the parser")
                .arg(
                    Arg::with_name("skip-decode")
                        .long("skip-decode")
                        .help("Don't run the decoder"),
                )
                .arg(
                    Arg::with_name("REPLAYS")
                        .help("The replay files to use")
                        .required(true)
                        .multiple(true),
                ),
        )
        .subcommand(
            SubCommand::with_name("chat")
                .about("Print the chat log of the given game")
                .arg(replay_arg.clone()),
        )
        .subcommand(
            SubCommand::with_name("summary")
                .about("Generate summary statistics of the game")
                .arg(replay_arg.clone()),
        )
        .subcommand(
            SubCommand::with_name("dump")
                .about("Dump the packets to console")
                .arg(
                    Arg::with_name("output")
                        .long("output")
                        .short("o")
                        .help("Output filename to dump to")
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("no-meta")
                        .long("no-meta")
                        .help("Don't output the metadata as first line"),
                )
                .arg(replay_arg.clone()),
        )
        .subcommand(
            SubCommand::with_name("spec")
                .about("Dump the scripts specifications to console")
                .arg(
                    Arg::with_name("version")
                        .help("Version to dump. Must be comma-delimited: major,minor,patch,build")
                        .takes_value(true)
                        .required(true),
                ),
        )
        .subcommand(
            SubCommand::with_name("search")
                .about("Search a directory full of replays")
                .arg(
                    Arg::with_name("REPLAYS")
                        .help("The replay files to use")
                        .required(true)
                        .multiple(true),
                ),
        )
        .subcommand(
            SubCommand::with_name("investigate")
                .about("Tools designed for reverse-engineering packets")
                .arg(
                    Arg::with_name("meta")
                        .long("meta")
                        .help("Don't output the metadata as first line"),
                )
                .arg(
                    Arg::with_name("timestamp")
                        .long("timestamp")
                        .takes_value(true)
                        .help("hh:mm:ss offset to render clock values with"),
                )
                .arg(
                    Arg::with_name("filter-packet")
                        .long("filter-packet")
                        .takes_value(true)
                        .help("If specified, only return packets of the given packet_type"),
                )
                .arg(
                    Arg::with_name("filter-method")
                        .long("filter-method")
                        .takes_value(true)
                        .help("If specified, only return method calls for the given method"),
                )
                .arg(
                    Arg::with_name("entity-id")
                        .long("entity-id")
                        .takes_value(true)
                        .help("Entity ID to apply to other filters if applicable"),
                )
                .arg(replay_arg.clone()),
        );

    #[cfg(feature = "graphics")]
    let matches = matches.subcommand(
        SubCommand::with_name("trace")
            .about("Renders an image showing the trails of ships over the course of the game")
            .arg(
                Arg::with_name("out")
                    .long("output")
                    .help("Output PNG file to write")
                    .takes_value(true)
                    .required(true),
            )
            .arg(replay_arg.clone()),
    );

    let matches = matches.get_matches();

    if let Some(matches) = matches.subcommand_matches("dump") {
        let input = matches.value_of("REPLAY").unwrap();
        let dump = wows_replays::analyzer::decoder::DecoderBuilder::new(
            false,
            matches.is_present("no-meta"),
            matches.value_of("output"),
        );
        parse_replay(&std::path::PathBuf::from(input), dump).unwrap();
    }
    if let Some(matches) = matches.subcommand_matches("investigate") {
        let input = matches.value_of("REPLAY").unwrap();
        let dump = InvestigativeBuilder {
            no_meta: !matches.is_present("meta"),
            filter_packet: matches.value_of("filter-packet").map(|s| s.to_string()),
            filter_method: matches.value_of("filter-method").map(|s| s.to_string()),
            entity_id: matches.value_of("entity-id").map(|s| s.to_string()),
            timestamp: matches.value_of("timestamp").map(|s| s.to_string()),
        };
        parse_replay(&std::path::PathBuf::from(input), dump).unwrap();
    }
    if let Some(matches) = matches.subcommand_matches("spec") {
        let datafiles = wows_replays::version::Datafiles::new(
            std::path::PathBuf::from("versions"),
            wows_replays::version::Version::from_client_exe(matches.value_of("version").unwrap()),
        )
        .unwrap();
        let specs = parse_scripts(&datafiles).unwrap();
        printspecs(&specs);
    }
    if let Some(matches) = matches.subcommand_matches("summary") {
        let input = matches.value_of("REPLAY").unwrap();
        let dump = wows_replays::analyzer::summary::SummaryBuilder::new();
        parse_replay(&std::path::PathBuf::from(input), dump).unwrap();
    }
    if let Some(matches) = matches.subcommand_matches("chat") {
        let input = matches.value_of("REPLAY").unwrap();
        let chatlogger = wows_replays::analyzer::chat::ChatLoggerBuilder::new();
        parse_replay(&std::path::PathBuf::from(input), chatlogger).unwrap();
    }
    #[cfg(feature = "graphics")]
    {
        if let Some(matches) = matches.subcommand_matches("trace") {
            let input = matches.value_of("REPLAY").unwrap();
            let output = matches.value_of("out").unwrap();
            let trailer = analysis::trails::TrailsBuilder::new(output);
            parse_replay(&std::path::PathBuf::from(input), trailer).unwrap();
        }
    }
    if let Some(matches) = matches.subcommand_matches("survey") {
        let mut survey_result = SurveyResults::empty();
        for replay in matches.values_of("REPLAYS").unwrap() {
            for entry in walkdir::WalkDir::new(replay) {
                let entry = entry.expect("Error unwrapping entry");
                if !entry.path().is_file() {
                    continue;
                }
                let replay = entry.path().to_path_buf();
                let result = survey_file(matches.is_present("skip-decode"), replay);
                survey_result.add(result);
            }
        }
        survey_result.print();
    }
    if let Some(matches) = matches.subcommand_matches("search") {
        let mut replays = vec![];
        for replay in matches.values_of("REPLAYS").unwrap() {
            for entry in walkdir::WalkDir::new(replay) {
                let entry = entry.expect("Error unwrapping entry");
                if !entry.path().is_file() {
                    continue;
                }
                let replay = entry.path().to_path_buf();
                let replay_path = replay.clone();

                let replay = match ReplayFile::from_file(&replay) {
                    Ok(replay) => replay,
                    Err(_) => {
                        continue;
                    }
                };
                replays.push((replay_path, replay.meta));

                if replays.len() % 100 == 0 {
                    println!("Parsed {} games...", replays.len());
                }

                //let result = survey_file(matches.is_present("skip-decode"), replay);
                //survey_result.add(result);
            }
        }
        replays.sort_by_key(|replay| {
            match chrono::NaiveDateTime::parse_from_str(&replay.1.dateTime, "%d.%m.%Y %H:%M:%S") {
                Ok(x) => x,
                Err(e) => {
                    println!("Couldn't parse '{}' because {:?}", replay.1.dateTime, e);
                    chrono::NaiveDateTime::parse_from_str(
                        "05.05.1995 01:02:03",
                        "%d.%m.%Y %H:%M:%S",
                    )
                    .unwrap()
                }
            }
            //replay.1.dateTime.clone()
        });
        println!("Found {} games", replays.len());
        for i in 0..10 {
            let idx = replays.len() - i - 1;
            println!(
                "{:?} {} {} {} {}",
                replays[idx].0,
                replays[idx].1.playerName,
                replays[idx].1.dateTime,
                replays[idx].1.mapDisplayName,
                replays[idx].1.playerVehicle
            );
        }
    }
}
