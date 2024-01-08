use std::{
    borrow::Borrow,
    cell::{Ref, RefCell},
    collections::HashMap,
    str::FromStr,
    sync::atomic::AtomicUsize,
    time::Duration,
};

use derive_builder::Builder;
use nom::{multi::count, number::complete::le_u32, sequence::pair};
use pickled::{HashableValue, Value};
use serde::{Deserialize, Serialize};
use strum::ParseError;
use strum_macros::EnumString;
use variantly::Variantly;

static TIME_UNTIL_GAME_START: Duration = Duration::from_secs(30);

use crate::{
    analyzer::{
        analyzer::AnalyzerMut,
        decoder::{
            ChatMessageExtra, DamageReceived, DeathCause, DecodedPacket, DecoderBuilder,
            OnArenaStateReceivedPlayer,
        },
        Analyzer,
    },
    game_params::{CrewSkill, GameParamProvider, Param, ParamType, Vehicle},
    packet2::{
        EntityCreatePacket, EntityMethodPacket, EntityPropertyPacket, Packet, PacketProcessor,
        PacketProcessorMut, PacketType, PacketTypeKind,
    },
    resource_loader::{self, ResourceLoader},
    rpc::{entitydefs::EntitySpec, typedefs::ArgValue},
    version::Version,
    IResult, Rc, ReplayMeta,
};

#[derive(Debug, Default, Clone)]
pub struct ShipConfig {
    abilities: Vec<u32>,
    hull: u32,
    modernization: Vec<u32>,
    units: Vec<u32>,
    signals: Vec<u32>,
}

impl ShipConfig {
    pub fn signals(&self) -> &[u32] {
        self.signals.as_ref()
    }

    pub fn units(&self) -> &[u32] {
        self.units.as_ref()
    }

    pub fn modernization(&self) -> &[u32] {
        self.modernization.as_ref()
    }

    pub fn hull(&self) -> u32 {
        self.hull
    }

    pub fn abilities(&self) -> &[u32] {
        self.abilities.as_ref()
    }
}

#[derive(Debug, Default, Clone)]
pub struct Skills {
    aircraft_carrier: Vec<u8>,
    battleship: Vec<u8>,
    cruiser: Vec<u8>,
    destroyer: Vec<u8>,
    auxiliary: Vec<u8>,
    submarine: Vec<u8>,
}

impl Skills {
    pub fn submarine(&self) -> &[u8] {
        self.submarine.as_ref()
    }

    pub fn auxiliary(&self) -> &[u8] {
        self.auxiliary.as_ref()
    }

    pub fn destroyer(&self) -> &[u8] {
        self.destroyer.as_ref()
    }

    pub fn cruiser(&self) -> &[u8] {
        self.cruiser.as_ref()
    }

    pub fn battleship(&self) -> &[u8] {
        self.battleship.as_ref()
    }

    pub fn aircraft_carrier(&self) -> &[u8] {
        self.aircraft_carrier.as_ref()
    }
}

#[derive(Debug, Default)]
pub struct ShipLoadout {
    config: Option<ShipConfig>,
    skills: Option<Skills>,
}

impl ShipLoadout {
    pub fn skills(&self) -> Option<&Skills> {
        self.skills.as_ref()
    }

    pub fn config(&self) -> Option<&ShipConfig> {
        self.config.as_ref()
    }
}

#[derive(Debug, Serialize, Deserialize)]
/// Players that were received from parsing the replay packets
pub struct Player {
    name: String,
    clan: String,
    realm: String,
    db_id: i64,
    relation: u32,
    avatar_id: u32,
    ship_id: u32,
    entity_id: u32,
    team_id: u32,
    max_health: u32,
    vehicle: Rc<Param>,
}

impl Player {
    fn from_arena_player<G: ResourceLoader>(
        player: &OnArenaStateReceivedPlayer,
        metadata_player: &MetadataPlayer,
        resources: &G,
    ) -> Player {
        let OnArenaStateReceivedPlayer {
            username,
            clan,
            realm,
            db_id,
            avatar_id: avatarid,
            meta_ship_id: shipid,
            entity_id,
            team_id: teamid,
            max_health: health,
            raw,
        } = player;

        Player {
            name: username.clone(),
            clan: clan.clone(),
            realm: realm.clone(),
            db_id: *db_id,
            avatar_id: *avatarid as u32,
            ship_id: *shipid as u32,
            entity_id: *entity_id as u32,
            team_id: *teamid as u32,
            max_health: *health as u32,
            vehicle: resources
                .game_param_by_id(metadata_player.vehicle.id())
                .expect("could not find vehicle"),
            relation: metadata_player.relation,
        }
    }

    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    pub fn clan(&self) -> &str {
        self.clan.as_ref()
    }

    pub fn relation(&self) -> u32 {
        self.relation
    }

    pub fn avatar_id(&self) -> u32 {
        self.avatar_id
    }

    pub fn ship_id(&self) -> u32 {
        self.ship_id
    }

    pub fn entity_id(&self) -> u32 {
        self.entity_id
    }

    pub fn team_id(&self) -> u32 {
        self.team_id
    }

    pub fn max_health(&self) -> u32 {
        self.max_health
    }

    pub fn vehicle(&self) -> &Param {
        self.vehicle.as_ref()
    }

    pub fn realm(&self) -> &str {
        self.realm.as_ref()
    }

    pub fn db_id(&self) -> i64 {
        self.db_id
    }
}

#[derive(Debug)]
/// Players that were parsed from just the replay metadata
pub struct MetadataPlayer {
    id: u32,
    name: String,
    relation: u32,
    vehicle: Rc<Param>,
}

impl MetadataPlayer {
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    pub fn relation(&self) -> u32 {
        self.relation
    }

    pub fn vehicle(&self) -> &Param {
        self.vehicle.as_ref()
    }

    pub fn id(&self) -> u32 {
        self.id
    }
}

pub type SharedPlayer = Rc<MetadataPlayer>;
type MethodName = String;

pub trait EventHandler {
    fn on_chat_message(&self, message: GameMessage) {}
    fn on_aren_state_received(&self, entity_id: u32) {}
}

pub enum xEntityType {
    Client = 1,
    Cell = 2,
    Base = 4,
}

#[derive(Debug, Clone, Copy, EnumString)]
pub enum EntityType {
    Building,
    BattleEntity,
    BattleLogic,
    Vehicle,
    InteractiveZone,
    SmokeScreen,
}

pub struct BattleReport {
    self_entity: Rc<VehicleEntity>,
    version: Version,
    map_name: String,
    game_mode: String,
    game_type: String,
    match_group: String,
    player_entities: Vec<Rc<VehicleEntity>>,
    game_chat: Vec<GameMessage>,
}

impl BattleReport {
    pub fn self_entity(&self) -> Rc<VehicleEntity> {
        self.self_entity.clone()
    }

    pub fn player_entities(&self) -> &[Rc<VehicleEntity>] {
        self.player_entities.as_ref()
    }

    pub fn game_chat(&self) -> &[GameMessage] {
        self.game_chat.as_ref()
    }

    pub fn match_group(&self) -> &str {
        self.match_group.as_ref()
    }

    pub fn map_name(&self) -> &str {
        self.map_name.as_ref()
    }

    pub fn version(&self) -> Version {
        self.version
    }

    pub fn game_mode(&self) -> &str {
        self.game_mode.as_ref()
    }

    pub fn game_type(&self) -> &str {
        self.game_type.as_ref()
    }
}

type Id = u32;

struct DamageEvent {
    amount: f32,
    victim: Id,
}

pub struct BattleController<'res, 'replay, G> {
    game_meta: &'replay ReplayMeta,
    game_resources: &'res G,
    metadata_players: Vec<SharedPlayer>,
    player_entities: HashMap<Id, Rc<Player>>,
    entities_by_id: HashMap<Id, Entity>,
    method_callbacks: HashMap<(ParamType, String), fn(&PacketType<'_, '_>)>,
    property_callbacks: HashMap<(ParamType, String), fn(&ArgValue<'_>)>,
    damage_dealt: HashMap<u32, Vec<DamageEvent>>,
    frags: HashMap<u32, Vec<Death>>,
    event_handler: Option<Rc<dyn EventHandler>>,
    game_chat: Vec<GameMessage>,
    version: Version,
}

impl<'res, 'replay, G> BattleController<'res, 'replay, G>
where
    G: ResourceLoader,
{
    pub fn new(game_meta: &'replay ReplayMeta, game_resources: &'res G) -> Self {
        let players: Vec<SharedPlayer> = game_meta
            .vehicles
            .iter()
            .map(|vehicle| {
                Rc::new(MetadataPlayer {
                    id: vehicle.id as u32,
                    name: vehicle.name.clone(),
                    relation: vehicle.relation,
                    vehicle: game_resources
                        .game_param_by_id(vehicle.shipId as u32)
                        .expect("could not find vehicle"),
                })
            })
            .collect();

        Self {
            game_meta,
            game_resources,
            metadata_players: players,
            player_entities: HashMap::default(),
            entities_by_id: Default::default(),
            method_callbacks: Default::default(),
            property_callbacks: Default::default(),
            event_handler: None,
            game_chat: Default::default(),
            version: crate::version::Version::from_client_exe(&game_meta.clientVersionFromExe),
            damage_dealt: Default::default(),
            frags: Default::default(),
        }
    }

    pub fn set_event_handler(&mut self, event_handler: Rc<dyn EventHandler>) {
        self.event_handler = Some(event_handler);
    }

    pub fn players(&self) -> &[SharedPlayer] {
        self.metadata_players.as_ref()
    }

    pub fn game_mode(&self) -> String {
        let id = format!("IDS_SCENARIO_{}", self.game_meta.scenario.to_uppercase());
        self.game_resources
            .localized_name_from_id(&id)
            .unwrap_or_else(|| self.game_meta.scenario.clone())
    }

    pub fn map_name(&self) -> String {
        let id = format!("IDS_{}", self.game_meta.mapName.to_uppercase());
        self.game_resources
            .localized_name_from_id(&id)
            .unwrap_or_else(|| self.game_meta.mapName.clone())
    }

    pub fn player_name(&self) -> &str {
        self.game_meta.playerName.as_ref()
    }

    pub fn match_group(&self) -> &str {
        self.game_meta.matchGroup.as_ref()
    }

    pub fn game_version(&self) -> &str {
        self.game_meta.clientVersionFromExe.as_ref()
    }

    pub fn game_type(&self) -> String {
        let id = format!("IDS_{}", self.game_meta.gameType.to_ascii_uppercase());
        self.game_resources
            .localized_name_from_id(&id)
            .unwrap_or_else(|| self.game_meta.gameType.clone())
    }

    fn handle_chat_message<'packet>(
        &mut self,
        entity_id: u32,
        sender_id: i32,
        audience: &str,
        message: &str,
        extra_data: Option<ChatMessageExtra>,
    ) {
        if sender_id == 0 {
            return;
        }

        let channel = match audience {
            "battle_common" => ChatChannel::Global,
            "battle_team" => ChatChannel::Team,
            other => panic!("unknown channel {}", other),
        };

        let mut sender_team = None;
        let mut sender_name = "Unknown".to_owned();
        for player in &self.game_meta.vehicles {
            if player.id == (sender_id as i64) {
                sender_name = player.name.clone();
                sender_team = Some(player.relation);
            }
        }

        println!("chat message from sender {sender_name} in channel {channel:?}: {message}");

        let message = GameMessage {
            sender_relation: sender_team.unwrap(),
            sender_name,
            channel,
            message: message.to_string(),
        };

        self.game_chat.push(message.clone());

        if let Some(event_handler) = self.event_handler.as_ref() {
            event_handler.on_chat_message(message);
        }
    }

    fn handle_entity_create<'packet>(&mut self, packet: &EntityCreatePacket<'packet>) {
        let entity_type = EntityType::from_str(packet.entity_type).unwrap_or_else(|_| {
            panic!(
                "failed to convert entity type {} to a string",
                packet.entity_type
            );
        });

        if packet.entity_id == 831749 {
            panic!("{:#?}", packet);
        }

        match entity_type {
            EntityType::Vehicle => {
                let mut props = VehicleProps::default();
                props.update_from_args(&packet.props);

                let player = self.player_entities.get(&packet.entity_id);

                let captain_id = props.crew_modifiers_compact_params.params_id;
                let captain = if captain_id != 0 {
                    Some(
                        self.game_resources
                            .game_param_by_id(captain_id)
                            .expect("failed to get captain"),
                    )
                } else {
                    None
                };

                let vehicle = Rc::new(RefCell::new(VehicleEntity {
                    id: packet.entity_id,
                    player: player.cloned(),
                    props,
                    captain,
                    damage: 0.0,
                    death_info: None,
                }));

                self.entities_by_id
                    .insert(packet.entity_id, Entity::Vehicle(vehicle.clone()));
            }
            EntityType::BattleLogic => eprintln!("BattleLogic create"),
            EntityType::InteractiveZone => eprintln!("InteractiveZone create"),
            EntityType::SmokeScreen => eprintln!("SmokeScreen create"),
            EntityType::BattleEntity => eprintln!("BattleEntity create"),
            EntityType::Building => eprintln!("Building create"),
        }
    }

    pub fn game_chat(&self) -> &[GameMessage] {
        self.game_chat.as_slice()
    }

    pub fn build_report(mut self) -> BattleReport {
        for (aggressor, damage_events) in &self.damage_dealt {
            if let Some(aggressor_player) = self.entities_by_id.get_mut(&aggressor) {
                let vehicle = aggressor_player
                    .vehicle_ref()
                    .expect("aggressor has no vehicle?");

                let mut vehicle = vehicle.borrow_mut();
                vehicle.damage += damage_events.iter().fold(0.0, |mut accum, event| {
                    accum += event.amount;
                    accum
                });
            } else {
                // panic!("unknown aggressor {:?}?", *aggressor);
            }
        }

        self.entities_by_id.values().for_each(|entity| {
            if let Some(vehicle) = entity.vehicle_ref() {
                let mut vehicle = vehicle.borrow_mut();

                if let Some(death) = self
                    .frags
                    .values()
                    .find_map(|deaths| deaths.iter().find(|death| death.victim == vehicle.id))
                {
                    vehicle.death_info = Some(DeathInfo {
                        time_lived: death.timestamp - TIME_UNTIL_GAME_START,
                        killer: death.killer,
                        cause: death.cause,
                    })
                }
            }
        });

        let player_entity_ids: Vec<_> = self.player_entities.keys().cloned().collect();
        let player_entities: Vec<Rc<VehicleEntity>> = self
            .entities_by_id
            .iter()
            .filter_map(|(entity_id, entity)| {
                if player_entity_ids.contains(entity_id) {
                    let vehicle: VehicleEntity = RefCell::borrow(entity.vehicle_ref()?).clone();
                    Some(Rc::new(vehicle))
                } else {
                    None
                }
            })
            .collect();

        BattleReport {
            self_entity: player_entities
                .iter()
                .find(|entity| entity.player.as_ref().unwrap().relation == 0)
                .cloned()
                .expect("could not find self_player"),
            version: Version::from_client_exe(self.game_version()),
            match_group: self.match_group().to_owned(),
            map_name: self.map_name(),
            game_mode: self.game_mode(),
            game_type: self.game_type(),
            player_entities,
            game_chat: self.game_chat,
        }
    }
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum ChatChannel {
    Division,
    Global,
    Team,
}

fn parse_ship_config<'a>(blob: &'a [u8]) -> IResult<&'a [u8], ShipConfig> {
    let i = blob;
    let (i, _unk) = le_u32(i)?;

    let (i, ship_params_id) = le_u32(i)?;
    let (i, _unk2) = le_u32(i)?;

    let (i, unit_count) = le_u32(i)?;
    let (i, units) = count(le_u32, unit_count as usize)(i)?;

    let (i, modernization_count) = le_u32(i)?;
    let (i, modernization) = count(le_u32, modernization_count as usize)(i)?;

    let (i, signal_count) = le_u32(i)?;
    let (i, signals) = count(le_u32, signal_count as usize)(i)?;

    let (i, _supply_state) = le_u32(i)?;

    let (i, camo_info_count) = le_u32(i)?;
    // First item in pair is camo_info, second is camo_scheme
    let (i, _camo) = count(pair(le_u32, le_u32), camo_info_count as usize)(i)?;

    let (i, abilities_count) = le_u32(i)?;
    let (i, abilities) = count(le_u32, abilities_count as usize)(i)?;

    Ok((
        i,
        ShipConfig {
            abilities,
            hull: units[0],
            modernization,
            units,
            signals,
        },
    ))
}

#[derive(Serialize, Deserialize, Clone)]
pub struct GameMessage {
    pub sender_relation: u32,
    pub sender_name: String,
    pub channel: ChatChannel,
    pub message: String,
}

#[derive(Debug, Default, Clone)]
pub struct AAAura {
    id: u32,
    enabled: bool,
}

#[derive(Debug, Default, Clone)]
pub struct VehicleState {
    /// TODO
    buffs: Option<()>,
    vehicle_visual_state: u8,
    /// TODO
    battery: Option<()>,
}

#[derive(Debug, Default, Clone)]
pub struct CrewModifiersCompactParams {
    params_id: u32,
    is_in_adaption: bool,
    learned_skills: Skills,
}

trait UpdateFromReplayArgs {
    fn update_from_args(&mut self, args: &HashMap<&str, ArgValue<'_>>);
}

macro_rules! set_arg_value {
    ($set_var:expr, $args:ident, $key:expr, String) => {
        $set_var = (*value
            .string_ref()
            .unwrap_or_else(|| panic!("{} is not a string", $key)))
        .clone()
    };
    ($set_var:expr, $args:ident, $key:expr, i8) => {
        set_arg_value!($set_var, $args, $key, int_8_ref, i8)
    };
    ($set_var:expr, $args:ident, $key:expr, i16) => {
        set_arg_value!($set_var, $args, $key, int_16_ref, i16)
    };
    ($set_var:expr, $args:ident, $key:expr, i32) => {
        set_arg_value!($set_var, $args, $key, int_32_ref, i32)
    };
    ($set_var:expr, $args:ident, $key:expr, u8) => {
        set_arg_value!($set_var, $args, $key, uint_8_ref, u8)
    };
    ($set_var:expr, $args:ident, $key:expr, u16) => {
        set_arg_value!($set_var, $args, $key, uint_16_ref, u16)
    };
    ($set_var:expr, $args:ident, $key:expr, u32) => {
        set_arg_value!($set_var, $args, $key, uint_32_ref, u32)
    };
    ($set_var:expr, $args:ident, $key:expr, f32) => {
        set_arg_value!($set_var, $args, $key, float_32_ref, f32)
    };
    ($set_var:expr, $args:ident, $key:expr, bool) => {
        if let Some(value) = $args.get($key) {
            $set_var = (*value
                .uint_8_ref()
                .unwrap_or_else(|| panic!("{} is not a u8", $key)))
                != 0
        }
    };
    ($set_var:expr, $args:ident, $key:expr, Vec<u8>) => {
        if let Some(value) = $args.get($key) {
            $set_var = value
                .blob_ref()
                .unwrap_or_else(|| panic!("{} is not a u8", $key))
                .clone()
        }
    };
    ($set_var:expr, $args:ident, $key:expr, &[()]) => {
        set_arg_value!($set_var, $args, $key, array_ref, &[()])
    };
    ($set_var:expr, $args:ident, $key:expr, $conversion_func:ident, $ty:ty) => {
        if let Some(value) = $args.get($key) {
            $set_var = value
                .$conversion_func()
                .unwrap_or_else(|| panic!("{} is not a {}", $key, stringify!($ty)))
                .clone()
        }
    };
}

macro_rules! arg_value_to_type {
    ($args:ident, $key:expr, String) => {
        arg_value_to_type!($args, $key, string_ref, String).clone()
    };
    ($args:ident, $key:expr, i8) => {
        *arg_value_to_type!($args, $key, int_8_ref, i8)
    };
    ($args:ident, $key:expr, i16) => {
        *arg_value_to_type!($args, $key, int_16_ref, i16)
    };
    ($args:ident, $key:expr, i32) => {
        *arg_value_to_type!($args, $key, int_32_ref, i32)
    };
    ($args:ident, $key:expr, u8) => {
        *arg_value_to_type!($args, $key, uint_8_ref, u8)
    };
    ($args:ident, $key:expr, u16) => {
        *arg_value_to_type!($args, $key, uint_16_ref, u16)
    };
    ($args:ident, $key:expr, u32) => {
        *arg_value_to_type!($args, $key, uint_32_ref, u32)
    };
    ($args:ident, $key:expr, bool) => {
        (*arg_value_to_type!($args, $key, uint_8_ref, u8)) != 0
    };
    ($args:ident, $key:expr, &[()]) => {
        arg_value_to_type!($args, $key, array_ref, &[()])
    };
    ($args:ident, $key:expr, &[u8]) => {
        arg_value_to_type!($args, $key, blob_ref, &[()]).as_ref()
    };
    ($args:ident, $key:expr, HashMap<(), ()>) => {
        arg_value_to_type!($args, $key, fixed_dict_ref, HashMap<(), ()>)
    };
    ($args:ident, $key:expr, $conversion_func:ident, $ty:ty) => {
        $args
            .get($key)
            .unwrap_or_else(|| panic!("could not get {}", $key))
            .$conversion_func()
            .unwrap_or_else(|| panic!("{} is not a {}", $key, stringify!($ty)))
    };
}

impl UpdateFromReplayArgs for CrewModifiersCompactParams {
    fn update_from_args(&mut self, args: &HashMap<&str, ArgValue<'_>>) {
        const PARAMS_ID_KEY: &'static str = "paramsId";
        const IS_IN_ADAPTION_KEY: &'static str = "isInAdaption";
        const LEARNED_SKILLS_KEY: &'static str = "learnedSkills";

        if args.contains_key(PARAMS_ID_KEY) {
            self.params_id = arg_value_to_type!(args, PARAMS_ID_KEY, u32);
        }
        if args.contains_key(IS_IN_ADAPTION_KEY) {
            self.is_in_adaption = arg_value_to_type!(args, IS_IN_ADAPTION_KEY, bool);
        }

        if args.contains_key(LEARNED_SKILLS_KEY) {
            let learned_skills = arg_value_to_type!(args, LEARNED_SKILLS_KEY, &[()]);
            let skills_from_idx = |idx: usize| -> Vec<u8> {
                learned_skills[idx]
                    .array_ref()
                    .unwrap()
                    .iter()
                    .map(|idx| *(*idx).uint_8_ref().unwrap())
                    .collect()
            };

            let skills = Skills {
                aircraft_carrier: skills_from_idx(0),
                battleship: skills_from_idx(1),
                cruiser: skills_from_idx(2),
                destroyer: skills_from_idx(3),
                auxiliary: skills_from_idx(4),
                submarine: skills_from_idx(5),
            };

            self.learned_skills = skills;
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct VehicleProps {
    ignore_map_borders: bool,
    air_defense_dispersion_radius: f32,
    death_settings: Vec<u8>,
    owner: u32,
    atba_targets: Vec<u32>,
    effects: Vec<String>,
    crew_modifiers_compact_params: CrewModifiersCompactParams,
    laser_target_local_pos: u16,
    anti_air_auras: Vec<AAAura>,
    selected_weapon: u32,
    regeneration_health: f32,
    is_on_forsage: bool,
    is_in_rage_mode: bool,
    has_air_targets_in_range: bool,
    torpedo_local_pos: u16,
    /// TODO
    air_defense_target_ids: Vec<()>,
    buoyancy: f32,
    max_health: f32,
    rudders_angle: f32,
    draught: f32,
    target_local_pos: u16,
    triggered_skills_data: Vec<u8>,
    regenerated_health: f32,
    blocked_controls: u8,
    is_invisible: bool,
    is_fog_horn_on: bool,
    server_speed_raw: u16,
    regen_crew_hp_limit: f32,
    /// TODO
    miscs_presets_status: Vec<()>,
    buoyancy_current_waterline: f32,
    is_alive: bool,
    is_bot: bool,
    visibility_flags: u32,
    heat_infos: Vec<()>,
    buoyancy_rudder_index: u8,
    is_anti_air_mode: bool,
    speed_sign_dir: i8,
    oil_leak_state: u8,
    /// TODO
    sounds: Vec<()>,
    ship_config: ShipConfig,
    wave_local_pos: u16,
    has_active_main_squadron: bool,
    weapon_lock_flags: u16,
    deep_rudders_angle: f32,
    /// TODO
    debug_text: Vec<()>,
    health: f32,
    engine_dir: i8,
    state: VehicleState,
    team_id: i8,
    buoyancy_current_state: u8,
    ui_enabled: bool,
    respawn_time: u16,
    engine_power: u8,
    max_server_speed_raw: u32,
    burning_flags: u16,
}

impl VehicleProps {
    pub fn ignore_map_borders(&self) -> bool {
        self.ignore_map_borders
    }

    pub fn air_defense_dispersion_radius(&self) -> f32 {
        self.air_defense_dispersion_radius
    }

    pub fn death_settings(&self) -> &[u8] {
        self.death_settings.as_ref()
    }

    pub fn owner(&self) -> u32 {
        self.owner
    }

    pub fn atba_targets(&self) -> &[u32] {
        self.atba_targets.as_ref()
    }

    pub fn effects(&self) -> &[String] {
        self.effects.as_ref()
    }

    pub fn crew_modifiers_compact_params(&self) -> &CrewModifiersCompactParams {
        &self.crew_modifiers_compact_params
    }

    pub fn laser_target_local_pos(&self) -> u16 {
        self.laser_target_local_pos
    }

    pub fn anti_air_auras(&self) -> &[AAAura] {
        self.anti_air_auras.as_ref()
    }

    pub fn selected_weapon(&self) -> u32 {
        self.selected_weapon
    }

    pub fn regeneration_health(&self) -> f32 {
        self.regeneration_health
    }

    pub fn is_on_forsage(&self) -> bool {
        self.is_on_forsage
    }

    pub fn is_in_rage_mode(&self) -> bool {
        self.is_in_rage_mode
    }

    pub fn has_air_targets_in_range(&self) -> bool {
        self.has_air_targets_in_range
    }

    pub fn torpedo_local_pos(&self) -> u16 {
        self.torpedo_local_pos
    }

    pub fn air_defense_target_ids(&self) -> &[()] {
        self.air_defense_target_ids.as_ref()
    }

    pub fn buoyancy(&self) -> f32 {
        self.buoyancy
    }

    pub fn max_health(&self) -> f32 {
        self.max_health
    }

    pub fn rudders_angle(&self) -> f32 {
        self.rudders_angle
    }

    pub fn draught(&self) -> f32 {
        self.draught
    }

    pub fn target_local_pos(&self) -> u16 {
        self.target_local_pos
    }

    pub fn triggered_skills_data(&self) -> &[u8] {
        self.triggered_skills_data.as_ref()
    }

    pub fn regenerated_health(&self) -> f32 {
        self.regenerated_health
    }

    pub fn blocked_controls(&self) -> u8 {
        self.blocked_controls
    }

    pub fn is_invisible(&self) -> bool {
        self.is_invisible
    }

    pub fn is_fog_horn_on(&self) -> bool {
        self.is_fog_horn_on
    }

    pub fn server_speed_raw(&self) -> u16 {
        self.server_speed_raw
    }

    pub fn regen_crew_hp_limit(&self) -> f32 {
        self.regen_crew_hp_limit
    }

    pub fn miscs_presets_status(&self) -> &[()] {
        self.miscs_presets_status.as_ref()
    }

    pub fn buoyancy_current_waterline(&self) -> f32 {
        self.buoyancy_current_waterline
    }

    pub fn is_alive(&self) -> bool {
        self.is_alive
    }

    pub fn is_bot(&self) -> bool {
        self.is_bot
    }

    pub fn visibility_flags(&self) -> u32 {
        self.visibility_flags
    }

    pub fn heat_infos(&self) -> &[()] {
        self.heat_infos.as_ref()
    }

    pub fn buoyancy_rudder_index(&self) -> u8 {
        self.buoyancy_rudder_index
    }

    pub fn is_anti_air_mode(&self) -> bool {
        self.is_anti_air_mode
    }

    pub fn speed_sign_dir(&self) -> i8 {
        self.speed_sign_dir
    }

    pub fn oil_leak_state(&self) -> u8 {
        self.oil_leak_state
    }

    pub fn sounds(&self) -> &[()] {
        self.sounds.as_ref()
    }

    pub fn ship_config(&self) -> &ShipConfig {
        &self.ship_config
    }

    pub fn wave_local_pos(&self) -> u16 {
        self.wave_local_pos
    }

    pub fn has_active_main_squadron(&self) -> bool {
        self.has_active_main_squadron
    }

    pub fn weapon_lock_flags(&self) -> u16 {
        self.weapon_lock_flags
    }

    pub fn deep_rudders_angle(&self) -> f32 {
        self.deep_rudders_angle
    }

    pub fn debug_text(&self) -> &[()] {
        self.debug_text.as_ref()
    }

    pub fn health(&self) -> f32 {
        self.health
    }

    pub fn engine_dir(&self) -> i8 {
        self.engine_dir
    }

    pub fn state(&self) -> &VehicleState {
        &self.state
    }

    pub fn team_id(&self) -> i8 {
        self.team_id
    }

    pub fn buoyancy_current_state(&self) -> u8 {
        self.buoyancy_current_state
    }

    pub fn ui_enabled(&self) -> bool {
        self.ui_enabled
    }

    pub fn respawn_time(&self) -> u16 {
        self.respawn_time
    }

    pub fn engine_power(&self) -> u8 {
        self.engine_power
    }

    pub fn max_server_speed_raw(&self) -> u32 {
        self.max_server_speed_raw
    }

    pub fn burning_flags(&self) -> u16 {
        self.burning_flags
    }
}

impl UpdateFromReplayArgs for VehicleProps {
    fn update_from_args(&mut self, args: &HashMap<&str, ArgValue<'_>>) {
        const IGNORE_MAP_BORDERS_KEY: &'static str = "ignoreMapBorders";
        const AIR_DEFENSE_DISPERSION_RADIUS_KEY: &'static str = "airDefenseDispRadius";
        const DEATH_SETTINGS_KEY: &'static str = "deathSettings";
        const OWNER_KEY: &'static str = "owner";
        const ATBA_TARGETS_KEY: &'static str = "atbaTargets";
        const EFFECTS_KEY: &'static str = "effects";
        const CREW_MODIFIERS_COMPACT_PARAMS_KEY: &'static str = "crewModifiersCompactParams";
        const LASER_TARGET_LOCAL_POS_KEY: &'static str = "laserTargetLocalPos";
        const ANTI_AIR_AUROS_KEY: &'static str = "antiAirAuras";
        const SELECTED_WEAPON_KEY: &'static str = "selectedWeapon";
        const REGENERATION_HEALTH_KEY: &'static str = "regenerationHealth";
        const IS_ON_FORSAGE_KEY: &'static str = "isOnForsage";
        const IS_IN_RAGE_MODE_KEY: &'static str = "isInRageMode";
        const HAS_AIR_TARGETS_IN_RANGE_KEY: &'static str = "hasAirTargetsInRange";
        const TORPEDO_LOCAL_POS_KEY: &'static str = "torpedoLocalPos";
        const AIR_DEFENSE_TARGET_IDS_KEY: &'static str = "airDefenseTargetIds";
        const BUOYANCY_KEY: &'static str = "buoyancy";
        const MAX_HEALTH_KEY: &'static str = "maxHealth";
        const DRAUGHT_KEY: &'static str = "draught";
        const RUDDERS_ANGLE_KEY: &'static str = "ruddersAngle";
        const TARGET_LOCAL_POSITION_KEY: &'static str = "targetLocalPos";
        const TRIGGERED_SKILLS_DATA_KEY: &'static str = "triggeredSkillsData";
        const REGENERATED_HEALTH_KEY: &'static str = "regeneratedHealth";
        const BLOCKED_CONTROLS_KEY: &'static str = "blockedControls";
        const IS_INVISIBLE_KEY: &'static str = "isInvisible";
        const IS_FOG_HORN_ON_KEY: &'static str = "isFogHornOn";
        const SERVER_SPEED_RAW_KEY: &'static str = "serverSpeedRaw";
        const REGEN_CREW_HP_LIMIT_KEY: &'static str = "regenCrewHpLimit";
        const MISCS_PRESETS_STATUS_KEY: &'static str = "miscsPresetsStatus";
        const BUOYANCY_CURRENT_WATERLINE_KEY: &'static str = "buoyancyCurrentWaterline";
        const IS_ALIVE_KEY: &'static str = "isAlive";
        const IS_BOT_KEY: &'static str = "isBot";
        const VISIBILITY_FLAGS_KEY: &'static str = "visibilityFlags";
        const HEAT_INFOS_KEY: &'static str = "heatInfos";
        const BUOYANCY_RUDDER_INDEX_KEY: &'static str = "buoyancyRudderIndex";
        const IS_ANTI_AIR_MODE_KEY: &'static str = "isAntiAirMode";
        const SPEED_SIGN_DIR_KEY: &'static str = "speedSignDir";
        const OIL_LEAK_STATE_KEY: &'static str = "oilLeakState";
        const SOUNDS_KEY: &'static str = "sounds";
        const SHIP_CONFIG_KEY: &'static str = "shipConfig";
        const WAVE_LOCAL_POS_KEY: &'static str = "waveLocalPos";
        const HAS_ACTIVE_MAIN_SQUADRON_KEY: &'static str = "hasActiveMainSquadron";
        const WEAPON_LOCK_FLAGS_KEY: &'static str = "weaponLockFlags";
        const DEEP_RUDDERS_ANGLE_KEY: &'static str = "deepRuddersAngle";
        const DEBUG_TEXT_KEY: &'static str = "debugText";
        const HEALTH_KEY: &'static str = "health";
        const ENGINE_DIR_KEY: &'static str = "engineDir";
        const STATE_KEY: &'static str = "state";
        const TEAM_ID_KEY: &'static str = "teamId";
        const BUOYANCY_CURRENT_STATE_KEY: &'static str = "buoyancyCurrentState";
        const UI_ENABLED_KEY: &'static str = "uiEnabled";
        const RESPAWN_TIME_KEY: &'static str = "respawnTime";
        const ENGINE_POWER_KEY: &'static str = "enginePower";
        const MAX_SERVER_SPEED_RAW_KEY: &'static str = "maxServerSpeedRaw";
        const BURNING_FLAGS_KEY: &'static str = "burningFlags";

        set_arg_value!(self.ignore_map_borders, args, IGNORE_MAP_BORDERS_KEY, bool);
        set_arg_value!(
            self.air_defense_dispersion_radius,
            args,
            AIR_DEFENSE_DISPERSION_RADIUS_KEY,
            f32
        );

        set_arg_value!(self.death_settings, args, DEATH_SETTINGS_KEY, Vec<u8>);
        if args.contains_key(OWNER_KEY) {
            let value: u32 = arg_value_to_type!(args, OWNER_KEY, i32) as u32;
            self.owner = value;
        }

        if args.contains_key(ATBA_TARGETS_KEY) {
            let value: Vec<u32> = arg_value_to_type!(args, ATBA_TARGETS_KEY, &[()])
                .iter()
                .map(|elem| {
                    elem.uint_32_ref()
                        .expect("atbaTargets elem is not a u32")
                        .clone()
                })
                .collect();
            self.atba_targets = value;
        }

        if args.contains_key(EFFECTS_KEY) {
            let value: Vec<String> = arg_value_to_type!(args, EFFECTS_KEY, &[()])
                .iter()
                .map(|elem| {
                    String::from_utf8(
                        elem.string_ref()
                            .expect("effects elem is not a string")
                            .clone(),
                    )
                    .expect("could not convert effects elem to string")
                })
                .collect();
            self.effects = value;
        }

        if args.contains_key(CREW_MODIFIERS_COMPACT_PARAMS_KEY) {
            self.crew_modifiers_compact_params.update_from_args(
                arg_value_to_type!(args, CREW_MODIFIERS_COMPACT_PARAMS_KEY, HashMap<(), ()>),
            );
        }

        set_arg_value!(
            self.laser_target_local_pos,
            args,
            LASER_TARGET_LOCAL_POS_KEY,
            u16
        );

        // TODO: AntiAirAuras
        set_arg_value!(self.selected_weapon, args, SELECTED_WEAPON_KEY, u32);

        set_arg_value!(self.is_on_forsage, args, IS_ON_FORSAGE_KEY, bool);

        set_arg_value!(self.is_in_rage_mode, args, IS_IN_RAGE_MODE_KEY, bool);

        set_arg_value!(
            self.has_air_targets_in_range,
            args,
            HAS_AIR_TARGETS_IN_RANGE_KEY,
            bool
        );

        set_arg_value!(self.torpedo_local_pos, args, TORPEDO_LOCAL_POS_KEY, u16);

        // TODO: airDefenseTargetIds

        set_arg_value!(self.buoyancy, args, BUOYANCY_KEY, f32);

        set_arg_value!(self.max_health, args, MAX_HEALTH_KEY, f32);

        set_arg_value!(self.draught, args, DRAUGHT_KEY, f32);

        set_arg_value!(self.rudders_angle, args, RUDDERS_ANGLE_KEY, f32);

        set_arg_value!(self.target_local_pos, args, TARGET_LOCAL_POSITION_KEY, u16);

        set_arg_value!(
            self.triggered_skills_data,
            args,
            TRIGGERED_SKILLS_DATA_KEY,
            Vec<u8>
        );

        set_arg_value!(self.regenerated_health, args, REGENERATED_HEALTH_KEY, f32);

        set_arg_value!(self.blocked_controls, args, BLOCKED_CONTROLS_KEY, u8);

        set_arg_value!(self.is_invisible, args, IS_INVISIBLE_KEY, bool);

        set_arg_value!(self.is_fog_horn_on, args, IS_FOG_HORN_ON_KEY, bool);

        set_arg_value!(self.server_speed_raw, args, SERVER_SPEED_RAW_KEY, u16);

        set_arg_value!(self.regen_crew_hp_limit, args, REGEN_CREW_HP_LIMIT_KEY, f32);

        // TODO: miscs_presets_status

        set_arg_value!(
            self.buoyancy_current_waterline,
            args,
            BUOYANCY_CURRENT_WATERLINE_KEY,
            f32
        );
        set_arg_value!(self.is_alive, args, IS_ALIVE_KEY, bool);
        set_arg_value!(self.is_bot, args, IS_BOT_KEY, bool);
        set_arg_value!(self.visibility_flags, args, VISIBILITY_FLAGS_KEY, u32);

        // TODO: heatInfos

        set_arg_value!(
            self.buoyancy_rudder_index,
            args,
            BUOYANCY_RUDDER_INDEX_KEY,
            u8
        );
        set_arg_value!(self.is_anti_air_mode, args, IS_ANTI_AIR_MODE_KEY, bool);
        set_arg_value!(self.speed_sign_dir, args, SPEED_SIGN_DIR_KEY, i8);
        set_arg_value!(self.oil_leak_state, args, OIL_LEAK_STATE_KEY, u8);

        // TODO: sounds

        if args.contains_key(SHIP_CONFIG_KEY) {
            let (_remainder, ship_config) =
                parse_ship_config(arg_value_to_type!(args, SHIP_CONFIG_KEY, &[u8]))
                    .expect("failed to parse ship config");

            self.ship_config = ship_config;
        }

        set_arg_value!(self.wave_local_pos, args, WAVE_LOCAL_POS_KEY, u16);
        set_arg_value!(
            self.has_active_main_squadron,
            args,
            HAS_ACTIVE_MAIN_SQUADRON_KEY,
            bool
        );
        set_arg_value!(self.weapon_lock_flags, args, WEAPON_LOCK_FLAGS_KEY, u16);
        set_arg_value!(self.deep_rudders_angle, args, DEEP_RUDDERS_ANGLE_KEY, f32);

        // TODO: debugText

        set_arg_value!(self.health, args, HEALTH_KEY, f32);
        set_arg_value!(self.engine_dir, args, ENGINE_DIR_KEY, i8);

        // TODO: state

        set_arg_value!(self.team_id, args, TEAM_ID_KEY, i8);
        set_arg_value!(
            self.buoyancy_current_state,
            args,
            BUOYANCY_CURRENT_STATE_KEY,
            u8
        );
        set_arg_value!(self.ui_enabled, args, UI_ENABLED_KEY, bool);
        set_arg_value!(self.respawn_time, args, RESPAWN_TIME_KEY, u16);
        set_arg_value!(self.engine_power, args, ENGINE_POWER_KEY, u8);
        set_arg_value!(
            self.max_server_speed_raw,
            args,
            MAX_SERVER_SPEED_RAW_KEY,
            u32
        );
        set_arg_value!(self.burning_flags, args, BURNING_FLAGS_KEY, u16);
    }
}

#[derive(Debug, Clone)]
pub struct DeathInfo {
    time_lived: Duration,
    killer: u32,
    cause: DeathCause,
}

impl DeathInfo {
    pub fn time_lived(&self) -> Duration {
        self.time_lived
    }

    pub fn killer(&self) -> u32 {
        self.killer
    }

    pub fn cause(&self) -> DeathCause {
        self.cause
    }
}

#[derive(Debug, Clone)]
pub struct VehicleEntity {
    id: u32,
    player: Option<Rc<Player>>,
    props: VehicleProps,
    captain: Option<Rc<Param>>,
    damage: f32,
    death_info: Option<DeathInfo>,
}

impl VehicleEntity {
    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn player(&self) -> Option<&Rc<Player>> {
        self.player.as_ref()
    }

    pub fn props(&self) -> &VehicleProps {
        &self.props
    }

    pub fn commander_id(&self) -> Id {
        self.props.crew_modifiers_compact_params.params_id
    }

    pub fn commander_skills(&self) -> Option<Vec<&CrewSkill>> {
        let vehicle_species = self
            .player
            .as_ref()
            .expect("player has not yet loaded")
            .vehicle
            .species()
            .expect("vehicle species not set");

        let skills = &self.props.crew_modifiers_compact_params.learned_skills;
        let skills_for_species = match vehicle_species {
            crate::game_params::Species::AirCarrier => skills.aircraft_carrier.as_slice(),
            crate::game_params::Species::Battleship => skills.battleship.as_slice(),
            crate::game_params::Species::Cruiser => skills.cruiser.as_slice(),
            crate::game_params::Species::Destroyer => skills.destroyer.as_slice(),
            crate::game_params::Species::Submarine => skills.submarine.as_slice(),
            other => {
                panic!("Unexpected vehicle species: {:?}", other);
            }
        };

        let captain = self
            .captain()?
            .data()
            .crew_ref()
            .expect("captain is not a crew?");

        let skills = skills_for_species
            .iter()
            .map(|skill_type| {
                captain
                    .skill_by_type(*skill_type as u32)
                    .expect("could not get skill type")
            })
            .collect();

        Some(skills)
    }

    pub fn commander_skills_raw(&self) -> &[u8] {
        let vehicle_species = self
            .player
            .as_ref()
            .expect("player has not yet loaded")
            .vehicle
            .species()
            .expect("vehicle species not set");

        let skills = &self.props.crew_modifiers_compact_params.learned_skills;
        match vehicle_species {
            crate::game_params::Species::AirCarrier => skills.aircraft_carrier.as_slice(),
            crate::game_params::Species::Battleship => skills.battleship.as_slice(),
            crate::game_params::Species::Cruiser => skills.cruiser.as_slice(),
            crate::game_params::Species::Destroyer => skills.destroyer.as_slice(),
            crate::game_params::Species::Submarine => skills.submarine.as_slice(),
            other => {
                panic!("Unexpected vehicle species: {:?}", other);
            }
        }
    }

    pub fn captain(&self) -> Option<&Param> {
        self.captain.as_ref().map(|rc| rc.as_ref())
    }

    pub fn damage(&self) -> f32 {
        self.damage
    }

    pub fn death_info(&self) -> Option<&DeathInfo> {
        self.death_info.as_ref()
    }
}

#[derive(Debug, Variantly)]
pub enum Entity {
    Vehicle(Rc<RefCell<VehicleEntity>>),
}

impl Entity {
    fn update_arena_player(&self, arena_player: Rc<Player>) {
        match self {
            Entity::Vehicle(vehicle) => {
                RefCell::borrow_mut(&*vehicle).player = Some(arena_player);
            }
        }
    }
}

#[derive(Debug)]
struct Death {
    timestamp: Duration,
    killer: u32,
    victim: u32,
    cause: DeathCause,
}

impl<'res, 'replay, G> AnalyzerMut for BattleController<'res, 'replay, G>
where
    G: ResourceLoader,
{
    fn process_mut(&mut self, packet: &Packet<'_, '_>) {
        let decoded = DecodedPacket::from(&self.version, false, packet);
        match decoded.payload {
            crate::analyzer::decoder::DecodedPacketPayload::Chat {
                entity_id,
                sender_id,
                audience,
                message,
                extra_data,
            } => {
                self.handle_chat_message(entity_id, sender_id, audience, message, extra_data);
            }
            crate::analyzer::decoder::DecodedPacketPayload::VoiceLine {
                sender_id,
                is_global,
                message,
            } => {
                eprintln!("HANDLE VOICE LINE");
            }
            crate::analyzer::decoder::DecodedPacketPayload::Ribbon(_) => {
                eprintln!("HANDLE RIBBON")
            }
            crate::analyzer::decoder::DecodedPacketPayload::Position(_) => {
                eprintln!("HANDLE POSITION")
            }
            crate::analyzer::decoder::DecodedPacketPayload::PlayerOrientation(_) => {
                eprintln!("PLAYER ORIENTATION")
            }
            crate::analyzer::decoder::DecodedPacketPayload::DamageStat(_) => {
                eprintln!("DAMAGE STAT")
            }
            crate::analyzer::decoder::DecodedPacketPayload::ShipDestroyed {
                killer,
                victim,
                cause,
            } => {
                self.frags.entry(killer as u32).or_default().push(Death {
                    timestamp: Duration::from_secs_f32(packet.clock),
                    killer: killer as u32,
                    victim: victim as u32,
                    cause,
                });
            }
            crate::analyzer::decoder::DecodedPacketPayload::EntityMethod(_) => {
                eprintln!("ENTITY METHOD")
            }
            crate::analyzer::decoder::DecodedPacketPayload::EntityProperty(_) => {
                eprintln!("ENTITY METHOD")
            }
            crate::analyzer::decoder::DecodedPacketPayload::BasePlayerCreate(base) => {
                eprintln!("BASE PLAYER CREATE");
            }
            crate::analyzer::decoder::DecodedPacketPayload::CellPlayerCreate(cell) => {
                // let metadata_player = self
                //     .metadata_players
                //     .iter()
                //     .find(|meta_player| meta_player.id == cell.vehicle_id as u32)
                //     .expect("could not map arena player to metadata player");
                // let battle_player = Player::from_arena_player(
                //     player,
                //     metadata_player.as_ref(),
                //     self.game_resources,
                // );

                // self.player_entities
                //     .insert(battle_player.entity_id, Rc::new(battle_player));
                eprintln!("CELL PLAYER CREATE");
            }
            crate::analyzer::decoder::DecodedPacketPayload::EntityEnter(e) => {
                eprintln!("ENTITY ENTER")
            }
            crate::analyzer::decoder::DecodedPacketPayload::EntityLeave(_) => {
                eprintln!("ENTITY LEAVE")
            }
            crate::analyzer::decoder::DecodedPacketPayload::EntityCreate(entity_create) => {
                self.handle_entity_create(entity_create);
            }
            crate::analyzer::decoder::DecodedPacketPayload::OnArenaStateReceived {
                arg0,
                arg1,
                arg2,
                players,
            } => {
                for player in &players {
                    let metadata_player = self
                        .metadata_players
                        .iter()
                        .find(|meta_player| meta_player.id == player.meta_ship_id as u32)
                        .expect("could not map arena player to metadata player");
                    let battle_player = Rc::new(Player::from_arena_player(
                        player,
                        metadata_player.as_ref(),
                        self.game_resources,
                    ));

                    self.player_entities
                        .insert(battle_player.entity_id, battle_player.clone());

                    if let Some(entity) = self.entities_by_id.get(&battle_player.entity_id) {
                        entity.update_arena_player(battle_player);
                    }
                }
            }
            crate::analyzer::decoder::DecodedPacketPayload::CheckPing(_) => eprintln!("CHECK PING"),
            crate::analyzer::decoder::DecodedPacketPayload::DamageReceived {
                victim,
                aggressors,
            } => {
                for damage in aggressors {
                    self.damage_dealt
                        .entry(damage.aggressor as u32)
                        .or_default()
                        .push(DamageEvent {
                            amount: damage.damage,
                            victim,
                        });
                }
            }
            crate::analyzer::decoder::DecodedPacketPayload::MinimapUpdate { updates, arg1 } => {
                eprintln!("MINIMAP UPDATE")
            }
            crate::analyzer::decoder::DecodedPacketPayload::PropertyUpdate(update) => {
                if let Some(entity) = self.entities_by_id.get(&(update.entity_id as u32)) {
                    //panic!("{:#?}", update);
                }
            }
            crate::analyzer::decoder::DecodedPacketPayload::BattleEnd {
                winning_team,
                unknown,
            } => eprintln!("BATTLE END"),
            crate::analyzer::decoder::DecodedPacketPayload::Consumable {
                entity,
                consumable,
                duration,
            } => eprintln!("CONSUMABLE"),
            crate::analyzer::decoder::DecodedPacketPayload::CruiseState { state, value } => {
                eprintln!("CRUISE STATE")
            }
            crate::analyzer::decoder::DecodedPacketPayload::Map(_) => eprintln!("MAP"),
            crate::analyzer::decoder::DecodedPacketPayload::Version(_) => eprintln!("VERSION"),
            crate::analyzer::decoder::DecodedPacketPayload::Camera(_) => eprintln!("CAMERA"),
            crate::analyzer::decoder::DecodedPacketPayload::CameraMode(_) => {
                eprintln!("CAMERA MODE")
            }
            crate::analyzer::decoder::DecodedPacketPayload::CameraFreeLook(_) => {
                eprintln!("CAMERA FREE LOOK")
            }
            crate::analyzer::decoder::DecodedPacketPayload::Unknown(_) => eprintln!("UNKNOWN"),
            crate::analyzer::decoder::DecodedPacketPayload::Invalid(_) => eprintln!("INVALID"),
            crate::analyzer::decoder::DecodedPacketPayload::Audit(_) => eprintln!("AUDIT"),
            crate::analyzer::decoder::DecodedPacketPayload::BattleResults(_) => {
                eprintln!("BATTLE RESULTS")
            }
        }
    }

    fn finish(&mut self) {}
}

impl<'res, 'replay, G> PacketProcessorMut for BattleController<'res, 'replay, G>
where
    G: ResourceLoader,
{
    fn process_mut(&mut self, packet: Packet<'_, '_>) {
        AnalyzerMut::process_mut(self, &packet);
    }
}
