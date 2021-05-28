use clap::{App, Arg, SubCommand};
use image::GenericImageView;
use image::Pixel;
use image::{imageops::FilterType, ImageFormat, RgbImage};
use plotters::prelude::*;
use std::collections::HashMap;
use std::convert::TryInto;

use wows_replays::{
    parse_packets, parse_scripts, Banner, ErrorKind, Packet, PacketType, ReplayFile, ReplayMeta,
};

fn extract_banners(packets: &[Packet]) -> HashMap<Banner, usize> {
    packets
        .iter()
        .filter_map(|packet| match packet.payload {
            PacketType::Banner(p) => Some(p),
            _ => None,
        })
        .fold(HashMap::new(), |mut acc, banner| {
            if !acc.contains_key(&banner) {
                acc.insert(banner, 0);
            }
            *acc.get_mut(&banner).unwrap() += 1;
            acc
        })
}

struct Survey {
    filename: String,
    meta: Option<ReplayMeta>,
}

impl MetaInjestor for Survey {
    fn meta(&mut self, meta: &ReplayMeta) {
        self.meta = Some((*meta).clone());
    }

    fn finish(&self) {
        let meta = self.meta.as_ref().unwrap();
        if meta.playerName == "lkolbly" && meta.clientVersionFromExe == "0,10,3,3747819" {
            println!("{}", self.filename);
            println!("Username: {}", meta.playerName);
            println!("Date/time: {}", meta.dateTime);
            println!("Map: {}", meta.mapDisplayName);
            println!("Vehicle: {}", meta.playerVehicle);
            println!("Game mode: {} {}", meta.name, meta.gameLogic);
            println!("Game version: {}", meta.clientVersionFromExe);
            println!();
        }
    }
}

impl wows_replays::packet2::PacketProcessor for Survey {
    fn process(&mut self, packet: wows_replays::packet2::Packet<'_, '_>) {}
}

fn print_summary(packets: &[Packet]) {
    let banners = extract_banners(packets);
    for (k, v) in banners.iter() {
        println!("Banner {:?}: {}x", k, v);
    }

    let damage_dealt = packets
        .iter()
        .filter_map(|packet| match &packet.payload {
            PacketType::ArtilleryHit(p) => {
                if !p.is_incoming {
                    Some(p.damage)
                } else {
                    None
                }
            }
            _ => None,
        })
        .fold(0, |acc, x| acc + x);
    println!("Player dealt {} damage", damage_dealt);
}

// From https://stackoverflow.com/questions/35901547/how-can-i-find-a-subsequence-in-a-u8-slice
fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn find_float_approx(haystack: &[u8], needle: f32, epsilon: f32) -> Option<usize> {
    haystack.windows(4).position(|window| {
        let x = f32::from_le_bytes(window.try_into().unwrap());
        (x.abs() - needle).abs() <= epsilon
    })
}

trait MetaInjestor {
    fn meta(&mut self, meta: &ReplayMeta) {}
    fn finish(&self) {}
}

//fn parse_replay<P: wows_replays::packet2::PacketProcessor + MetaInjestor>(
fn parse_replay<P: wows_replays::analyzer::AnalyzerBuilder>(
    replay: &std::path::PathBuf,
    mut processor: P,
) -> Result<(), wows_replays::ErrorKind> {
    let replay_file = ReplayFile::from_file(replay)?;

    let datafiles = wows_replays::version::Datafiles::new(
        std::path::PathBuf::from("versions"),
        wows_replays::version::Version::from_client_exe(&replay_file.meta.clientVersionFromExe),
    );
    let specs = parse_scripts(&datafiles);

    let version_parts: Vec<_> = replay_file.meta.clientVersionFromExe.split(",").collect();
    assert!(version_parts.len() == 4);
    let build: u32 = version_parts[3].parse().unwrap();

    if replay_file.meta.clientVersionFromExe != "0,10,3,3747819" {
        // TODO: Return invalid version error
        return Ok(());
    }

    let mut processor = processor.build(&replay_file.meta);
    //processor.meta(&replay_file.meta);

    // Parse packets
    let mut p = wows_replays::packet2::Parser::new(specs);
    let mut analyzer_set = wows_replays::analyzer::AnalyzerAdapter::new(vec![processor]);
    match p.parse_packets::<wows_replays::analyzer::AnalyzerAdapter>(
        &replay_file.packet_data,
        &mut analyzer_set,
    ) {
        Ok(packets) => {
            //processor.finish();
            analyzer_set.finish();
            Ok(())
        }
        Err(e) => Err(e),
    }
}

fn parse_replay_force_version<F: FnMut(u32, &ReplayMeta, &[Packet])>(
    version: Option<u32>,
    replay: &std::path::PathBuf,
    mut cb: F,
) -> Result<(), wows_replays::ErrorKind> {
    let replay_file = ReplayFile::from_file(replay)?;

    let version_parts: Vec<_> = replay_file.meta.clientVersionFromExe.split(",").collect();
    assert!(version_parts.len() == 4);
    let build: u32 = version.unwrap_or(version_parts[3].parse().unwrap());

    // Parse packets
    let packets = parse_packets(build, &replay_file.packet_data)?;

    cb(build, &replay_file.meta, &packets);

    Ok(())
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
            "{} has {} properties and {}/{}/{} base/cell/client methods",
            entity.name,
            entity.properties.len(),
            entity.base_methods.len(),
            entity.cell_methods.len(),
            entity.client_methods.len()
        );

        println!("Properties:");
        for (i, property) in entity.properties.iter().enumerate() {
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

fn main() {
    /*let specs = parse_scripts(std::path::PathBuf::from("versions/0.10.3/scripts"));
    printspecs(&specs);
    return;*/

    /*let replay_file = ReplayFile::from_file(&std::path::PathBuf::from(
        "test/replays/version-3747819.wowsreplay",
    ))
    .unwrap();

    let version_parts: Vec<_> = replay_file.meta.clientVersionFromExe.split(",").collect();
    assert!(version_parts.len() == 4);
    let build: u32 = version_parts[3].parse().unwrap();

    // Parse packets
    let mut p = wows_replays::packet2::Parser::new(specs);
    match p.parse_packets(&replay_file.packet_data) {
        Ok(packets) => {
            //cb(build, &replay_file.meta, &packets);
            for packet in packets.iter() {
                println!("{:?}", packet);
            }
            println!("Parsed {} packets", packets.len());
        }
        Err(e) => {
            println!("Got error parsing!");
        }
    }
    return;*/

    let replay_arg = Arg::with_name("REPLAY")
        .help("The replay file to use")
        .required(true)
        .index(1);
    let matches = App::new("World of Warships Replay Parser Utility")
        .version("0.1.0")
        .author("Lane Kolbly <lane@rscheme.org>")
        .about("Parses & processes World of Warships replay files")
        .subcommand(
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
        )
        .subcommand(
            SubCommand::with_name("survey")
                .about("Runs the parser against a directory of replays to validate the parser")
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
                    Arg::with_name("no-parse-entity")
                        .long("no-parse-entity")
                        .help("Parse all entity packets as unknown"),
                )
                .arg(
                    Arg::with_name("filter-super")
                        .long("filter-super")
                        .help("Filter packets to be the given entity supertype")
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("filter-sub")
                        .long("filter-sub")
                        .help("Filter packets to be the given entity subtype")
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("exclude-subtypes")
                        .long("exclude-subtypes")
                        .help("A comma-delimited list of Entity subtypes to exclude")
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("timestamps")
                        .long("timestamps")
                        .help("A comma-delimited list of timestamps to highlight in the output")
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("timestamp-offset")
                        .long("timestamp-offset")
                        .help("Number of seconds to subtract from the timestamps")
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("no-meta")
                        .long("no-meta")
                        .help("Don't output the metadata"),
                )
                .arg(
                    Arg::with_name("speed")
                        .long("speed")
                        .help("Play back the file at the given speed multiplier")
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("xxd")
                        .long("xxd")
                        .help("Print out the packets as xxd-formatted binary dumps"),
                )
                .arg(replay_arg.clone()),
        )
        .get_matches();

    if let Some(matches) = matches.subcommand_matches("dump") {
        let input = matches.value_of("REPLAY").unwrap();
        /*let mut dump = PacketDump {
            time_offset: 2431.0,
        };
        parse_replay(&std::path::PathBuf::from(input), dump).unwrap();*/
        /*let mut dump = DamageMonitor {
            avatarid: 511279, //avatarid: 576297,
            shipid: 511280,   //shipid: 576298,
            time_offset: 5824.0,
            artillery_shots: HashMap::new(),
            position: (1e9, 1e9, 1e9),
            trail: vec![],
            meta: None,
            output: "foo.png".to_string(),
            damages: vec![],
        };*/
        let mut dump = wows_replays::analyzer::packet_dump::PacketDumpBuilder::new(2431.0);
        //let mut dump = wows_replays::analyzer::damage_trails::DamageTrailsBuilder::new();
        parse_replay(&std::path::PathBuf::from(input), dump).unwrap();
        return;
        parse_replay_force_version(
            if matches.is_present("no-parse-entity") {
                Some(0)
            } else {
                None
            },
            &std::path::PathBuf::from(input),
            |_, meta, packets| {
                let timestamp_offset: u32 = matches
                    .value_of("timestamp-offset")
                    .unwrap_or("0")
                    .parse()
                    .expect("Couldn't parse timestamp-offset as float");

                let mut timestamps: Vec<u32> = matches
                    .value_of("timestamps")
                    .unwrap_or("")
                    .split(',')
                    .map(|x| {
                        if x.len() == 0 {
                            // TODO: This is a workaround for empty
                            return 0;
                        }
                        let parts: Vec<&str> = x.split(':').collect();
                        assert!(parts.len() == 2);
                        let m: u32 = parts[0].parse().unwrap();
                        let s: u32 = parts[1].parse().unwrap();
                        (m * 60 + s) - timestamp_offset
                    })
                    .collect();
                timestamps.sort();
                if timestamps.len() == 1 && timestamps[0] == 0 {
                    // TODO: Workaround for empty timestamps specifier
                    timestamps = vec![];
                }
                //println!("{:?}", timestamps);

                #[derive(PartialEq)]
                enum PacketIdent {
                    PacketType(u32),
                    WithSubtype((u32, u32)),
                };

                let mut exclude_packets: Vec<PacketIdent> = matches
                    .value_of("exclude-subtypes")
                    .unwrap_or("")
                    .split(',')
                    .map(|x| {
                        if x.len() == 0 {
                            // TODO: This is a workaround for empty lists
                            return PacketIdent::PacketType(0);
                        }
                        if x.contains(":") {
                            let parts: Vec<&str> = x.split(':').collect();
                            assert!(parts.len() == 2);
                            let supertype = parts[0].parse().expect("Couldn't parse supertype");
                            let subtype = parts[1].parse().expect("Couldn't parse subtype");
                            PacketIdent::WithSubtype((supertype, subtype))
                        } else {
                            PacketIdent::PacketType(x.parse().expect("Couldn't parse u32"))
                        }
                        //x.parse().expect("Couldn't parse exclude packet")
                    })
                    .collect();
                if exclude_packets.len() == 0 && exclude_packets[0] == PacketIdent::PacketType(0) {
                    // TODO: This is a workaround for empty lists
                    exclude_packets = vec![];
                }

                if !matches.is_present("no-meta") {
                    println!(
                        "{}",
                        serde_json::to_string(&meta).expect("Couldn't JSON-format metadata")
                    );
                }
                let speed: u32 = matches
                    .value_of("speed")
                    .map(|x| {
                        x.parse()
                            .expect("Couldn't parse speed! Must specify an integer")
                    })
                    .unwrap_or(0);
                let start_tm = std::time::Instant::now();
                for packet in packets {
                    if timestamps.len() > 0 && timestamps[0] < packet.clock as u32 {
                        println!("{{\"clock\":{},\"timestamp\":1}}", packet.clock);
                        timestamps.remove(0);
                    }
                    let superfilter: Option<u32> =
                        matches.value_of("filter-super").map(|x| x.parse().unwrap());
                    let subfilter: Option<u32> =
                        matches.value_of("filter-sub").map(|x| x.parse().unwrap());
                    match &packet.payload {
                        PacketType::Entity(p) => {
                            if let Some(sup) = superfilter {
                                if p.supertype != sup {
                                    continue;
                                }
                            }
                            if let Some(sub) = subfilter {
                                if p.subtype != sub {
                                    continue;
                                }
                            }
                        }
                        _ => match (superfilter, subfilter) {
                            (None, None) => {}
                            _ => {
                                continue;
                            }
                        },
                    };
                    /*if exclude_packets.len() > 0 {
                        let packet_type = if packet.packet_type == 7 || packet.packet_type == 8 {
                            match &packet.payload {
                                PacketType::Entity(p) => {
                                    PacketIdent::WithSubtype((p.supertype, p.subtype))
                                }
                                _ => {
                                    // Skip known packets
                                    continue;
                                }
                            }
                        } else {
                            PacketIdent::PacketType(packet.packet_type)
                        };
                        if exclude_packets.contains(&packet_type) {
                            continue;
                        }
                    }*/
                    if speed > 0 {
                        let current_tm = start_tm.elapsed().as_secs_f32() * speed as f32;
                        if packet.clock > current_tm {
                            let millis = (packet.clock - current_tm) * 1000.0;
                            //println!("Sleeping for {}", millis);
                            std::thread::sleep(std::time::Duration::from_millis(millis as u64));
                        }
                    }
                    if matches.is_present("xxd") {
                        println!("clock={} type=0x{:x}", packet.clock, packet.packet_type);
                        hexdump::hexdump(packet.raw);
                        match &packet.payload {
                            PacketType::Unknown(_) => {
                                // Wasn't parsed, don't print the serialization
                            }
                            payload => {
                                println!("Deserialized as:");
                                println!("{:?}", payload);
                            }
                        }
                        println!();
                    } else {
                        let s = serde_json::to_string(&packet)
                            .expect("Couldn't JSON-format serialize packet");
                        println!("{}", s);
                    }
                }
            },
        )
        .unwrap();
    }
    if let Some(matches) = matches.subcommand_matches("summary") {
        let input = matches.value_of("REPLAY").unwrap();
        /*parse_replay(&std::path::PathBuf::from(input), |_, meta, packets| {
            println!("Username: {}", meta.playerName);
            println!("Date/time: {}", meta.dateTime);
            println!("Map: {}", meta.mapDisplayName);
            println!("Vehicle: {}", meta.playerVehicle);
            println!("Game mode: {} {}", meta.name, meta.gameLogic);
            println!("Game version: {}", meta.clientVersionFromExe);
            println!();
            // TODO: Update to packet2
            //print_summary(packets);
        })
        .unwrap();*/
        //let mut dump = Summarizer { meta: None };
        let mut dump = wows_replays::analyzer::summary::SummaryBuilder::new();
        parse_replay(&std::path::PathBuf::from(input), dump).unwrap();
    }
    if let Some(matches) = matches.subcommand_matches("chat") {
        let input = matches.value_of("REPLAY").unwrap();
        /*parse_replay(&std::path::PathBuf::from(input), |_, _, packets| {
            print_chatlog(packets);
        })
        .unwrap();*/
        let mut chatlogger = wows_replays::analyzer::chat::ChatLoggerBuilder::new();
        parse_replay(&std::path::PathBuf::from(input), chatlogger).unwrap();
    }
    if let Some(matches) = matches.subcommand_matches("trace") {
        let input = matches.value_of("REPLAY").unwrap();
        let output = matches.value_of("out").unwrap();
        /*parse_replay(&std::path::PathBuf::from(input), |_, meta, packets| {
            // TODO: Update to packet2
            //render_trails(meta, packets, output);
        })
        .unwrap();*/
        /*let mut trailer = TrailRenderer {
            usernames: HashMap::new(),
            player_trail: vec![],
            trails: HashMap::new(),
            output: output.to_string(),
            meta: None,
        };*/
        let mut trailer = wows_replays::analyzer::trails::TrailsBuilder::new(output);
        parse_replay(&std::path::PathBuf::from(input), trailer).unwrap();
    }
    if let Some(matches) = matches.subcommand_matches("survey") {
        let mut version_failures = 0;
        let mut other_failures = 0;
        let mut successes = 0;
        let mut total = 0;
        //let mut invalid_versions = HashMap::new();
        for replay in matches.values_of("REPLAYS").unwrap() {
            for entry in walkdir::WalkDir::new(replay) {
                let entry = entry.expect("Error unwrapping entry");
                if !entry.path().is_file() {
                    continue;
                }
                let replay = entry.path().to_path_buf();
                let filename = replay.file_name().unwrap().to_str().unwrap();
                if filename.contains("8654fea76d1a758ea40d") {
                    // This one fails to parse the initial bit
                    continue;
                }
                if filename.contains("537e4d5f3b01e17ac02d")
                    || filename.contains("6a07f3222eca0cf9a585")
                    || filename.contains("82f2cf97f44dc188bf3b")
                    || filename.contains("ac054684b5450f908f1f")
                {
                    // These fail due to unknown death cause 10
                    continue;
                }
                if filename.contains("a71c42aabe17848bf618")
                    || filename.contains("cb5b3f96018265ef8dbb")
                {
                    // Ship ID was not a U32
                    continue;
                }
                /*if filename.contains("03f2f7372aff4b0e8c0e")
                    || filename.contains("0567dab0a0d21ebb42b7")
                    || filename.contains("0f053ddd1c3d3db4fa47")
                {
                    // Some serde issue
                    continue;
                }*/
                //println!("Parsing {}: ", truncate_string(filename, 20));
                total += 1;
                let mut dump = Survey {
                    meta: None,
                    filename: filename.to_string(),
                };
                //parse_replay(&std::path::PathBuf::from(replay), dump);
                /*match parse_replay(&replay, |_, _, packets| {
                    // TODO: Update to packet2
                    /*let invalid_packets: Vec<_> = packets
                        .iter()
                        .filter_map(|packet| match &packet.payload {
                            PacketType::Invalid(p) => Some(p),
                            _ => None,
                        })
                        .collect();
                    if invalid_packets.len() > 0 {
                        other_failures += 1;
                        println!(
                            "Failed to parse {} of {} packets",
                            invalid_packets.len(),
                            packets.len(),
                        );
                    } else {
                        println!("Successful!");
                    }*/
                }) {
                    Ok(_) => {
                        successes += 1;
                    }
                    Err(ErrorKind::UnsupportedReplayVersion(n)) => {
                        version_failures += 1;
                        if !invalid_versions.contains_key(&n) {
                            invalid_versions.insert(n, 0);
                        }
                        *invalid_versions.get_mut(&n).unwrap() += 1;
                        println!("Unsupported version {}", n,);
                    }
                    Err(e) => {
                        other_failures += 1;
                        println!("Parse error: {:?}", e);
                    }
                };*/
            }
        }
        println!();
        println!("Found {} replay files", total);
        println!(
            "- {} ({:.0}%) were parsed",
            successes,
            100. * successes as f64 / total as f64
        );
        println!(
            "  - {} ({:.0}%) had parse errors",
            other_failures,
            100. * other_failures as f64 / successes as f64
        );
        println!(
            "- {} ({:.0}%) are an unrecognized version",
            version_failures,
            100. * version_failures as f64 / total as f64
        );
        /*if invalid_versions.len() > 0 {
            for (k, v) in invalid_versions.iter() {
                println!("  - Version {} appeared {} times", k, v);
            }
        }*/
    }
}
