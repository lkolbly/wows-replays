use crate::analyzer::{Analyzer, AnalyzerBuilder};
use crate::packet2::{EntityMethodPacket, Packet, PacketType};
use crate::unpack_rpc_args;
use modular_bitfield::prelude::*;
use serde_derive::Serialize;
use std::collections::HashMap;

pub struct DecoderBuilder {
    silent: bool,
    path: Option<String>,
}

impl DecoderBuilder {
    pub fn new(silent: bool, output: Option<&str>) -> Self {
        Self {
            silent,
            path: output.map(|s| s.to_string()),
        }
    }
}

impl AnalyzerBuilder for DecoderBuilder {
    fn build(&self, _: &crate::ReplayMeta) -> Box<dyn Analyzer> {
        Box::new(Decoder {
            silent: self.silent,
            output: self.path.as_ref().map(|path| {
                Box::new(std::fs::File::create(path).unwrap()) as Box<dyn std::io::Write>
            }),
        })
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum VoiceLine {
    IntelRequired,
    FairWinds,
    Wilco,
    Negative,
    WellDone,
    Curses,
    UsingRadar,
    UsingHydroSearch,
    DefendTheBase, // TODO: ...except when it's "thank you"?
    SetSmokeScreen,
    ProvideAntiAircraft,
    RequestingSupport(Option<u32>),
    Retreat(Option<i32>),

    /// Fields are (letter,number) and zero-indexed. e.g. F2 is (5,1)
    AttentionToSquare((u32, u32)),

    /// Field is the ID of the target
    ConcentrateFire(i32),
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, Serialize)]
pub enum Ribbon {
    PlaneShotDown,
    Incapacitation,
    SetFire,
    Citadel,
    SecondaryHit,
    OverPenetration,
    Penetration,
    NonPenetration,
    Ricochet,
    TorpedoProtectionHit,
    Captured,
    AssistedInCapture,
    Spotted,
    Destroyed,
    TorpedoHit,
    Defended,
    Flooding,
    DiveBombPenetration,
    RocketPenetration,
    RocketNonPenetration,
    RocketTorpedoProtectionHit,
    ShotDownByAircraft,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, Serialize)]
pub enum DeathCause {
    Secondaries,
    Artillery,
    Fire,
    Flooding,
    Torpedo,
    DiveBomber,
    AerialRocket,
    AerialTorpedo,
    Detonation,
    Ramming,
}

#[derive(Debug, Clone, Serialize)]
pub struct OnArenaStateReceivedPlayer {
    username: String,
    avatarid: i64,
    shipid: i64,
    playerid: i64,
    //playeravatarid: i64,

    // TODO: Replace String with the actual pickle value (which is cleanly serializable)
    raw: HashMap<i64, String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DamageReceived {
    target: i32,
    damage: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct MinimapUpdate {
    entity_id: i32,
    disappearing: bool,
    heading: f32,

    /// Zero is left edge, 1.0 is right edge
    x: f32,

    /// Zero is bottom edge, 1.0 is top edge
    y: f32,
}

#[derive(Debug, Serialize)]
enum DecodedPacketPayload<'replay, 'argtype, 'rawpacket> {
    Chat {
        entity_id: u32, // TODO: Is entity ID different than sender ID?
        sender_id: i32,
        audience: &'replay str,
        message: &'replay str,
    },
    VoiceLine {
        sender_id: i32,
        is_global: bool,
        message: VoiceLine,
    },
    Ribbon(Ribbon),
    Position(crate::packet2::PositionPacket),
    PlayerOrientation(crate::packet2::PlayerOrientationPacket),
    DamageStat(Vec<((i64, i64), (i64, f64))>),
    ShipDestroyed {
        killer: i32,
        victim: i32,
        cause: DeathCause,
    },
    EntityMethod(&'rawpacket EntityMethodPacket<'argtype>),
    EntityProperty(&'rawpacket crate::packet2::EntityPropertyPacket<'argtype>),
    BasePlayerCreate(&'rawpacket crate::packet2::BasePlayerCreatePacket<'replay>),
    CellPlayerCreate(&'rawpacket crate::packet2::CellPlayerCreatePacket<'replay>),
    EntityEnter(&'rawpacket crate::packet2::EntityEnterPacket),
    EntityLeave(&'rawpacket crate::packet2::EntityLeavePacket),
    EntityCreate(&'rawpacket crate::packet2::EntityCreatePacket<'argtype>),
    OnArenaStateReceived {
        arg0: i64,
        arg1: i8,
        arg2: HashMap<i64, Vec<Option<HashMap<String, String>>>>,
        players: Vec<OnArenaStateReceivedPlayer>,
    },
    CheckPing(u64),
    DamageReceived(Vec<DamageReceived>),
    MinimapUpdate(Vec<MinimapUpdate>),
    Unknown(&'replay [u8]),
    Invalid(&'rawpacket crate::packet2::InvalidPacket<'replay>),
    /*
    ArtilleryHit(ArtilleryHitPacket<'a>),
    Type24(Type24Packet),
    */
}

#[derive(Debug, Serialize)]
struct DecodedPacket<'replay, 'argtype, 'rawpacket> {
    //pub packet_size: u32,
    //pub packet_type: u32,
    pub clock: f32,
    pub payload: DecodedPacketPayload<'replay, 'argtype, 'rawpacket>,
    //pub raw: &'a [u8],
}

struct Decoder {
    silent: bool,
    output: Option<Box<dyn std::io::Write>>,
}

fn format_time_with_offset(offset: f32, clock: f32) -> String {
    let time = clock + offset;
    let minutes = (time / 60.0).floor() as i32;
    let seconds = (time - minutes as f32 * 60.0).floor() as i32;
    format!("{:02}:{:02}", minutes, seconds)
}

#[bitfield]
struct RawMinimapUpdate {
    x: B11,
    y: B11,
    heading: B8,
    unknown: bool,
    is_disappearing: bool,
}

impl Analyzer for Decoder {
    fn finish(&self) {}

    fn process(&mut self, packet: &Packet<'_, '_>) {
        /*let time = packet.clock + self.time_offset;
        let minutes = (time / 60.0).floor() as i32;
        let seconds = (time - minutes as f32 * 60.0).floor() as i32;*/
        //println!("{:02}:{:02}: {:?}", minutes, seconds, packet.payload);
        //println!("{}", serde_json::to_string(packet).unwrap());
        //println!("{:?}", packet);

        let decoded = match &packet.payload {
            PacketType::EntityMethod(EntityMethodPacket {
                entity_id,
                method,
                args,
            }) => {
                if *method == "onChatMessage" {
                    let target = match &args[1] {
                        crate::rpc::typedefs::ArgValue::String(s) => s,
                        _ => panic!("foo"),
                    };
                    let message = match &args[2] {
                        crate::rpc::typedefs::ArgValue::String(s) => s,
                        _ => panic!("foo"),
                    };
                    let sender_id = match &args[0] {
                        crate::rpc::typedefs::ArgValue::Int32(i) => i,
                        _ => panic!("foo"),
                    };
                    DecodedPacketPayload::Chat {
                        entity_id: *entity_id,
                        sender_id: *sender_id,
                        audience: std::str::from_utf8(&target).unwrap(),
                        message: std::str::from_utf8(&message).unwrap(),
                    }
                } else if *method == "receive_CommonCMD" {
                    let (audience, sender_id, line, a, b) =
                        unpack_rpc_args!(args, u8, i32, u8, u32, u64);

                    let is_global = match audience {
                        0 => false,
                        1 => true,
                        _ => {
                            panic!(format!(
                                "Got unknown audience {} sender=0x{:x} line={} a={:x} b={:x}",
                                audience, sender_id, line, a, b
                            ));
                        }
                    };
                    let message = match line {
                        1 => VoiceLine::AttentionToSquare((a, b as u32)),
                        2 => VoiceLine::ConcentrateFire(b as i32),
                        3 => VoiceLine::RequestingSupport(None),
                        5 => VoiceLine::Wilco,
                        6 => VoiceLine::Negative,
                        7 => VoiceLine::WellDone, // TODO: Find the corresponding field
                        8 => VoiceLine::FairWinds,
                        9 => VoiceLine::Curses,
                        10 => VoiceLine::DefendTheBase,
                        11 => VoiceLine::ProvideAntiAircraft,
                        12 => VoiceLine::Retreat(if b != 0 { Some(b as i32) } else { None }),
                        13 => VoiceLine::IntelRequired,
                        14 => VoiceLine::SetSmokeScreen,
                        15 => VoiceLine::UsingRadar,
                        16 => VoiceLine::UsingHydroSearch,
                        _ => {
                            panic!(format!("Unknown voice line {} a={:x} b={:x}!", line, a, b));
                        }
                    };

                    DecodedPacketPayload::VoiceLine {
                        sender_id,
                        is_global,
                        message,
                    }
                } else if *method == "onArenaStateReceived" {
                    let (arg0, arg1) = unpack_rpc_args!(args, i64, i8);

                    let value = serde_pickle::de::value_from_slice(match &args[2] {
                        crate::rpc::typedefs::ArgValue::Blob(x) => x,
                        _ => panic!("foo"),
                    })
                    .unwrap();

                    let value = match value {
                        serde_pickle::value::Value::Dict(d) => d,
                        _ => panic!(),
                    };
                    let mut arg2 = HashMap::new();
                    for (k, v) in value.iter() {
                        let k = match k {
                            serde_pickle::value::HashableValue::I64(i) => *i,
                            _ => panic!(),
                        };
                        let v = match v {
                            serde_pickle::value::Value::List(l) => l,
                            _ => panic!(),
                        };
                        let v: Vec<_> = v
                            .iter()
                            .map(|elem| match elem {
                                serde_pickle::value::Value::Dict(d) => Some(
                                    d.iter()
                                        .map(|(k, v)| {
                                            let k = match k {
                                                serde_pickle::value::HashableValue::Bytes(b) => {
                                                    std::str::from_utf8(b).unwrap().to_string()
                                                }
                                                _ => panic!(),
                                            };
                                            let v = format!("{:?}", v);
                                            (k, v)
                                        })
                                        .collect(),
                                ),
                                serde_pickle::value::Value::None => None,
                                _ => panic!(),
                            })
                            .collect();
                        arg2.insert(k, v);
                    }

                    let value = serde_pickle::de::value_from_slice(match &args[3] {
                        crate::rpc::typedefs::ArgValue::Blob(x) => x,
                        _ => panic!("foo"),
                    })
                    .unwrap();

                    let mut players_out = vec![];
                    if let serde_pickle::value::Value::List(players) = &value {
                        for player in players.iter() {
                            let mut values = HashMap::new();
                            if let serde_pickle::value::Value::List(elements) = player {
                                for elem in elements.iter() {
                                    if let serde_pickle::value::Value::Tuple(kv) = elem {
                                        let key = match kv[0] {
                                            serde_pickle::value::Value::I64(key) => key,
                                            _ => panic!(),
                                        };
                                        values.insert(key, kv[1].clone());
                                    }
                                }
                            }
                            //println!("onArenaStateReceived: {:#?}", values);
                            /*
                            1: Player ID
                            5: Clan name
                            16: Username
                            1c: Equipped equipment (?)
                            1d: Ship/hull ID? (1 more than player ID)
                            1e: Player ship ID
                            1f: Player ship ID (why does this appear twice?)
                            */
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
                            /*println!(
                                "{}: {}/{}/{}/{}",
                                username, avatar, shipid, playerid, playeravatarid
                            );*/
                            let mut raw = HashMap::new();
                            for (k, v) in values.iter() {
                                raw.insert(*k, format!("{:?}", v));
                            }
                            players_out.push(OnArenaStateReceivedPlayer {
                                username: username.to_string(),
                                avatarid: match avatar {
                                    serde_pickle::value::Value::I64(i) => *i,
                                    _ => panic!("foo"),
                                },
                                shipid: match shipid {
                                    serde_pickle::value::Value::I64(i) => *i,
                                    _ => panic!("foo"),
                                },
                                playerid: match playerid {
                                    serde_pickle::value::Value::I64(i) => *i,
                                    _ => panic!("foo"),
                                },
                                raw: raw,
                            });
                            /*self.usernames.insert(
                                match avatar {
                                    serde_pickle::value::Value::I64(i) => *i as i32,
                                    _ => panic!(),
                                },
                                username.to_string(),
                            );*/
                        }
                        //println!("found {} players", players.len());
                    }
                    DecodedPacketPayload::OnArenaStateReceived {
                        arg0,
                        arg1,
                        arg2,
                        players: players_out,
                    }
                } else if *method == "receiveDamageStat" {
                    let value = serde_pickle::de::value_from_slice(match &args[0] {
                        crate::rpc::typedefs::ArgValue::Blob(x) => x,
                        _ => panic!("foo"),
                    })
                    .unwrap();

                    let mut stats = vec![];
                    match value {
                        serde_pickle::value::Value::Dict(d) => {
                            for (k, v) in d.iter() {
                                let k = match k {
                                    serde_pickle::value::HashableValue::Tuple(t) => {
                                        assert!(t.len() == 2);
                                        (
                                            match t[0] {
                                                serde_pickle::value::HashableValue::I64(i) => i,
                                                _ => panic!("foo"),
                                            },
                                            match t[1] {
                                                serde_pickle::value::HashableValue::I64(i) => i,
                                                _ => panic!("foo"),
                                            },
                                        )
                                    }
                                    _ => panic!("foo"),
                                };
                                let v = match v {
                                    serde_pickle::value::Value::List(t) => {
                                        assert!(t.len() == 2);
                                        (
                                            match t[0] {
                                                serde_pickle::value::Value::I64(i) => i,
                                                _ => panic!("foo"),
                                            },
                                            match t[1] {
                                                serde_pickle::value::Value::F64(i) => i,
                                                // TODO: This appears in the (17,2) key,
                                                // it is unknown what it means
                                                serde_pickle::value::Value::I64(i) => i as f64,
                                                _ => panic!("foo"),
                                            },
                                        )
                                    }
                                    _ => panic!("foo"),
                                };
                                //println!("{:?}: {:?}", k, v);

                                // The (1,0) key is (# AP hits that dealt damage, total AP damage dealt)
                                // (1,3) is (# artillery fired, total possible damage) ?
                                // (2, 0) is (# HE penetrations, total HE damage)
                                // (17, 0) is (# fire tick marks, total fire damage)
                                stats.push((k, v));
                            }
                        }
                        _ => panic!("foo"),
                    }
                    DecodedPacketPayload::DamageStat(stats)
                } else if *method == "receiveVehicleDeath" {
                    let (victim, killer, cause) = unpack_rpc_args!(args, i32, i32, u32);
                    let cause = match cause {
                        2 => DeathCause::Secondaries,
                        3 => DeathCause::Torpedo,
                        4 => DeathCause::DiveBomber,
                        5 => DeathCause::AerialTorpedo,
                        6 => DeathCause::Fire,
                        7 => DeathCause::Ramming,
                        9 => DeathCause::Flooding,
                        14 => DeathCause::AerialRocket,
                        15 => DeathCause::Detonation,
                        17 => DeathCause::Artillery,
                        18 => DeathCause::Artillery,
                        19 => DeathCause::Artillery,
                        _ => {
                            panic!(format!("Found unknown death_cause {}", cause));
                        }
                    };
                    DecodedPacketPayload::ShipDestroyed {
                        victim,
                        killer,
                        cause,
                    }
                } else if *method == "onRibbon" {
                    let (ribbon,) = unpack_rpc_args!(args, i8);
                    let ribbon = match ribbon {
                        1 => Ribbon::TorpedoHit,
                        3 => Ribbon::PlaneShotDown,
                        4 => Ribbon::Incapacitation,
                        5 => Ribbon::Destroyed,
                        6 => Ribbon::SetFire,
                        7 => Ribbon::Flooding,
                        8 => Ribbon::Citadel,
                        9 => Ribbon::Defended,
                        10 => Ribbon::Captured,
                        11 => Ribbon::AssistedInCapture,
                        13 => Ribbon::SecondaryHit,
                        14 => Ribbon::OverPenetration,
                        15 => Ribbon::Penetration,
                        16 => Ribbon::NonPenetration,
                        17 => Ribbon::Ricochet,
                        19 => Ribbon::Spotted,
                        21 => Ribbon::DiveBombPenetration,
                        25 => Ribbon::RocketPenetration,
                        26 => Ribbon::RocketNonPenetration,
                        27 => Ribbon::ShotDownByAircraft,
                        28 => Ribbon::TorpedoProtectionHit,
                        30 => Ribbon::RocketTorpedoProtectionHit,
                        _ => {
                            panic!("Unrecognized ribbon {}", ribbon);
                        }
                    };
                    DecodedPacketPayload::Ribbon(ribbon)
                } else if *method == "receiveDamagesOnShip" {
                    let mut v = vec![];
                    for elem in match &args[0] {
                        crate::rpc::typedefs::ArgValue::Array(a) => a,
                        _ => panic!(),
                    } {
                        let map = match elem {
                            crate::rpc::typedefs::ArgValue::FixedDict(m) => m,
                            _ => panic!(),
                        };
                        v.push(DamageReceived {
                            target: match map.get("vehicleID").unwrap() {
                                crate::rpc::typedefs::ArgValue::Int32(i) => *i,
                                _ => panic!(),
                            },
                            damage: match map.get("damage").unwrap() {
                                crate::rpc::typedefs::ArgValue::Float32(f) => *f,
                                _ => panic!(),
                            },
                        });
                    }
                    DecodedPacketPayload::DamageReceived(v)
                } else if *method == "onCheckGamePing" {
                    let (ping,) = unpack_rpc_args!(args, u64);
                    DecodedPacketPayload::CheckPing(ping)
                } else if *method == "updateMinimapVisionInfo" {
                    let v = match &args[0] {
                        crate::rpc::typedefs::ArgValue::Array(a) => a,
                        _ => panic!(),
                    };
                    let mut updates = vec![];
                    for minimap_update in v.iter() {
                        let minimap_update = match minimap_update {
                            crate::rpc::typedefs::ArgValue::FixedDict(m) => m,
                            _ => panic!(),
                        };
                        let vehicle_id = minimap_update.get("vehicleID").unwrap();

                        let packed_data = match minimap_update.get("packedData").unwrap() {
                            crate::rpc::typedefs::ArgValue::Uint32(u) => *u,
                            _ => panic!(),
                        };
                        let update = RawMinimapUpdate::from_bytes(packed_data.to_le_bytes());
                        let heading = update.heading() as f32 / 256. * 360. - 180.;

                        let x = update.x() as f32 / 512. - 1.5;
                        let y = update.y() as f32 / 512. - 1.5;

                        assert!(update.unknown() == false);
                        updates.push(MinimapUpdate {
                            entity_id: match vehicle_id {
                                crate::rpc::typedefs::ArgValue::Uint32(u) => *u as i32,
                                _ => panic!(),
                            },
                            x,
                            y,
                            heading,
                            disappearing: update.is_disappearing(),
                        })
                    }

                    let args1 = match &args[1] {
                        crate::rpc::typedefs::ArgValue::Array(a) => a,
                        _ => panic!(),
                    };
                    assert!(args1.len() == 0);

                    DecodedPacketPayload::MinimapUpdate(updates)
                } else {
                    //DecodedPacketPayload::Other(&packet.payload)
                    DecodedPacketPayload::EntityMethod(match &packet.payload {
                        PacketType::EntityMethod(em) => em,
                        _ => panic!(),
                    })
                }
            }
            PacketType::EntityProperty(p) => DecodedPacketPayload::EntityProperty(p),
            PacketType::Position(pos) => DecodedPacketPayload::Position((*pos).clone()),
            PacketType::PlayerOrientation(pos) => {
                DecodedPacketPayload::PlayerOrientation((*pos).clone())
            }
            PacketType::BasePlayerCreate(b) => DecodedPacketPayload::BasePlayerCreate(b),
            PacketType::CellPlayerCreate(c) => DecodedPacketPayload::CellPlayerCreate(c),
            PacketType::EntityEnter(e) => DecodedPacketPayload::EntityEnter(e),
            PacketType::EntityLeave(e) => DecodedPacketPayload::EntityLeave(e),
            PacketType::EntityCreate(e) => DecodedPacketPayload::EntityCreate(e),
            PacketType::Unknown(u) => DecodedPacketPayload::Unknown(&u),
            PacketType::Invalid(u) => DecodedPacketPayload::Invalid(&u),
        };
        let decoded = DecodedPacket {
            clock: packet.clock,
            payload: decoded,
        };
        //println!("{:#?}", decoded);
        //println!("{}", serde_json::to_string_pretty(&decoded).unwrap());
        if !self.silent {
            let encoded = serde_json::to_string(&decoded).unwrap();
            match self.output.as_mut() {
                Some(f) => {
                    writeln!(f, "{}", encoded);
                }
                None => {
                    println!("{}", encoded);
                }
            }
        }
    }
}
