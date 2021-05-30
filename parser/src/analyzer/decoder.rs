use crate::analyzer::{Analyzer, AnalyzerBuilder};
use crate::packet2::{EntityMethodPacket, Packet, PacketType};
use crate::unpack_rpc_args;
use serde_derive::Serialize;
use std::collections::HashMap;

pub struct DecoderBuilder {}

impl DecoderBuilder {
    pub fn new() -> Self {
        Self {}
    }
}

impl AnalyzerBuilder for DecoderBuilder {
    fn build(&self, _: &crate::ReplayMeta) -> Box<dyn Analyzer> {
        Box::new(Decoder {})
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
    Other(&'rawpacket PacketType<'replay, 'argtype>),
    /*Position(PositionPacket),
    Entity(EntityPacket<'a>), // 0x7 and 0x8 are known to be of this type
    Chat(ChatPacket<'a>),
    Timing(TimingPacket),
    ArtilleryHit(ArtilleryHitPacket<'a>),
    Banner(Banner),
    DamageReceived(DamageReceivedPacket),
    Type24(Type24Packet),
    PlayerOrientation(PlayerOrientationPacket),
    Type8_79(Vec<(u32, u32)>),
    Setup(SetupPacket),
    ShipDestroyed(ShipDestroyedPacket),
    VoiceLine(VoiceLinePacket),
    Unknown(&'a [u8]),

    /// These are packets which we thought we understood, but couldn't parse
    Invalid(InvalidPacket<'a>),*/
}

#[derive(Debug, Serialize)]
struct DecodedPacket<'replay, 'argtype, 'rawpacket> {
    //pub packet_size: u32,
    //pub packet_type: u32,
    pub clock: f32,
    pub payload: DecodedPacketPayload<'replay, 'argtype, 'rawpacket>,
    //pub raw: &'a [u8],
}

struct Decoder {}

impl Analyzer for Decoder {
    fn finish(&self) {}

    fn process(&mut self, packet: &Packet<'_, '_>) {
        /*let time = packet.clock + self.time_offset;
        let minutes = (time / 60.0).floor() as i32;
        let seconds = (time - minutes as f32 * 60.0).floor() as i32;*/
        //println!("{:02}:{:02}: {:?}", minutes, seconds, packet.payload);
        println!("{}", serde_json::to_string(packet).unwrap());

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
                    let value = serde_pickle::de::value_from_slice(match &args[3] {
                        crate::rpc::typedefs::ArgValue::Blob(x) => x,
                        _ => panic!("foo"),
                    })
                    .unwrap();

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
                            /*self.usernames.insert(
                                match avatar {
                                    serde_pickle::value::Value::I64(i) => *i as i32,
                                    _ => panic!(),
                                },
                                username.to_string(),
                            );*/
                        }
                        println!("found {} players", players.len());
                    }
                    DecodedPacketPayload::Other(&packet.payload)
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
                } else if *method == "receiveDamageReport" {
                    DecodedPacketPayload::Other(&packet.payload)
                } else if *method == "receiveDamagesOnShip" {
                    DecodedPacketPayload::Other(&packet.payload)
                } else if *method == "receiveArtilleryShots" {
                    DecodedPacketPayload::Other(&packet.payload)
                } else if *method == "receiveHitLocationsInitialState" {
                    DecodedPacketPayload::Other(&packet.payload)
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
                } else {
                    //DecodedPacketPayload::Other(&packet.payload)
                    DecodedPacketPayload::EntityMethod(match &packet.payload {
                        PacketType::EntityMethod(em) => em,
                        _ => panic!(),
                    })
                }
            }
            PacketType::Position(pos) => DecodedPacketPayload::Position((*pos).clone()),
            PacketType::PlayerOrientation(pos) => {
                DecodedPacketPayload::PlayerOrientation((*pos).clone())
            }
            _ => DecodedPacketPayload::Other(&packet.payload),
        };
        let decoded = DecodedPacket {
            clock: packet.clock,
            payload: decoded,
        };
        //println!("{:#?}", decoded);
        println!("{}", serde_json::to_string_pretty(&decoded).unwrap());
    }
}
