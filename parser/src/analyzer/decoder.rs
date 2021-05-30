use crate::analyzer::{Analyzer, AnalyzerBuilder};
use crate::packet2::{EntityMethodPacket, Packet, PacketType};
use crate::unpack_rpc_args;
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

#[derive(Debug, Clone, Copy)]
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

#[derive(Debug)]
enum DecodedPacket<'a, 'b, 'c> {
    Chat {
        entity_id: u32, // TODO: Is entity ID different than sender ID?
        sender_id: i32,
        audience: &'a str,
        message: &'a str,
    },
    VoiceLine {
        sender_id: i32,
        is_global: bool,
        message: VoiceLine,
    },
    Other(&'c Packet<'a, 'b>),
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
                    DecodedPacket::Chat {
                        entity_id: *entity_id,
                        sender_id: *sender_id,
                        audience: std::str::from_utf8(&target).unwrap(),
                        message: std::str::from_utf8(&message).unwrap(),
                    }
                /*println!(
                    "{}: {}: {} {}",
                    clock,
                    self.usernames.get(sender_id).unwrap(),
                    std::str::from_utf8(&target).unwrap(),
                    std::str::from_utf8(&message).unwrap()
                );*/
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

                    DecodedPacket::VoiceLine {
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
                    DecodedPacket::Other(packet)
                } else if *method == "receiveDamageStat" {
                    DecodedPacket::Other(packet)
                } else if *method == "receiveDamageReport" {
                    DecodedPacket::Other(packet)
                } else if *method == "receiveDamagesOnShip" {
                    DecodedPacket::Other(packet)
                } else if *method == "receiveArtilleryShots" {
                    DecodedPacket::Other(packet)
                } else if *method == "receiveHitLocationsInitialState" {
                    DecodedPacket::Other(packet)
                } else if *method == "onRibbon" {
                    DecodedPacket::Other(packet)
                } else {
                    DecodedPacket::Other(packet)
                }
            }
            PacketType::Position(pos) => DecodedPacket::Other(packet),
            PacketType::PlayerOrientation(pos) => DecodedPacket::Other(packet),
            _ => DecodedPacket::Other(packet),
        };
        println!("{:#?}", decoded);
    }
}
