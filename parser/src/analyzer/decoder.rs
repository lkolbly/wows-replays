use crate::analyzer::{Analyzer, AnalyzerBuilder};
use crate::packet2::{Entity, EntityMethodPacket, Packet, PacketType};
use crate::{unpack_rpc_args, ErrorKind, IResult};
use modular_bitfield::prelude::*;
use nom::number::complete::{le_f32, le_i32, le_u16, le_u32, le_u64, le_u8};
use serde::Serialize;
use serde_pickle::Value;
use std::collections::HashMap;
use std::convert::TryInto;
use std::iter::FromIterator;

use super::analyzer::{AnalyzerMut, AnalyzerMutBuilder};

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

impl AnalyzerMutBuilder for DecoderBuilder {
    fn build(&self, meta: &crate::ReplayMeta) -> Box<dyn AnalyzerMut> {
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

/// Enumerates voicelines which can be said in the game.
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
    FollowMe,
    // TODO: definitely has associated data similar to AttentionToSquare
    /// World x and y coordinates corresponding to the map grid
    /// MapPointQuickCommand in game code
    MapPointAttention(f32, f32),
    UsingSubmarineLocator,
    /// "Provide anti-aircraft support"
    ProvideAntiAircraft,
    /// If a player is called out in the message, their avatar ID will be here.
    RequestingSupport(Option<u32>),
    /// If a player is called out in the message, their avatar ID will be here.
    Retreat(Option<i32>),

    /// The position is (letter,number) and zero-indexed. e.g. F2 is (5,1)
    /// `RectangleAttentionCommand`` in game code
    AttentionToSquare(u32, u32),

    /// Field is the avatar ID of the target
    /// Pair of the target type and target ID
    QuickTactic(u16, u64),
}

/// Enumerates the ribbons which appear in the top-right
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
    DepthChargeHit,
    ShotDownByAircraft,
    BuffSeized,
    SonarOneHit,
    SonarTwoHits,
    SonarNeutralized,
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
    DepthCharge,
    SkipBombs,
    Unknown(u32),
}

/// Contains the information describing a player
#[derive(Debug, Clone, Serialize)]
pub struct OnArenaStateReceivedPlayer {
    /// The username of this player
    pub username: String,
    /// The player's clan
    pub clan: String,
    /// Their avatar ID in the game
    pub avatar_id: i64,
    /// Their ship ID in the game
    pub meta_ship_id: i64,
    /// This player's entity created by a CreateEntity packet
    pub entity_id: i64,
    //playeravatarid: i64,
    /// Which team they're on.
    pub team_id: i64,
    /// Their starting health
    pub max_health: i64,

    /// This is a raw dump (with the values converted to strings) of every key for the player.
    // TODO: Replace String with the actual pickle value (which is cleanly serializable)
    pub raw: HashMap<i64, String>,
}

/// Indicates that the given attacker has dealt damage
#[derive(Debug, Clone, Serialize)]
pub struct DamageReceived {
    /// Ship ID of the aggressor
    pub aggressor: i32,
    /// Amount of damage dealt
    pub damage: f32,
}

/// Sent to update the minimap display
#[derive(Debug, Clone, Serialize)]
pub struct MinimapUpdate {
    /// The ship ID of the ship to update
    pub entity_id: i32,
    /// Set to true if the ship should disappear from the minimap (false otherwise)
    pub disappearing: bool,
    /// The heading of the ship. Unit is degrees, 0 is up, positive is clockwise
    /// (so 90.0 is East)
    pub heading: f32,

    /// Zero is the left edge of the map, 1.0 is the right edge
    pub x: f32,

    /// Zero is the bottom edge of the map, 1.0 is the top edge
    pub y: f32,

    /// Unknown, but this appears to be something related to the big hunt
    pub unknown: bool,
}

/// Enumerates usable consumables in-game
#[derive(Debug, Clone, Copy, Serialize)]
pub enum Consumable {
    DamageControl,
    SpottingAircraft,
    DefensiveAntiAircraft,
    SpeedBoost,
    RepairParty,
    CatapultFighter,
    MainBatteryReloadBooster,
    TorpedoReloadBooster,
    Smoke,
    Radar,
    HydroacousticSearch,
    Hydrophone,
    EnhancedRudders,
    ReserveBattery,
    Unknown(i8),
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum CameraMode {
    OverheadMap,
    FollowingShells,
    FollowingPlanes,
    FollowingShip,
    FollowingSubmarine,
    FreeFlying,
    Unknown(u32),
}

/// Enumerates the "cruise states". See <https://github.com/lkolbly/wows-replays/issues/14#issuecomment-976784004>
/// for more information.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum CruiseState {
    /// Possible values for the throttle range from -1 for reverse to 4 for full power ahead.
    Throttle,
    /// Note that not all rudder changes are indicated via cruise states, only ones
    /// set via the Q & E keys. Temporarily setting the rudder will not trigger this
    /// packet.
    ///
    /// Possible associated values are:
    /// - -2: Full rudder to port,
    /// - -1: Half rudder to port,
    /// - 0: Neutral
    /// - 1: Half rudder to starboard,
    /// - 2: Full rudder to starboard.
    Rudder,
    /// Sets the dive depth. Known values are:
    /// - 0: 0m
    /// - 1: -6m (periscope depth)
    /// - 2: -18m
    /// - 3: -30m
    /// - 4: -42m
    /// - 5: -54m
    /// - 6: -66m
    /// - 7: -80m
    DiveDepth,
    /// Indicates an unknown cruise state. Send me your replay!
    Unknown(u32),
}

#[derive(Debug, Serialize)]
pub struct ChatMessageExtra {
    pre_battle_sign: i64,
    pre_battle_id: i64,
    player_clan_tag: String,
    typ: i64,
    player_avatar_id: i64,
    player_name: String,
}

#[derive(Debug, Serialize)]
pub enum DecodedPacketPayload<'replay, 'argtype, 'rawpacket> {
    /// Represents a chat message. Note that this only includes text chats, voicelines
    /// are represented by the VoiceLine variant.
    Chat {
        entity_id: u32, // TODO: Is entity ID different than sender ID?
        /// Avatar ID of the sender
        sender_id: i32,
        /// Represents the audience for the chat: Division, team, or all.
        audience: &'replay str,
        /// The actual chat message.
        message: &'replay str,
        /// Extra data that may be present if sender_id is 0
        extra_data: Option<ChatMessageExtra>,
    },
    /// Sent when a voice line is played (for example, "Wilco!")
    VoiceLine {
        /// Avatar ID of the player sending the voiceline
        sender_id: i32,
        /// True if the voiceline is visible in all chat, false if only in team chat
        is_global: bool,
        /// Which voiceline it is.
        message: VoiceLine,
    },
    /// Sent when the player earns a ribbon
    Ribbon(Ribbon),
    /// Indicates the position of the given object.
    Position(crate::packet2::PositionPacket),
    /// Indicates the position of the player's object or camera.
    PlayerOrientation(crate::packet2::PlayerOrientationPacket),
    /// Indicates updating a damage statistic. The first tuple, `(i64,i64)`, is a two-part
    /// label indicating what type of damage this refers to. The second tuple, `(i64,f64)`,
    /// indicates the actual damage counter increment.
    ///
    /// Some known keys include:
    /// - (1, 0) key is (# AP hits that dealt damage, total AP damage dealt)
    /// - (1, 3) is (# artillery fired, total possible damage) ?
    /// - (2, 0) is (# HE penetrations, total HE damage)
    /// - (17, 0) is (# fire tick marks, total fire damage)
    DamageStat(Vec<((i64, i64), (i64, f64))>),
    /// Sent when a ship is destroyed.
    ShipDestroyed {
        /// The ship ID (note: Not the avatar ID) of the killer
        killer: i32,
        /// The ship ID (note: Not the avatar ID) of the victim
        victim: i32,
        /// Cause of death
        cause: DeathCause,
    },
    EntityMethod(&'rawpacket EntityMethodPacket<'argtype>),
    EntityProperty(&'rawpacket crate::packet2::EntityPropertyPacket<'argtype>),
    BasePlayerCreate(&'rawpacket crate::packet2::BasePlayerCreatePacket<'replay, 'argtype>),
    CellPlayerCreate(&'rawpacket crate::packet2::CellPlayerCreatePacket<'replay>),
    EntityEnter(&'rawpacket crate::packet2::EntityEnterPacket),
    EntityLeave(&'rawpacket crate::packet2::EntityLeavePacket),
    EntityCreate(&'rawpacket crate::packet2::EntityCreatePacket<'argtype>),
    /// Contains all of the info required to setup the arena state and show the initial loading screen.
    OnArenaStateReceived {
        /// Unknown
        arg0: i64,
        /// Unknown
        arg1: i8,
        /// Unknown
        arg2: HashMap<i64, Vec<Option<HashMap<String, String>>>>,
        /// A list of the players in this game
        players: Vec<OnArenaStateReceivedPlayer>,
    },
    CheckPing(u64),
    /// Indicates that the given victim has received damage from one or more attackers.
    DamageReceived {
        /// Ship ID of the ship being damaged
        victim: u32,
        /// List of damages happening to this ship
        aggressors: Vec<DamageReceived>,
    },
    /// Contains data for a minimap update
    MinimapUpdate {
        /// A list of the updates to make to the minimap
        updates: Vec<MinimapUpdate>,
        /// Unknown
        arg1: &'rawpacket Vec<crate::rpc::typedefs::ArgValue<'argtype>>,
    },
    /// Indicates a property update. Note that many properties contain a hierarchy of properties,
    /// for example the "state" property on the battle manager contains nested dictionaries and
    /// arrays. The top-level entity and property are specified by the `entity_id` and `property`
    /// fields. The nesting structure and how to modify the leaves are indicated by the
    /// `update_cmd` field.
    ///
    /// Within the `update_cmd` field is two fields, `levels` and `action`. `levels` indicates how
    /// to traverse to the leaf property, for example by following a dictionary key or array index.
    /// `action` indicates what action to perform once there, such as setting a subproperty to
    /// a specific value.
    ///
    /// For example, to set the `state[controlPoints][0][hasInvaders]` property, you will see a
    /// packet payload that looks like:
    /// ```ignore
    /// {
    ///     "entity_id": 576258,
    ///     "property": "state",
    ///     "update_cmd": {
    ///         "levels": [
    ///             {"DictKey": "controlPoints"},
    ///             {"ArrayIndex": 0}
    ///         ],
    ///         "action": {
    ///             "SetKey":{"key":"hasInvaders","value":1}
    ///         }
    ///     }
    /// }
    /// ```
    /// This says to take the "state" property on entity 576258, navigate to `state["controlPoints"][0]`,
    /// and set the sub-key `hasInvaders` there to 1.
    ///
    /// The following properties and values are known:
    /// - `state["controlPoints"][N]["invaderTeam"]`: Indicates the team ID of the team currently
    ///   contesting the control point. -1 if nobody is invading point.
    /// - `state["controlPoints"][N]["hasInvaders"]`: 1 if the point is being contested, 0 otherwise.
    /// - `state["controlPoints"][N]["progress"]`: A tuple of two elements. The first is the fraction
    ///   captured, ranging from 0 to 1 as the point is captured, and the second is the amount of
    ///   time remaining until the point is captured.
    /// - `state["controlPoints"][N]["bothInside"]`: 1 if both teams are currently in point, 0 otherwise.
    /// - `state["missions"]["teamsScore"][N]["score"]`: The value of team N's score.
    PropertyUpdate(&'rawpacket crate::packet2::PropertyUpdatePacket<'argtype>),
    /// Indicates that the battle has ended
    BattleEnd {
        /// The team ID of the winning team (corresponds to the teamid in [OnArenaStateReceivedPlayer])
        winning_team: Option<i8>,
        /// Unknown
        // TODO: Probably how the game was won? (time expired, score, or ships destroyed)
        unknown: Option<u8>,
    },
    /// Sent when a consumable is activated
    Consumable {
        /// The ship ID of the ship using the consumable
        entity: u32,
        /// The consumable
        consumable: Consumable,
        /// How long the consumable will be active for
        duration: f32,
    },
    /// Indicates a change to the "cruise state," which is the fixed settings for various controls
    /// such as steering (using the Q & E keys), throttle, and dive planes.
    CruiseState {
        /// Which cruise state is being affected
        state: CruiseState,
        /// See [CruiseState] for what the values mean.
        value: i32,
    },
    Map(&'rawpacket crate::packet2::MapPacket<'replay>),
    /// A string representation of the game version this replay is from.
    Version(String),
    Camera(&'rawpacket crate::packet2::CameraPacket),
    /// Indicates a change in the current camera mode
    CameraMode(CameraMode),
    /// If true, indicates that the player has enabled the "free look" camera (by holding right click)
    CameraFreeLook(bool),
    /// This is a packet of unknown type
    Unknown(&'replay [u8]),
    /// This is a packet of known type, but which we were unable to parse
    Invalid(&'rawpacket crate::packet2::InvalidPacket<'replay>),
    /// If parsing with audits enabled, this indicates a packet that may be of special interest
    /// for whoever is reading the audits.
    Audit(String),
    /// End of battle results (free xp, damage details, etc.)
    BattleResults(&'replay str),
    /*
    ArtilleryHit(ArtilleryHitPacket<'a>),
    */
}

fn try_convert_hashable_pickle_to_string(
    value: serde_pickle::value::HashableValue,
) -> serde_pickle::value::HashableValue {
    match value {
        serde_pickle::value::HashableValue::Bytes(b) => {
            if let Ok(s) = std::str::from_utf8(&b) {
                serde_pickle::value::HashableValue::String(s.to_owned())
            } else {
                serde_pickle::value::HashableValue::Bytes(b)
            }
        }
        serde_pickle::value::HashableValue::Tuple(t) => serde_pickle::value::HashableValue::Tuple(
            t.into_iter()
                .map(|item| try_convert_hashable_pickle_to_string(item))
                .collect(),
        ),
        serde_pickle::value::HashableValue::FrozenSet(s) => {
            serde_pickle::value::HashableValue::FrozenSet(
                s.into_iter()
                    .map(|item| try_convert_hashable_pickle_to_string(item))
                    .collect(),
            )
        }
        value => value,
    }
}

fn try_convert_pickle_to_string(value: serde_pickle::value::Value) -> serde_pickle::value::Value {
    match value {
        serde_pickle::value::Value::Bytes(b) => {
            if let Ok(s) = std::str::from_utf8(&b) {
                serde_pickle::value::Value::String(s.to_owned())
            } else {
                serde_pickle::value::Value::Bytes(b)
            }
        }
        serde_pickle::value::Value::List(l) => serde_pickle::value::Value::List(
            l.into_iter()
                .map(|item| try_convert_pickle_to_string(item))
                .collect(),
        ),
        serde_pickle::value::Value::Tuple(t) => serde_pickle::value::Value::Tuple(
            t.into_iter()
                .map(|item| try_convert_pickle_to_string(item))
                .collect(),
        ),
        serde_pickle::value::Value::Set(s) => serde_pickle::value::Value::Set(
            s.into_iter()
                .map(|item| try_convert_hashable_pickle_to_string(item))
                .collect(),
        ),
        serde_pickle::value::Value::FrozenSet(s) => serde_pickle::value::Value::FrozenSet(
            s.into_iter()
                .map(|item| try_convert_hashable_pickle_to_string(item))
                .collect(),
        ),
        serde_pickle::value::Value::Dict(d) => serde_pickle::value::Value::Dict(
            d.into_iter()
                .map(|(k, v)| {
                    (
                        try_convert_hashable_pickle_to_string(k),
                        try_convert_pickle_to_string(v),
                    )
                })
                .collect(),
        ),
        value => value,
    }
}

fn parse_receive_common_cmd_blob(blob: &[u8]) -> IResult<&[u8], (VoiceLine, bool)> {
    let i = blob;
    let (i, line) = le_u16(i)?;
    let (i, audience) = le_u8(i)?;

    // if !matches!(line, 2 | 13 | 16 | 15 | 19) {
    //     panic!("{:#X?}", blob);
    // }

    let is_global = match audience {
        0 => false,
        1 => true,
        _ => {
            panic!("Got unknown audience {}", audience);
        }
    };
    let (i, message) = match line {
        1 => {
            let (i, x) = le_u16(i)?;
            let (i, y) = le_u16(i)?;
            (i, VoiceLine::AttentionToSquare(x as u32, y as u32))
        }
        2 => {
            let (i, target_type) = le_u16(i)?;
            let (i, target_id) = le_u64(i)?;
            (i, VoiceLine::QuickTactic(target_type, target_id))
        }
        3 => (i, VoiceLine::RequestingSupport(None)),
        // 4 is "QUICK_SOS"
        // 5 is AYE_AYE
        5 => (i, VoiceLine::Wilco),
        // 6 is NO_WAY
        6 => (i, VoiceLine::Negative),
        // GOOD_GAME
        7 => (i, VoiceLine::WellDone), // TODO: Find the corresponding field
        // GOOD_LUCK
        8 => (i, VoiceLine::FairWinds),
        // CARAMBA
        9 => (i, VoiceLine::Curses),
        // 10 -> THANK_YOU
        10 => (i, VoiceLine::DefendTheBase),
        // 11 -> NEED_AIR_DEFENSE
        11 => (i, VoiceLine::ProvideAntiAircraft),
        // BACK
        12 => {
            let (i, target_type) = le_u16(i)?;
            let (i, target_id) = le_u64(i)?;
            (
                i,
                VoiceLine::Retreat(if target_id != 0 {
                    Some(target_id as i32)
                } else {
                    None
                }),
            )
        }
        // NEED_VISION
        13 => (i, VoiceLine::IntelRequired),
        // NEED_SMOKE
        14 => (i, VoiceLine::SetSmokeScreen),
        // RLS
        15 => (i, VoiceLine::UsingRadar),
        // SONAR
        16 => (i, VoiceLine::UsingHydroSearch),
        // FOLLOW_ME
        17 => (i, VoiceLine::FollowMe),
        // MAP_POINT_ATTENTION
        18 => {
            let (i, x) = le_f32(i)?;
            let (i, y) = le_f32(i)?;
            (i, VoiceLine::MapPointAttention(x, y))
        }
        //  SUBMARINE_LOCATOR
        19 => (i, VoiceLine::UsingSubmarineLocator),
        line => {
            panic!("Unknown voice line {}, {:#X?}", line, i);
        }
    };

    Ok((i, (message, is_global)))
}

impl<'replay, 'argtype, 'rawpacket> DecodedPacketPayload<'replay, 'argtype, 'rawpacket>
where
    'rawpacket: 'replay,
    'rawpacket: 'argtype,
{
    fn from(
        version: &crate::version::Version,
        audit: bool,
        payload: &'rawpacket crate::packet2::PacketType<'replay, 'argtype>,
        packet_type: u32,
    ) -> Self {
        match payload {
            PacketType::EntityMethod(ref em) => {
                DecodedPacketPayload::from_entity_method(version, audit, em)
            }
            PacketType::Camera(camera) => DecodedPacketPayload::Camera(camera),
            PacketType::CameraMode(mode) => match mode {
                3 => DecodedPacketPayload::CameraMode(CameraMode::OverheadMap),
                5 => DecodedPacketPayload::CameraMode(CameraMode::FollowingShells),
                6 => DecodedPacketPayload::CameraMode(CameraMode::FollowingPlanes),
                8 => DecodedPacketPayload::CameraMode(CameraMode::FollowingShip),
                9 => DecodedPacketPayload::CameraMode(CameraMode::FreeFlying),
                11 => DecodedPacketPayload::CameraMode(CameraMode::FollowingSubmarine),
                _ => {
                    if audit {
                        DecodedPacketPayload::Audit(format!("CameraMode({})", mode))
                    } else {
                        DecodedPacketPayload::CameraMode(CameraMode::Unknown(*mode))
                    }
                }
            },
            PacketType::CameraFreeLook(freelook) => match freelook {
                0 => DecodedPacketPayload::CameraFreeLook(false),
                1 => DecodedPacketPayload::CameraFreeLook(true),
                _ => {
                    if audit {
                        DecodedPacketPayload::Audit(format!("CameraFreeLook({})", freelook))
                    } else {
                        DecodedPacketPayload::CameraFreeLook(true)
                    }
                }
            },
            PacketType::CruiseState(cs) => match cs.key {
                0 => DecodedPacketPayload::CruiseState {
                    state: CruiseState::Throttle,
                    value: cs.value,
                },
                1 => DecodedPacketPayload::CruiseState {
                    state: CruiseState::Rudder,
                    value: cs.value,
                },
                2 => DecodedPacketPayload::CruiseState {
                    state: CruiseState::DiveDepth,
                    value: cs.value,
                },
                _ => {
                    if audit {
                        DecodedPacketPayload::Audit(format!(
                            "CruiseState(unknown={}, {})",
                            cs.key, cs.value
                        ))
                    } else {
                        DecodedPacketPayload::CruiseState {
                            state: CruiseState::Unknown(cs.key),
                            value: cs.value,
                        }
                    }
                }
            },
            PacketType::Map(map) => {
                if audit && map.unknown != 0 && map.unknown != 1 {
                    DecodedPacketPayload::Audit(format!(
                        "Map: Unknown bool is not a bool (is {})",
                        map.unknown
                    ))
                } else if audit
                    && map.matrix
                        != [
                            0, 0, 128, 63, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                            128, 63, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 128, 63,
                            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 128, 63,
                        ]
                {
                    DecodedPacketPayload::Audit(format!(
                        "Map: Unit matrix is not a unit matrix (is {:?})",
                        map.matrix
                    ))
                } else {
                    DecodedPacketPayload::Map(map)
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
            PacketType::Version(version) => DecodedPacketPayload::Version(version.clone()),
            PacketType::Unknown(u) => {
                if packet_type == 0x18 {
                    if audit
                        && u != &[
                            00, 00, 00, 00, 00, 00, 00, 00, 00, 00, 00, 00, 00, 00, 00, 00, 00, 00,
                            00, 00, 00, 00, 00, 00, 00, 00, 00, 00, 00, 00, 00, 00, 00, 00, 00, 00,
                            00, 00, 00, 00, 00, 00, 0x80, 0xbf, 00, 00, 0x80, 0xbf, 00, 00, 0x80,
                            0xbf,
                        ]
                    {
                        DecodedPacketPayload::Audit(format!("Camera18 unexpected value!"))
                    } else {
                        DecodedPacketPayload::Unknown(&u)
                    }
                } else {
                    DecodedPacketPayload::Unknown(&u)
                }
            }
            PacketType::Invalid(u) => DecodedPacketPayload::Invalid(&u),
            PacketType::BattleResults(results) => DecodedPacketPayload::BattleResults(results),
        }
    }

    fn from_entity_method(
        version: &crate::version::Version,
        audit: bool,
        packet: &'rawpacket EntityMethodPacket<'argtype>,
    ) -> Self {
        let entity_id = &packet.entity_id;
        let method = &packet.method;
        let args = &packet.args;
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
            let mut extra_data = None;
            if *sender_id == 0 && args.len() >= 4 {
                let extra = serde_pickle::de::value_from_slice(
                    args[3].string_ref().expect("failed"),
                    serde_pickle::de::DeOptions::new(),
                )
                .expect("value is not pickled");
                let mut extra_dict: HashMap<String, Value> = HashMap::from_iter(
                    extra
                        .dict()
                        .expect("value is not a dictionary")
                        .into_iter()
                        .map(|(key, value)| {
                            let key = match key {
                                serde_pickle::HashableValue::Bytes(bytes) => {
                                    String::from_utf8(bytes)
                                        .expect("key is not a valid utf-8 sequence")
                                }
                                serde_pickle::HashableValue::String(string) => string,
                                other => {
                                    panic!("unexpected key type {:?}", other)
                                }
                            };

                            let value = match value {
                                Value::Bytes(bytes) => {
                                    if let Ok(result) = String::from_utf8(bytes.clone()) {
                                        Value::String(result)
                                    } else {
                                        Value::Bytes(bytes)
                                    }
                                }
                                other => other,
                            };

                            (key, value)
                        }),
                );

                let extra = ChatMessageExtra {
                    pre_battle_sign: extra_dict
                        .remove("preBattleSign")
                        .unwrap()
                        .i64()
                        .expect("preBattleSign is not an i64"),
                    pre_battle_id: extra_dict
                        .remove("preBattleId")
                        .unwrap()
                        .i64()
                        .expect("preBattleId is not an i64"),
                    player_clan_tag: extra_dict
                        .remove("playerClanTag")
                        .unwrap()
                        .string()
                        .expect("playerClanTag is not a string"),
                    typ: extra_dict
                        .remove("type")
                        .unwrap()
                        .i64()
                        .expect("type is not an i64"),
                    player_avatar_id: extra_dict
                        .remove("playerAvatarId")
                        .unwrap()
                        .i64()
                        .expect("playerAvatarId is not an i64"),
                    player_name: extra_dict
                        .remove("playerName")
                        .unwrap()
                        .string()
                        .expect("playerName is not a string"),
                };

                assert!(extra_dict.is_empty());

                extra_data = Some(extra);
            }
            DecodedPacketPayload::Chat {
                entity_id: *entity_id,
                sender_id: *sender_id,
                audience: std::str::from_utf8(&target).unwrap(),
                message: std::str::from_utf8(&message).unwrap(),
                extra_data,
            }
        } else if *method == "receive_CommonCMD" {
            let (sender_id, message, is_global) =
                if version.is_at_least(&crate::version::Version::from_client_exe("0,12,8,0")) {
                    let sender = *args[0]
                        .int_32_ref()
                        .expect("receive_CommonCMD: sender is not an i32");

                    let blob = args[1]
                        .blob_ref()
                        .expect("receive_CommonCMD: second argument is not a blob");

                    let (_reminader, (message_type, is_global)) =
                        parse_receive_common_cmd_blob(blob.as_ref())
                            .expect("receive_CommonCMD: failed to parse blob");

                    (sender, message_type, is_global)
                } else {
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
                        1 => VoiceLine::AttentionToSquare(a, b as u32),
                        2 => VoiceLine::QuickTactic(a as u16, b as u64),
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
                        17 => VoiceLine::FollowMe,
                        18 => VoiceLine::MapPointAttention(a as f32, b as f32),
                        19 => VoiceLine::UsingSubmarineLocator,
                        _ => {
                            panic!("Unknown voice line {} a={:x} b={:x}!", line, a, b);
                        }
                    };

                    (sender_id, message, is_global)
                };

            // let (audience, sender_id, line, a, b) = unpack_rpc_args!(args, u8, i32, u8, u32, u64);

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
            let value = try_convert_pickle_to_string(value);

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
                        .is_at_least(&crate::version::Version::from_client_exe("0,12,8,0"))
                    {
                        let mut h = HashMap::new();
                        h.insert("accountDBID", 0);
                        h.insert("antiAbuseEnabled", 1);
                        h.insert("avatarId", 2);
                        h.insert("camouflageInfo", 3);
                        h.insert("clanColor", 4);
                        h.insert("clanID", 5);
                        h.insert("clanTag", 6);
                        h.insert("crewParams", 7);
                        h.insert("dogTag", 8);
                        h.insert("fragsCount", 9);
                        h.insert("friendlyFireEnabled", 10);
                        h.insert("id", 11);
                        h.insert("invitationsEnabled", 12);
                        h.insert("isAbuser", 13);
                        h.insert("isAlive", 14);
                        h.insert("isBot", 15);
                        h.insert("isClientLoaded", 16);
                        h.insert("isConnected", 17);
                        h.insert("isHidden", 18);
                        h.insert("isLeaver", 19);
                        h.insert("isPreBattleOwner", 20);
                        h.insert("isTShooter", 21);
                        h.insert("keyTargetMarkers", 22);
                        h.insert("killedBuildingsCount", 23);
                        h.insert("maxHealth", 24);
                        h.insert("name", 25);
                        h.insert("playerMode", 26);
                        h.insert("preBattleIdOnStart", 27);
                        h.insert("preBattleSign", 28);
                        h.insert("prebattleId", 29);
                        h.insert("realm", 30);
                        h.insert("shipComponents", 31);
                        h.insert("shipConfigDump", 32);
                        h.insert("shipId", 33);
                        h.insert("shipParamsId", 34);
                        h.insert("skinId", 35);
                        h.insert("teamId", 36);
                        h.insert("ttkStatus", 37);
                        h
                    } else if version
                        .is_at_least(&crate::version::Version::from_client_exe("0,10,9,0"))
                    {
                        // 0.10.9 inserted things at 0x1 and 0x1F
                        let mut h = HashMap::new();
                        h.insert("avatarId", 0x2);
                        h.insert("clanTag", 0x6);
                        h.insert("maxHealth", 0x17);
                        h.insert("name", 0x18);
                        h.insert("shipId", 0x20);
                        h.insert("shipParamsId", 0x21);
                        h.insert("skinId", 0x22);
                        h.insert("teamId", 0x23);
                        h
                    } else if version
                        .is_at_least(&crate::version::Version::from_client_exe("0,10,7,0"))
                    {
                        // 0.10.7
                        let mut h = HashMap::new();
                        h.insert("avatarId", 0x1);
                        h.insert("clanTag", 0x5);
                        h.insert("maxHealth", 0x16);
                        h.insert("name", 0x17);
                        h.insert("shipId", 0x1e);
                        h.insert("shipParamsId", 0x1f);
                        h.insert("skinId", 0x20);
                        h.insert("teamId", 0x21);
                        h
                    } else {
                        // 0.10.6 and earlier
                        let mut h = HashMap::new();
                        h.insert("avatarId", 0x1);
                        h.insert("clanTag", 0x5);
                        h.insert("maxHealth", 0x15);
                        h.insert("name", 0x16);
                        h.insert("shipId", 0x1d);
                        h.insert("shipParamsId", 0x1e);
                        h.insert("skinId", 0x1f);
                        h.insert("teamId", 0x20);
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
                    let avatar = *values
                        .get(keys.get("avatarId").unwrap())
                        .unwrap()
                        .i64_ref()
                        .expect("avatarId is not an i64");

                    let username = values
                        .get(keys.get("name").unwrap())
                        .unwrap()
                        .string_ref()
                        .expect("name is not a string")
                        .clone();

                    let clan = values
                        .get(keys.get("clanTag").unwrap())
                        .unwrap()
                        .string_ref()
                        .expect("clanTag is not a string")
                        .clone();

                    let shipid = *values
                        .get(keys.get("shipId").unwrap())
                        .unwrap()
                        .i64_ref()
                        .expect("shipId is not an i64");
                    let meta_ship_id = *values
                        .get(keys.get("id").unwrap())
                        .unwrap()
                        .i64_ref()
                        .expect("shipId is not an i64");
                    let playerid = *values
                        .get(keys.get("shipParamsId").unwrap())
                        .unwrap()
                        .i64_ref()
                        .expect("shipParamsId is not an i64");
                    let _playeravatarid = *values
                        .get(keys.get("skinId").unwrap())
                        .unwrap()
                        .i64_ref()
                        .expect("skinId is not an i64");
                    let team = *values
                        .get(keys.get("teamId").unwrap())
                        .unwrap()
                        .i64_ref()
                        .expect("teamId is not an i64");
                    let health = *values
                        .get(keys.get("maxHealth").unwrap())
                        .unwrap()
                        .i64_ref()
                        .expect("maxHealth is not an i64");

                    let mut raw = HashMap::new();
                    for (k, v) in values.iter() {
                        raw.insert(*k, format!("{:?}", v));
                    }
                    players_out.push(OnArenaStateReceivedPlayer {
                        username,
                        clan,
                        avatar_id: avatar,
                        meta_ship_id,
                        entity_id: shipid,
                        team_id: team,
                        max_health: health,
                        raw,
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
                13 => DeathCause::DepthCharge,
                14 => DeathCause::AerialRocket,
                15 => DeathCause::Detonation,
                17 => DeathCause::Artillery,
                18 => DeathCause::Artillery,
                19 => DeathCause::Artillery,
                22 => DeathCause::SkipBombs,
                28 => DeathCause::DepthCharge, // TODO: Why is this different from the above depth charge?
                cause => {
                    if audit {
                        return DecodedPacketPayload::Audit(format!(
                            "receiveVehicleDeath(victim={}, killer={}, unknown cause {})",
                            victim, killer, cause
                        ));
                    } else {
                        DeathCause::Unknown(cause)
                    }
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
                31 => Ribbon::DepthChargeHit,
                33 => Ribbon::BuffSeized,
                39 => Ribbon::SonarOneHit,
                40 => Ribbon::SonarTwoHits,
                41 => Ribbon::SonarNeutralized,
                ribbon => {
                    if audit {
                        return DecodedPacketPayload::Audit(format!(
                            "onRibbon(unknown ribbon {})",
                            ribbon
                        ));
                    } else {
                        Ribbon::Unknown(ribbon)
                    }
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
                    aggressor: map.get("vehicleID").unwrap().try_into().unwrap(),
                    damage: map.get("damage").unwrap().try_into().unwrap(),
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

                let packed_data: u32 = minimap_update
                    .get("packedData")
                    .unwrap()
                    .try_into()
                    .unwrap();
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
            let (winning_team, unknown) =
                if version.is_at_least(&crate::version::Version::from_client_exe("0,12,8,0")) {
                    (None, None)
                } else {
                    let (winning_team, unknown) = unpack_rpc_args!(args, i8, u8);
                    (Some(winning_team), Some(unknown))
                };
            DecodedPacketPayload::BattleEnd {
                winning_team,
                unknown,
            }
        } else if *method == "consumableUsed" {
            let (consumable, duration) = unpack_rpc_args!(args, i8, f32);
            let raw_consumable = consumable;
            let consumable = match consumable {
                0 => Consumable::DamageControl,
                1 => Consumable::SpottingAircraft,
                2 => Consumable::DefensiveAntiAircraft,
                3 => Consumable::SpeedBoost,
                5 => Consumable::MainBatteryReloadBooster,
                7 => Consumable::Smoke,
                9 => Consumable::RepairParty,
                10 => Consumable::CatapultFighter,
                11 => Consumable::HydroacousticSearch,
                12 => Consumable::TorpedoReloadBooster,
                13 => Consumable::Radar,
                35 => Consumable::Hydrophone,
                36 => Consumable::EnhancedRudders,
                37 => Consumable::ReserveBattery,
                _ => {
                    if audit {
                        return DecodedPacketPayload::Audit(format!(
                            "consumableUsed({},{},{})",
                            entity_id, raw_consumable, duration
                        ));
                    } else {
                        Consumable::Unknown(consumable)
                    }
                }
            };
            DecodedPacketPayload::Consumable {
                entity: *entity_id,
                consumable: consumable,
                duration: duration,
            }
        } else {
            DecodedPacketPayload::EntityMethod(packet)
        }
    }
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
    pub fn from(
        version: &crate::version::Version,
        audit: bool,
        packet: &'rawpacket Packet<'_, '_>,
    ) -> Self {
        let decoded = Self {
            clock: packet.clock,
            packet_type: packet.packet_type,
            payload: DecodedPacketPayload::from(
                version,
                audit,
                &packet.payload,
                packet.packet_type,
            ),
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

impl AnalyzerMut for Decoder {
    fn finish(&mut self) {}

    fn process_mut(&mut self, packet: &Packet<'_, '_>) {
        let decoded = DecodedPacket::from(&self.version, false, packet);
        //println!("{:#?}", decoded);
        //println!("{}", serde_json::to_string_pretty(&decoded).unwrap());
        let encoded = serde_json::to_string(&decoded).unwrap();
        self.write(&encoded);
    }
}
