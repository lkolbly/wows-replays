use crate::analyzer::decoder::{DecodedPacket, DecodedPacketPayload};
use crate::analyzer::*;
use crate::packet2::{EntityMethodPacket, Packet, PacketType};
use crate::version::Version;
use crate::ReplayMeta;
use image::GenericImageView;
use image::Pixel;
use image::{imageops::FilterType, ImageFormat, RgbImage};
use plotters::prelude::*;
use std::collections::HashMap;

pub struct DamageTrailsBuilder {
    output: String,
}

impl DamageTrailsBuilder {
    pub fn new(output: &str) -> Self {
        Self {
            output: output.to_string(),
        }
    }
}

impl AnalyzerBuilder for DamageTrailsBuilder {
    fn build(&self, meta: &ReplayMeta) -> Box<dyn Analyzer> {
        Box::new(DamageMonitor {
            version: Version::from_client_exe(&meta.clientVersionFromExe),
            username: meta.playerName.clone(),
            avatarid: None,
            shipid: None,
            artillery_shots: HashMap::new(),
            position: (1e9, 1e9, 1e9),
            trail: vec![],
            meta: Some((*meta).clone()),
            output: self.output.clone(),
            damages: vec![],
        })
    }
}

struct ArtilleryShot {
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
    version: Version,
    username: String,
    avatarid: Option<u32>,
    shipid: Option<u32>,
    artillery_shots: HashMap<i32, Vec<ArtilleryShot>>,
    position: (f32, f32, f32),
    trail: Vec<(f32, f32)>,
    meta: Option<ReplayMeta>,
    output: String,
    damages: Vec<DamageVector>,
}

impl Analyzer for DamageMonitor {
    fn finish(&self) {
        let start = std::time::Instant::now();

        // Setup the render context
        let root = BitMapBackend::new(&self.output, (2048, 2048)).into_drawing_area();
        root.fill(&BLACK).unwrap();

        println!("Black fill time = {:?}", start.elapsed());
        let start = std::time::Instant::now();

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

            println!("Minimap load time = {:?}", start.elapsed());
            let start = std::time::Instant::now();

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

            println!("Minimap 760px fill time = {:?}", start.elapsed());
            let start = std::time::Instant::now();

            let image = image::DynamicImage::ImageRgb8(image);
            let image = image.resize_exact(2048, 2048, FilterType::Lanczos3);

            let mut ctx = ChartBuilder::on(&root)
                .x_label_area_size(0)
                .y_label_area_size(0)
                .build_cartesian_2d(0.0..1.0, 0.0..1.0)
                .unwrap();

            let elem: BitMapElement<_> = ((0.0, 1.0), image).into();
            ctx.draw_series(std::iter::once(elem)).unwrap();

            println!("Resize time = {:?}", start.elapsed());
        }

        let start = std::time::Instant::now();

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
            .build_cartesian_2d(-scale..scale, -scale..scale)
            .unwrap();

        // Add the trail for the player
        scatter_ctx
            .draw_series(
                self.trail
                    .iter()
                    .map(|(x, y)| Circle::new((*x as f64, *y as f64), 2, WHITE.filled())),
            )
            .unwrap();

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
        println!("Trail render time = {:?}", start.elapsed());
    }

    fn process(&mut self, packet: &Packet<'_, '_>) {
        let time = packet.clock;
        let minutes = (time / 60.0).floor() as i32;
        let seconds = (time - minutes as f32 * 60.0).floor() as i32;
        let time = format!("{:02}:{:02}", minutes, seconds);

        let decoded = DecodedPacket::from(&self.version, packet);
        match &decoded.payload {
            DecodedPacketPayload::OnArenaStateReceived { players, .. } => {
                for player in players.iter() {
                    if player.username == self.username {
                        self.shipid = Some(player.shipid as u32);
                        self.avatarid = Some(player.avatarid as u32);
                        break;
                    }
                }
            }
            _ => {}
        }

        match &packet.payload {
            PacketType::Position(pos) => {
                if pos.pid == self.shipid.unwrap_or(0) {
                    self.position = (pos.x, pos.y, pos.z);
                    self.trail.push((pos.x, pos.z));
                }
            }
            PacketType::PlayerOrientation(pos) => {
                if pos.pid == self.shipid.unwrap_or(0) {
                    self.position = (pos.x, pos.y, pos.z);
                    self.trail.push((pos.x, pos.z));
                }
            }
            PacketType::EntityMethod(EntityMethodPacket {
                entity_id,
                method,
                args,
            }) => {
                if *method == "receiveDamageStat" {
                    let value = serde_pickle::de::value_from_slice(
                        match &args[0] {
                            crate::rpc::typedefs::ArgValue::Blob(x) => x,
                            _ => panic!("foo"),
                        },
                        serde_pickle::de::DeOptions::new(),
                    )
                    .unwrap();
                    println!("{}: receiveDamageStat({}: {:#?})", time, entity_id, value);
                } else if *method == "receiveDamageReport" {
                    let value = serde_pickle::de::value_from_slice(
                        match &args[0] {
                            crate::rpc::typedefs::ArgValue::Blob(x) => x,
                            _ => panic!("foo"),
                        },
                        serde_pickle::de::DeOptions::new(),
                    )
                    .unwrap();
                    println!(
                        "{}: receiveDamageReport({}: {:#?}, {:?}, {:?})",
                        time, entity_id, value, args[1], args[2]
                    );
                } else if *method == "receiveDamagesOnShip" {
                    println!("{}: receiveDamagesOnShip({}, {:?})", time, entity_id, args);
                    if *entity_id != self.shipid.unwrap_or(0) {
                        return;
                    }
                    match &args[0] {
                        crate::rpc::typedefs::ArgValue::Array(a) => {
                            for damage in a.iter() {
                                let damage = match damage {
                                    crate::rpc::typedefs::ArgValue::FixedDict(m) => m,
                                    _ => panic!("foo"),
                                };
                                let aggressor = match damage.get("vehicleID").unwrap() {
                                    crate::rpc::typedefs::ArgValue::Int32(i) => *i,
                                    _ => panic!("foo"),
                                };
                                let amount = match damage.get("damage").unwrap() {
                                    crate::rpc::typedefs::ArgValue::Float32(f) => *f,
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
                } else if *method == "receiveArtilleryShots" {
                    println!("{}: receiveArtilleryShots({:?})", time, args);
                    match &args[0] {
                        crate::rpc::typedefs::ArgValue::Array(a) => {
                            for salvo in a.iter() {
                                //println!("Salvo: {:?}", salvo);
                                let salvo = match salvo {
                                    crate::rpc::typedefs::ArgValue::FixedDict(m) => m,
                                    _ => panic!("foo"),
                                };
                                let owner_id = match salvo.get("ownerID").unwrap() {
                                    crate::rpc::typedefs::ArgValue::Int32(i) => *i,
                                    _ => panic!("foo"),
                                };
                                for shot in match salvo.get("shots").unwrap() {
                                    crate::rpc::typedefs::ArgValue::Array(a) => a,
                                    _ => panic!("foo"),
                                } {
                                    //println!("Shot: {:?}", shot);
                                    let shot = match shot {
                                        crate::rpc::typedefs::ArgValue::FixedDict(m) => m,
                                        _ => panic!("foo"),
                                    };
                                    if !self.artillery_shots.contains_key(&owner_id) {
                                        self.artillery_shots.insert(owner_id, vec![]);
                                    }
                                    self.artillery_shots.get_mut(&owner_id).unwrap().push(
                                        ArtilleryShot {
                                            start_time: packet.clock,
                                            start_pos: match shot.get("pos").unwrap() {
                                                crate::rpc::typedefs::ArgValue::Vector3(v) => {
                                                    v.clone()
                                                }
                                                _ => panic!("foo"),
                                            },
                                            target: match shot.get("tarPos").unwrap() {
                                                crate::rpc::typedefs::ArgValue::Vector3(v) => {
                                                    v.clone()
                                                }
                                                _ => panic!("foo"),
                                            },
                                        },
                                    );
                                }
                            }
                        }
                        _ => panic!("foo"),
                    }
                } else if *method == "receiveHitLocationsInitialState" {
                    println!(
                        "{}: receiveHitLocationsInitialState({}, {:?})",
                        time, entity_id, args
                    );
                }
            }
            _ => {}
        }
    }
}
