use std::collections::HashMap;

use wows_replays::{parse_packets, Packet, PacketType, ReplayFile};

fn parse_replay(replay: &std::path::PathBuf) {
    let replay_file = ReplayFile::from_file(replay);

    let version_parts: Vec<_> = replay_file.meta.clientVersionFromExe.split(",").collect();
    assert!(version_parts.len() == 4);
    let build: u32 = version_parts[3].parse().unwrap();
    println!("File build version: {}", build);

    // Parse packets
    let (_, packets) = parse_packets(build, &replay_file.packet_data).unwrap();

    let mut total_damage = 0;
    let mut banners = HashMap::new();
    for packet in packets.iter() {
        match packet {
            Packet {
                payload: PacketType::ArtilleryHit(p),
                ..
            } => {
                if !p.is_incoming {
                    total_damage += p.damage;
                }
            }
            Packet {
                payload: PacketType::Banner(p),
                ..
            } => {
                if !banners.contains_key(&p) {
                    banners.insert(p, 0);
                }
                *banners.get_mut(&p).unwrap() += 1;
            }
            _ => {}
        }
    }
    println!("Player did {} damage!", total_damage);

    for (k, v) in banners.iter() {
        println!("Banner {:?}: {}x", k, v);
    }
}

fn main() {
    parse_replay(&std::path::PathBuf::from(
        "replays/20200605_183626_PASB008-Colorado-1945_13_OC_new_dawn.wowsreplay",
    ));
}
