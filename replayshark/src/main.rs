use clap::{App, Arg, SubCommand};
use image::GenericImageView;
use image::Pixel;
use image::{imageops::FilterType, ImageFormat, RgbImage};
use plotters::prelude::*;
use std::collections::HashMap;
use std::convert::TryInto;

use wows_replays::{parse_packets, Banner, ErrorKind, Packet, PacketType, ReplayFile, ReplayMeta};

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

fn render_trails(meta: &ReplayMeta, packets: &[Packet], output: &str) {
    let trails = packets
        .iter()
        .filter_map(|packet| match &packet.payload {
            PacketType::Position(p) => Some(p),
            _ => None,
        })
        .fold(HashMap::new(), |mut acc, p| {
            if !acc.contains_key(&p.pid) {
                acc.insert(p.pid, vec![]);
            }
            acc.get_mut(&p.pid).unwrap().push((p.x as f64, p.z as f64));
            acc
        });

    let player_trail = packets
        .iter()
        .filter_map(|packet| match &packet.payload {
            PacketType::PlayerOrientation(p) => Some(p),
            _ => None,
        })
        .fold(vec![], |mut acc, p| {
            acc.push((p.x as f64, p.z as f64));
            acc
        });

    // Setup the render context
    let root = BitMapBackend::new(output, (2048, 2048)).into_drawing_area();
    root.fill(&BLACK).unwrap();

    // Blit the background into the image
    {
        let minimap = image::load(
            std::io::BufReader::new(
                std::fs::File::open(&format!("res_unpack/{}/minimap.png", meta.mapName)).unwrap(),
            ),
            ImageFormat::Png,
        )
        .unwrap();
        let minimap_background = image::load(
            std::io::BufReader::new(
                std::fs::File::open(&format!("res_unpack/{}/minimap_water.png", meta.mapName))
                    .unwrap(),
            ),
            ImageFormat::Png,
        )
        .unwrap();

        let mut image = RgbImage::new(760, 760);
        for x in 0..760 {
            for y in 0..760 {
                let bg = minimap_background.get_pixel(x, y);
                let fg = minimap.get_pixel(x, y);
                let mut bg = bg.clone();
                bg.blend(&fg);
                image.put_pixel(x, y, bg.to_rgb());
            }
        }
        let image = image::DynamicImage::ImageRgb8(image);
        let image = image.resize_exact(2048, 2048, FilterType::Lanczos3);

        let mut ctx = ChartBuilder::on(&root)
            .x_label_area_size(0)
            .y_label_area_size(0)
            .build_ranged(0.0..1.0, 0.0..1.0)
            .unwrap();

        let elem: BitMapElement<_> = ((0.0, 1.0), image).into();
        ctx.draw_series(std::iter::once(elem)).unwrap();
    }

    // Render the actual trails

    let mut map_widths: HashMap<String, u32> = HashMap::new();
    map_widths.insert("spaces/34_OC_islands".to_string(), 24);
    map_widths.insert("spaces/33_new_tierra".to_string(), 24);
    map_widths.insert("spaces/01_solomon_islands".to_string(), 30);
    map_widths.insert("spaces/10_NE_big_race".to_string(), 30);
    map_widths.insert("spaces/04_Archipelago".to_string(), 30);
    map_widths.insert("spaces/05_Ring".to_string(), 36);
    map_widths.insert("spaces/08_NE_passage".to_string(), 36);
    map_widths.insert("spaces/13_OC_new_dawn".to_string(), 36);
    map_widths.insert("spaces/17_NA_fault_line".to_string(), 42);
    map_widths.insert("spaces/41_Conquest".to_string(), 42);
    map_widths.insert("spaces/46_Estuary".to_string(), 42);
    map_widths.insert("spaces/42_Neighbors".to_string(), 42);
    map_widths.insert("spaces/50_Gold_harbor".to_string(), 42);
    map_widths.insert("spaces/20_NE_two_brothers".to_string(), 42);
    map_widths.insert("spaces/16_OC_bees_to_honey".to_string(), 48);
    map_widths.insert("spaces/22_tierra_del_fuego".to_string(), 48);
    map_widths.insert("spaces/15_NE_north".to_string(), 48);
    map_widths.insert("spaces/35_NE_north_winter".to_string(), 48);
    map_widths.insert("spaces/53_Shoreside".to_string(), 42);
    map_widths.insert("spaces/23_Shards".to_string(), 42);
    map_widths.insert("spaces/19_OC_prey".to_string(), 42);
    map_widths.insert("spaces/52_Britain".to_string(), 42);
    map_widths.insert("spaces/40_Okinawa".to_string(), 42);
    map_widths.insert("spaces/18_NE_ice_islands".to_string(), 42);
    map_widths.insert("spaces/14_Atlantic".to_string(), 42);
    map_widths.insert("spaces/38_Canada".to_string(), 48);
    map_widths.insert("spaces/37_Ridge".to_string(), 48);
    map_widths.insert("spaces/44_Path_warrior".to_string(), 48);
    map_widths.insert("spaces/25_sea_hope".to_string(), 48);
    map_widths.insert("spaces/45_Zigzag".to_string(), 48);
    map_widths.insert("spaces/47_Sleeping_Giant".to_string(), 48);
    map_widths.insert("spaces/51_Greece".to_string(), 42);
    map_widths.insert("spaces/28_naval_mission".to_string(), 42);
    map_widths.insert("spaces/00_CO_ocean".to_string(), 36);

    // 600 for New Dawn (36x36km)
    // 700 for Fault Line (42x42km)
    let scale = map_widths
        .get(&meta.mapName)
        .expect(&format!("Could not find size of map {}!", meta.mapName))
        * 50
        / 3;
    let scale = scale as f64;
    let mut scatter_ctx = ChartBuilder::on(&root)
        .x_label_area_size(0)
        .y_label_area_size(0)
        .build_ranged(-scale..scale, -scale..scale)
        .unwrap();

    let colors = [BLUE, CYAN, GREEN, MAGENTA, RED, WHITE, YELLOW];
    let mut min_x = 0.;
    let mut max_x = 0.;
    for (i, (_k, v)) in trails.iter().enumerate() {
        //println!("{}", v.len());
        let series_minx = v
            .iter()
            .map(|(x, _y)| x)
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap();
        let series_maxx = v
            .iter()
            .map(|(x, _y)| x)
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap();
        if *series_minx < min_x {
            min_x = *series_minx;
        }
        if *series_maxx > max_x {
            max_x = *series_maxx;
        }
        scatter_ctx
            .draw_series(
                v.iter()
                    .map(|(x, y)| Circle::new((*x, *y), 1, colors[i % colors.len()].filled())),
            )
            .unwrap();
    }

    // Add the trail for the player
    {
        /*let mut v = vec!();
        for idx in 0..d0.len() {
            v.push((d0[idx].1 as f64, d2[idx].1 as f64));
        }*/
        scatter_ctx
            .draw_series(
                player_trail
                    .iter()
                    .map(|(x, y)| Circle::new((*x, *y), 2, WHITE.filled())),
            )
            .unwrap();
    }
}

fn print_chatlog(packets: &[Packet]) {
    for packet in packets.iter() {
        match packet {
            Packet {
                clock,
                payload: PacketType::Chat(p),
                ..
            } => {
                println!("{}: {:?}", clock, p);
            }
            _ => {}
        }
    }
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

fn parse_replay<F: FnMut(u32, &ReplayMeta, &[Packet])>(
    replay: &std::path::PathBuf,
    mut cb: F,
) -> Result<(), wows_replays::ErrorKind> {
    let replay_file = ReplayFile::from_file(replay);

    let version_parts: Vec<_> = replay_file.meta.clientVersionFromExe.split(",").collect();
    assert!(version_parts.len() == 4);
    let build: u32 = version_parts[3].parse().unwrap();

    // Parse packets
    let packets = parse_packets(build, &replay_file.packet_data)?;

    cb(build, &replay_file.meta, &packets);

    Ok(())
}

fn parse_replay_force_version<F: FnMut(u32, &ReplayMeta, &[Packet])>(
    version: Option<u32>,
    replay: &std::path::PathBuf,
    mut cb: F,
) -> Result<(), wows_replays::ErrorKind> {
    let replay_file = ReplayFile::from_file(replay);

    let version_parts: Vec<_> = replay_file.meta.clientVersionFromExe.split(",").collect();
    assert!(version_parts.len() == 4);
    let build: u32 = version.unwrap_or(version_parts[3].parse().unwrap());

    // Parse packets
    let packets = parse_packets(build, &replay_file.packet_data)?;

    cb(build, &replay_file.meta, &packets);

    Ok(())
}

fn main() {
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
                    Arg::with_name("no-meta")
                        .long("no-meta")
                        .help("Don't output the metadata"),
                )
                .arg(replay_arg.clone()),
        )
        .get_matches();

    if let Some(matches) = matches.subcommand_matches("dump") {
        let input = matches.value_of("REPLAY").unwrap();
        parse_replay_force_version(
            if matches.is_present("no-parse-entity") {
                Some(0)
            } else {
                None
            },
            &std::path::PathBuf::from(input),
            |_, meta, packets| {
                if !matches.is_present("no-meta") {
                    println!(
                        "{}",
                        serde_json::to_string(&meta).expect("Couldn't JSON-format metadata")
                    );
                }
                for packet in packets {
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
                    let s = serde_json::to_string(&packet)
                        .expect("Couldn't JSON-format serialize packet");
                    println!("{}", s);
                }
            },
        )
        .unwrap();
    }
    if let Some(matches) = matches.subcommand_matches("summary") {
        let input = matches.value_of("REPLAY").unwrap();
        parse_replay(&std::path::PathBuf::from(input), |_, meta, packets| {
            println!("Username: {}", meta.playerName);
            println!("Date/time: {}", meta.dateTime);
            println!("Map: {}", meta.mapDisplayName);
            println!("Vehicle: {}", meta.playerVehicle);
            println!("Game mode: {} {}", meta.name, meta.gameLogic);
            println!();
            print_summary(packets);
        })
        .unwrap();
    }
    if let Some(matches) = matches.subcommand_matches("chat") {
        let input = matches.value_of("REPLAY").unwrap();
        parse_replay(&std::path::PathBuf::from(input), |_, _, packets| {
            print_chatlog(packets);
        })
        .unwrap();
    }
    if let Some(matches) = matches.subcommand_matches("trace") {
        let input = matches.value_of("REPLAY").unwrap();
        let output = matches.value_of("out").unwrap();
        parse_replay(&std::path::PathBuf::from(input), |_, meta, packets| {
            render_trails(meta, packets, output);
        })
        .unwrap();
    }
    if let Some(matches) = matches.subcommand_matches("survey") {
        let mut version_failures = 0;
        let mut other_failures = 0;
        let mut successes = 0;
        let mut total = 0;
        for replay in matches.values_of("REPLAYS").unwrap() {
            total += 1;
            match parse_replay(&std::path::PathBuf::from(replay), |_, _, packets| {
                let invalid_packets: Vec<_> = packets
                    .iter()
                    .filter_map(|packet| match &packet.payload {
                        PacketType::Invalid(p) => Some(p),
                        _ => None,
                    })
                    .collect();
                if invalid_packets.len() > 0 {
                    other_failures += 1;
                    println!(
                        "Failed to parse {} of {} packets in {}",
                        invalid_packets.len(),
                        packets.len(),
                        replay
                    );
                } else {
                    println!("Successfully parsed {}", replay);
                }
            }) {
                Ok(_) => {
                    successes += 1;
                }
                Err(ErrorKind::UnsupportedReplayVersion(n)) => {
                    version_failures += 1;
                    println!("Unsupported version {} for {}", n, replay);
                }
                Err(e) => {
                    other_failures += 1;
                    println!("Error parsing {}: {:?}", replay, e);
                }
            };
        }
        println!();
        println!("Found {} replay files", total);
        println!(
            "- {} ({:.0}%) were parsed",
            successes,
            100. * successes as f64 / total as f64
        );
        println!(
            "- {} ({:.0}%) are an unrecognized version",
            version_failures,
            100. * version_failures as f64 / total as f64
        );
        println!(
            "- {} ({:.0}%) had parse errors",
            other_failures,
            100. * other_failures as f64 / total as f64
        );
    }
}
