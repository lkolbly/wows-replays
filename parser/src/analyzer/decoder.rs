use crate::analyzer::{Analyzer, AnalyzerBuilder};
use crate::packet2::{EntityMethodPacket, Packet, PacketType};
use crate::unpack_rpc_args;
use modular_bitfield::prelude::*;
use serde_derive::Serialize;
use std::collections::HashMap;

pub struct DecoderBuilder {
    silent: bool,
    no_meta: bool,
    path: Option<String>,
}

impl DecoderBuilder {
    pub fn new(silent: bool, no_meta: bool, output: Option<&str>) -> Self {
        Self {
            silent,
            no_meta,
            path: output.map(|s| s.to_string()),
        }
    }
}

impl AnalyzerBuilder for DecoderBuilder {
    fn build(&self, meta: &crate::ReplayMeta) -> Box<dyn Analyzer> {
        let version = crate::version::Version::from_client_exe(&meta.clientVersionFromExe);
        let mut decoder = Decoder {
            silent: self.silent,
            output: self.path.as_ref().map(|path| {
                Box::new(std::fs::File::create(path).unwrap()) as Box<dyn std::io::Write>
            }),
            version: version,
        };
        if !self.no_meta {
            decoder.write(&serde_json::to_string(&meta).unwrap());
        }
        Box::new(decoder)
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
    Unknown(i8),
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
    Unknown(u32),
}

#[derive(Debug, Clone, Serialize)]
pub struct OnArenaStateReceivedPlayer {
    pub username: String,
    pub clan: String,
    pub avatarid: i64,
    pub shipid: i64,
    pub playerid: i64,
    //playeravatarid: i64,
    pub teamid: i64,
    pub health: i64,

    // TODO: Replace String with the actual pickle value (which is cleanly serializable)
    pub raw: HashMap<i64, String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DamageReceived {
    aggressor: i32,
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

    /// This appears to be something related to the big hunt
    unknown: bool,
}

#[derive(Debug, Serialize)]
pub enum DecodedPacketPayload<'replay, 'argtype, 'rawpacket> {
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
    BasePlayerCreate(&'rawpacket crate::packet2::BasePlayerCreatePacket<'replay, 'argtype>),
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
    DamageReceived {
        victim: u32,
        aggressors: Vec<DamageReceived>,
    },
    MinimapUpdate {
        updates: Vec<MinimapUpdate>,
        arg1: &'rawpacket Vec<crate::rpc::typedefs::ArgValue<'argtype>>,
    },
    PropertyUpdate(&'rawpacket crate::packet2::PropertyUpdatePacket<'argtype>),
    BattleEnd {
        winning_team: i8,
        unknown: u8,
    },
    Unknown(&'replay [u8]),
    Invalid(&'rawpacket crate::packet2::InvalidPacket<'replay>),
    /*
    ArtilleryHit(ArtilleryHitPacket<'a>),
    Type24(Type24Packet),
    */
}

#[derive(Debug, Serialize)]
pub struct DecodedPacket<'replay, 'argtype, 'rawpacket> {
    pub packet_type: u32,
    pub clock: f32,
    pub payload: DecodedPacketPayload<'replay, 'argtype, 'rawpacket>,
}

impl<'replay, 'argtype, 'rawpacket> DecodedPacket<'replay, 'argtype, 'rawpacket>
where
    'rawpacket: 'replay,
    'rawpacket: 'argtype,
{
    pub fn from(version: &crate::version::Version, packet: &'rawpacket Packet<'_, '_>) -> Self {
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
                            panic!(
                                "Got unknown audience {} sender=0x{:x} line={} a={:x} b={:x}",
                                audience, sender_id, line, a, b
                            );
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
                            panic!("Unknown voice line {} a={:x} b={:x}!", line, a, b);
                        }
                    };

                    DecodedPacketPayload::VoiceLine {
                        sender_id,
                        is_global,
                        message,
                    }
                } else if *method == "onArenaStateReceived" {
                    let (arg0, arg1) = unpack_rpc_args!(args, i64, i8);

                    let value = serde_pickle::de::value_from_slice(
                        match &args[2] {
                            crate::rpc::typedefs::ArgValue::Blob(x) => x,
                            _ => panic!("foo"),
                        },
                        serde_pickle::de::DeOptions::new(),
                    )
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

                    let value = serde_pickle::de::value_from_slice(
                        match &args[3] {
                            crate::rpc::typedefs::ArgValue::Blob(x) => x,
                            _ => panic!("foo"),
                        },
                        serde_pickle::de::DeOptions::new(),
                    )
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

                            let keys: HashMap<&'static str, i64> = if version
                                .is_at_least(&crate::version::Version::from_client_exe("0,10,7,0"))
                            {
                                // 0.10.7
                                let mut h = HashMap::new();
                                h.insert("avatarid", 0x1);
                                h.insert("clan", 0x5);
                                h.insert("health", 0x16);
                                h.insert("username", 0x17);
                                h.insert("shipid", 0x1e);
                                h.insert("playerid", 0x1f);
                                h.insert("playeravatarid", 0x20);
                                h.insert("team", 0x21);
                                h
                            } else {
                                // 0.10.6 and earlier
                                let mut h = HashMap::new();
                                h.insert("avatarid", 0x1);
                                h.insert("clan", 0x5);
                                h.insert("health", 0x15);
                                h.insert("username", 0x16);
                                h.insert("shipid", 0x1d);
                                h.insert("playerid", 0x1e);
                                h.insert("playeravatarid", 0x1f);
                                h.insert("team", 0x20);
                                h
                            };

                            /*
                            1: Player ID
                            5: Clan name
                            16: Username
                            1c: Equipped equipment (?)
                            1d: Ship/hull ID? (1 more than player ID)
                            1e: Player ship ID
                            1f: Player ship ID (why does this appear twice?)
                            */
                            let avatar = values.get(keys.get("avatarid").unwrap()).unwrap();
                            let username = values.get(keys.get("username").unwrap()).unwrap();
                            let username = std::str::from_utf8(match username {
                                serde_pickle::value::Value::Bytes(u) => u,
                                _ => {
                                    panic!("{:?}", username);
                                }
                            })
                            .unwrap();
                            let clan = values.get(keys.get("clan").unwrap()).unwrap();
                            let clan = match clan {
                                serde_pickle::value::Value::String(s) => s.clone(),
                                _ => {
                                    panic!("{:?}", clan);
                                }
                            };
                            let shipid = values.get(keys.get("shipid").unwrap()).unwrap();
                            let playerid = values.get(keys.get("playerid").unwrap()).unwrap();
                            let _playeravatarid =
                                values.get(keys.get("playeravatarid").unwrap()).unwrap();
                            let team = values.get(keys.get("team").unwrap()).unwrap();
                            let health = values.get(keys.get("health").unwrap()).unwrap();

                            let mut raw = HashMap::new();
                            for (k, v) in values.iter() {
                                raw.insert(*k, format!("{:?}", v));
                            }
                            players_out.push(OnArenaStateReceivedPlayer {
                                username: username.to_string(),
                                clan: clan,
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
                                teamid: match team {
                                    serde_pickle::value::Value::I64(i) => *i,
                                    _ => panic!("foo"),
                                },
                                health: match health {
                                    serde_pickle::value::Value::I64(i) => *i,
                                    _ => panic!("foo"),
                                },
                                raw: raw,
                            });
                        }
                    }
                    DecodedPacketPayload::OnArenaStateReceived {
                        arg0,
                        arg1,
                        arg2,
                        players: players_out,
                    }
                } else if *method == "receiveDamageStat" {
                    let value = serde_pickle::de::value_from_slice(
                        match &args[0] {
                            crate::rpc::typedefs::ArgValue::Blob(x) => x,
                            _ => panic!("foo"),
                        },
                        serde_pickle::de::DeOptions::new(),
                    )
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
                        cause => DeathCause::Unknown(cause),
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
                        ribbon => Ribbon::Unknown(ribbon),
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
                            aggressor: match map.get("vehicleID").unwrap() {
                                crate::rpc::typedefs::ArgValue::Int32(i) => *i,
                                _ => panic!(),
                            },
                            damage: match map.get("damage").unwrap() {
                                crate::rpc::typedefs::ArgValue::Float32(f) => *f,
                                _ => panic!(),
                            },
                        });
                    }
                    DecodedPacketPayload::DamageReceived {
                        victim: *entity_id,
                        aggressors: v,
                    }
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

                        updates.push(MinimapUpdate {
                            entity_id: match vehicle_id {
                                crate::rpc::typedefs::ArgValue::Uint32(u) => *u as i32,
                                _ => panic!(),
                            },
                            x,
                            y,
                            heading,
                            disappearing: update.is_disappearing(),
                            unknown: update.unknown(),
                        })
                    }

                    let args1 = match &args[1] {
                        crate::rpc::typedefs::ArgValue::Array(a) => a,
                        _ => panic!(),
                    };

                    DecodedPacketPayload::MinimapUpdate {
                        updates,
                        arg1: args1,
                    }
                } else if *method == "onBattleEnd" {
                    let winning_team = match &args[0] {
                        crate::rpc::typedefs::ArgValue::Int8(i) => *i,
                        _ => panic!("foo"),
                    };
                    let unknown = match &args[1] {
                        crate::rpc::typedefs::ArgValue::Uint8(i) => *i,
                        _ => panic!("foo"),
                    };
                    DecodedPacketPayload::BattleEnd {
                        winning_team,
                        unknown,
                    }
                } else {
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
            PacketType::PropertyUpdate(update) => DecodedPacketPayload::PropertyUpdate(update),
            PacketType::Unknown(u) => DecodedPacketPayload::Unknown(&u),
            PacketType::Invalid(u) => DecodedPacketPayload::Invalid(&u),
        };
        let decoded = Self {
            clock: packet.clock,
            packet_type: packet.packet_type,
            payload: decoded,
        };
        decoded
    }
}

struct Decoder {
    silent: bool,
    output: Option<Box<dyn std::io::Write>>,
    version: crate::version::Version,
}

impl Decoder {
    fn write(&mut self, line: &str) {
        if !self.silent {
            match self.output.as_mut() {
                Some(f) => {
                    writeln!(f, "{}", line).unwrap();
                }
                None => {
                    println!("{}", line);
                }
            }
        }
    }
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
        let decoded = DecodedPacket::from(&self.version, packet);
        //println!("{:#?}", decoded);
        //println!("{}", serde_json::to_string_pretty(&decoded).unwrap());
        let encoded = serde_json::to_string(&decoded).unwrap();
        self.write(&encoded);
    }
}
