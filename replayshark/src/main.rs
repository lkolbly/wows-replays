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

/*struct TrailRenderer {
    usernames: HashMap<i32, String>,
    trails: HashMap<u32, Vec<(f32, f32)>>,
    player_trail: Vec<(f32, f32)>,
    output: String,
    meta: Option<ReplayMeta>,
}

impl wows_replays::packet2::PacketProcessor for TrailRenderer {
    fn process(&mut self, packet: wows_replays::packet2::Packet<'_, '_>) {
        match packet {
            wows_replays::packet2::Packet {
                clock,
                payload: wows_replays::packet2::PacketType::Position(pos),
                ..
            } => {
                //println!("{:?}", ec);
                if !self.trails.contains_key(&pos.pid) {
                    self.trails.insert(pos.pid, vec![]);
                }
                self.trails.get_mut(&pos.pid).unwrap().push((pos.x, pos.z));
            }
            wows_replays::packet2::Packet {
                clock,
                payload: wows_replays::packet2::PacketType::PlayerOrientation(pos),
                ..
            } => {
                self.player_trail.push((pos.x, pos.z));
            }
            _ => {}
        }
    }
}

impl MetaInjestor for TrailRenderer {
    fn meta(&mut self, meta: &ReplayMeta) {
        self.meta = Some((*meta).clone());
    }

    fn finish(&self) {
        // Setup the render context
        let root = BitMapBackend::new(&self.output, (2048, 2048)).into_drawing_area();
        root.fill(&BLACK).unwrap();

        // Blit the background into the image
        {
            let minimap = image::load(
                std::io::BufReader::new(
                    std::fs::File::open(&format!(
                        "res_unpack/{}/minimap.png",
                        self.meta.as_ref().unwrap().mapName
                    ))
                    .unwrap(),
                ),
                ImageFormat::Png,
            )
            .unwrap();
            let minimap_background = image::load(
                std::io::BufReader::new(
                    std::fs::File::open(&format!(
                        "res_unpack/{}/minimap_water.png",
                        self.meta.as_ref().unwrap().mapName
                    ))
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
            .get(&self.meta.as_ref().unwrap().mapName)
            .expect(&format!(
                "Could not find size of map {}!",
                self.meta.as_ref().unwrap().mapName
            ))
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
        for (i, (_k, v)) in self.trails.iter().enumerate() {
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
                .draw_series(v.iter().map(|(x, y)| {
                    Circle::new((*x as f64, *y as f64), 1, colors[i % colors.len()].filled())
                }))
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
                    self.player_trail
                        .iter()
                        .map(|(x, y)| Circle::new((*x as f64, *y as f64), 2, WHITE.filled())),
                )
                .unwrap();
        }
    }
}*/

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

/*struct Summarizer {
    meta: Option<ReplayMeta>,
}

impl MetaInjestor for Summarizer {
    fn meta(&mut self, meta: &ReplayMeta) {
        self.meta = Some((*meta).clone());
    }

    fn finish(&self) {
        let meta = self.meta.as_ref().unwrap();
        println!("Username: {}", meta.playerName);
        println!("Date/time: {}", meta.dateTime);
        println!("Map: {}", meta.mapDisplayName);
        println!("Vehicle: {}", meta.playerVehicle);
        println!("Game mode: {} {}", meta.name, meta.gameLogic);
        println!("Game version: {}", meta.clientVersionFromExe);
        println!();
        // TODO: Banners, damage, etc.
    }
}

impl wows_replays::packet2::PacketProcessor for Summarizer {
    fn process(&mut self, packet: wows_replays::packet2::Packet<'_, '_>) {
        // Collect banners, damage reports, etc.
    }
}*/

/*struct PacketDump {
    time_offset: f32,
}

impl MetaInjestor for PacketDump {}

impl wows_replays::packet2::PacketProcessor for PacketDump {
    fn process(&mut self, packet: wows_replays::packet2::Packet<'_, '_>) {
        let time = packet.clock + self.time_offset;
        let minutes = (time / 60.0).floor() as i32;
        let seconds = (time - minutes as f32 * 60.0).floor() as i32;
        println!("{:02}:{:02}: {:?}", minutes, seconds, packet.payload);
    }
}*/

/*struct ArtilleryShot {
    start_time: f32,
    start_pos: (f32, f32, f32),
    target: (f32, f32, f32),
}

struct DamageVector {
    start: (f32, f32),
    target: (f32, f32),
    amount: f32,
}

struct DamageMonitor {
    avatarid: u32,
    shipid: u32,
    time_offset: f32,
    artillery_shots: HashMap<i32, Vec<ArtilleryShot>>,
    position: (f32, f32, f32),
    trail: Vec<(f32, f32)>,
    meta: Option<ReplayMeta>,
    output: String,
    damages: Vec<DamageVector>,
}

impl MetaInjestor for DamageMonitor {
    fn meta(&mut self, meta: &ReplayMeta) {
        self.meta = Some((*meta).clone());
    }

    fn finish(&self) {
        // Setup the render context
        let root = BitMapBackend::new(&self.output, (2048, 2048)).into_drawing_area();
        root.fill(&BLACK).unwrap();

        // Blit the background into the image
        {
            println!("Map name = {}", self.meta.as_ref().unwrap().mapName);
            let minimap = image::load(
                std::io::BufReader::new(
                    std::fs::File::open(&format!(
                        "versions/0.10.3/{}/minimap.png",
                        self.meta.as_ref().unwrap().mapName
                    ))
                    .unwrap(),
                ),
                ImageFormat::Png,
            )
            .unwrap();
            let minimap_background = image::load(
                std::io::BufReader::new(
                    std::fs::File::open(&format!(
                        "versions/0.10.3/{}/minimap_water.png",
                        self.meta.as_ref().unwrap().mapName
                    ))
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
            .get(&self.meta.as_ref().unwrap().mapName)
            .expect(&format!(
                "Could not find size of map {}!",
                self.meta.as_ref().unwrap().mapName
            ))
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

        // Add the trail for the player
        {
            scatter_ctx
                .draw_series(
                    self.trail
                        .iter()
                        .map(|(x, y)| Circle::new((*x as f64, *y as f64), 2, WHITE.filled())),
                )
                .unwrap();
        }

        // Mark each damage vector
        for dv in self.damages.iter() {
            scatter_ctx
                .draw_series(
                    [dv.start, dv.target]
                        .iter()
                        .map(|(x, y)| Circle::new((*x as f64, *y as f64), 5, RED.filled())),
                )
                .unwrap();
            scatter_ctx
                .draw_series(vec![plotters::element::Polygon::new(
                    vec![
                        (dv.start.0 as f64, dv.start.1 as f64),
                        (dv.target.0 as f64, dv.target.1 as f64),
                    ],
                    ShapeStyle::from(&RED),
                )])
                .unwrap();
        }
    }
}

impl wows_replays::packet2::PacketProcessor for DamageMonitor {
    fn process(&mut self, packet: wows_replays::packet2::Packet<'_, '_>) {
        let time = packet.clock + self.time_offset;
        let minutes = (time / 60.0).floor() as i32;
        let seconds = (time - minutes as f32 * 60.0).floor() as i32;
        //println!("{:02}:{:02}: {:?}", minutes, seconds, packet.payload);
        let time = format!("{:02}:{:02}", minutes, seconds);

        match packet {
            wows_replays::packet2::Packet {
                clock,
                payload: wows_replays::packet2::PacketType::Position(pos),
                ..
            } => {
                //println!("{:?}", pos);
                if pos.pid == self.shipid {
                    self.position = (pos.x, pos.y, pos.z);
                    self.trail.push((pos.x, pos.z));
                }
                /*if !self.trails.contains_key(&pos.pid) {
                    self.trails.insert(pos.pid, vec![]);
                }
                self.trails.get_mut(&pos.pid).unwrap().push((pos.x, pos.z));*/
            }
            wows_replays::packet2::Packet {
                clock,
                payload: wows_replays::packet2::PacketType::PlayerOrientation(pos),
                ..
            } => {
                //println!("{:?}", pos);
                if pos.pid == self.shipid {
                    self.position = (pos.x, pos.y, pos.z);
                    self.trail.push((pos.x, pos.z));
                }
                //self.player_trail.push((pos.x, pos.z));
            }
            wows_replays::packet2::Packet {
                clock,
                payload:
                    wows_replays::packet2::PacketType::EntityMethod(
                        wows_replays::packet2::EntityMethodPacket {
                            entity_id,
                            method,
                            args,
                        },
                    ),
                ..
            } => {
                if method == "receiveDamageStat" {
                    let value = serde_pickle::de::value_from_slice(match &args[0] {
                        wows_replays::rpc::typedefs::ArgValue::Blob(x) => x,
                        _ => panic!("foo"),
                    })
                    .unwrap();
                    println!("{}: receiveDamageStat({}: {:#?})", time, entity_id, value);
                } else if method == "receiveDamageReport" {
                    let value = serde_pickle::de::value_from_slice(match &args[0] {
                        wows_replays::rpc::typedefs::ArgValue::Blob(x) => x,
                        _ => panic!("foo"),
                    })
                    .unwrap();
                    println!(
                        "{}: receiveDamageReport({}: {:#?}, {:?}, {:?})",
                        time, entity_id, value, args[1], args[2]
                    );
                } else if method == "receiveDamagesOnShip" {
                    println!("{}: receiveDamagesOnShip({}, {:?})", time, entity_id, args);
                    if entity_id != self.shipid {
                        return;
                    }
                    match &args[0] {
                        wows_replays::rpc::typedefs::ArgValue::Array(a) => {
                            for damage in a.iter() {
                                let damage = match damage {
                                    wows_replays::rpc::typedefs::ArgValue::FixedDict(m) => m,
                                    _ => panic!("foo"),
                                };
                                let aggressor = match damage.get("vehicleID").unwrap() {
                                    wows_replays::rpc::typedefs::ArgValue::Int32(i) => *i,
                                    _ => panic!("foo"),
                                };
                                let amount = match damage.get("damage").unwrap() {
                                    wows_replays::rpc::typedefs::ArgValue::Float32(f) => *f,
                                    _ => panic!("foo"),
                                };

                                // Go find the most recent shot fired by the aggressor at us
                                // Note that this won't technically be correct if the aggressor
                                // has multiple shots in the air at a given time
                                println!("agressor={}", aggressor);
                                if let Some(shots) = self.artillery_shots.get(&aggressor) {
                                    let tolerance = 200.0;
                                    for i in 1..shots.len() + 1 {
                                        let shot = &shots[shots.len() - i];
                                        let dx = shot.target.0 - self.position.0;
                                        let dz = shot.target.2 - self.position.2;
                                        let dist = (dx * dx + dz * dz).sqrt();
                                        if dist < tolerance {
                                            // Found it!
                                            println!(
                                                "{:?} was shot from {:?} causing {} damage",
                                                self.position, shot.start_pos, amount
                                            );
                                            self.damages.push(DamageVector {
                                                start: (shot.start_pos.0, shot.start_pos.2),
                                                target: (self.position.0, self.position.2),
                                                amount: amount,
                                            });
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        _ => panic!("foo"),
                    }
                } else if method == "receiveArtilleryShots" {
                    println!("{}: receiveArtilleryShots({:?})", time, args);
                    match &args[0] {
                        wows_replays::rpc::typedefs::ArgValue::Array(a) => {
                            for salvo in a.iter() {
                                //println!("Salvo: {:?}", salvo);
                                let salvo = match salvo {
                                    wows_replays::rpc::typedefs::ArgValue::FixedDict(m) => m,
                                    _ => panic!("foo"),
                                };
                                let owner_id = match salvo.get("ownerID").unwrap() {
                                    wows_replays::rpc::typedefs::ArgValue::Int32(i) => *i,
                                    _ => panic!("foo"),
                                };
                                for shot in match salvo.get("shots").unwrap() {
                                    wows_replays::rpc::typedefs::ArgValue::Array(a) => a,
                                    _ => panic!("foo"),
                                } {
                                    //println!("Shot: {:?}", shot);
                                    let shot = match shot {
                                        wows_replays::rpc::typedefs::ArgValue::FixedDict(m) => m,
                                        _ => panic!("foo"),
                                    };
                                    if !self.artillery_shots.contains_key(&owner_id) {
                                        self.artillery_shots.insert(owner_id, vec![]);
                                    }
                                    self.artillery_shots.get_mut(&owner_id).unwrap().push(
                                        ArtilleryShot {
                                            start_time: clock,
                                            start_pos: match shot.get("pos").unwrap() {
                                                wows_replays::rpc::typedefs::ArgValue::Vector3(
                                                    v,
                                                ) => v.clone(),
                                                _ => panic!("foo"),
                                            },
                                            target: match shot.get("tarPos").unwrap() {
                                                wows_replays::rpc::typedefs::ArgValue::Vector3(
                                                    v,
                                                ) => v.clone(),
                                                _ => panic!("foo"),
                                            },
                                        },
                                    );
                                }
                            }
                        }
                        _ => panic!("foo"),
                    }
                } else if method == "receiveHitLocationsInitialState" {
                    println!(
                        "{}: receiveHitLocationsInitialState({}, {:?})",
                        time, entity_id, args
                    );
                }
            }
            _ => {}
        }
    }
}*/

/*struct ChatLogger {
    usernames: HashMap<i32, String>,
}

impl MetaInjestor for ChatLogger {}

impl wows_replays::packet2::PacketProcessor for ChatLogger {
    fn process(&mut self, packet: wows_replays::packet2::Packet<'_, '_>) {
        match packet {
            wows_replays::packet2::Packet {
                clock,
                payload: wows_replays::packet2::PacketType::EntityCreate(ec),
                ..
            } => {
                //println!("{:?}", ec);
            }
            wows_replays::packet2::Packet {
                clock,
                payload:
                    wows_replays::packet2::PacketType::EntityMethod(
                        wows_replays::packet2::EntityMethodPacket {
                            entity_id,
                            method,
                            args,
                        },
                    ),
                ..
            } => {
                if method == "onChatMessage" {
                    let target = match &args[1] {
                        wows_replays::rpc::typedefs::ArgValue::String(s) => s,
                        _ => panic!("foo"),
                    };
                    let message = match &args[2] {
                        wows_replays::rpc::typedefs::ArgValue::String(s) => s,
                        _ => panic!("foo"),
                    };
                    let sender_id = match &args[0] {
                        wows_replays::rpc::typedefs::ArgValue::Int32(i) => i,
                        _ => panic!("foo"),
                    };
                    //println!("{:?}", parsed_call.args);
                    println!(
                        "{}: {}: {} {}",
                        clock,
                        self.usernames.get(sender_id).unwrap(),
                        std::str::from_utf8(&target).unwrap(),
                        std::str::from_utf8(&message).unwrap()
                    );
                } else if method == "receieve_CommonCMD" {
                    // Voiceline
                } else if method == "onArenaStateReceived" {
                    let value = serde_pickle::de::value_from_slice(match &args[3] {
                        wows_replays::rpc::typedefs::ArgValue::Blob(x) => x,
                        _ => panic!("foo"),
                    })
                    .unwrap();
                    //println!("{:#?}", value);
                    if let serde_pickle::value::Value::List(players) = &value {
                        for player in players.iter() {
                            let mut values = HashMap::new();
                            if let serde_pickle::value::Value::List(elements) = player {
                                for elem in elements.iter() {
                                    if let serde_pickle::value::Value::Tuple(kv) = elem {
                                        //println!("{:?}", kv);
                                        let key = match kv[0] {
                                            serde_pickle::value::Value::I64(key) => key,
                                            _ => panic!(),
                                        };
                                        values.insert(key, kv[1].clone());
                                    }
                                }
                            }
                            //println!("{:#?}", values);
                            let avatar = values.get(&0x1).unwrap();
                            let username = values.get(&0x16).unwrap();
                            let username = std::str::from_utf8(match username {
                                serde_pickle::value::Value::Bytes(u) => u,
                                _ => panic!(),
                            })
                            .unwrap();
                            let shipid = values.get(&0x1d).unwrap();
                            let playerid = values.get(&0x1e).unwrap();
                            let playeravatarid = values.get(&0x1f).unwrap();
                            println!(
                                "{}: {}/{}/{}/{}",
                                username, avatar, shipid, playerid, playeravatarid
                            );
                            self.usernames.insert(
                                match avatar {
                                    serde_pickle::value::Value::I64(i) => *i as i32,
                                    _ => panic!(),
                                },
                                username.to_string(),
                            );
                        }
                        println!("found {} players", players.len());
                    }
                    //panic!();
                }
            }
            _ => {}
        }
    }
}*/

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
