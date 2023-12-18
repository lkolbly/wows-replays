use std::{
    cell::{Ref, RefCell},
    collections::HashMap,
    rc::Rc,
};

use derive_builder::Builder;
use nom::{multi::count, number::complete::le_u32, sequence::pair};
use serde::{Deserialize, Serialize};

use crate::{
    analyzer::Analyzer,
    game_params::Param,
    packet2::{
        EntityCreatePacket, EntityMethodPacket, EntityPropertyPacket, Packet, PacketProcessor,
        PacketType, PacketTypeKind,
    },
    resource_loader::{ParamType, ResourceLoader, Vehicle},
    rpc::{entitydefs::EntitySpec, typedefs::ArgValue},
    IResult, ReplayMeta,
};

#[derive(Debug)]
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

#[derive(Debug)]
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

#[derive(Debug)]
pub struct Player<'res> {
    id: u32,
    name: String,
    relation: u32,
    vehicle: &'res Param,
    loadout: ShipLoadout,
}

impl<'res> Player<'res> {
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    pub fn relation(&self) -> u32 {
        self.relation
    }

    pub fn vehicle(&self) -> &Param {
        self.vehicle
    }

    pub fn loadout(&self) -> &ShipLoadout {
        &self.loadout
    }

    pub fn id(&self) -> u32 {
        self.id
    }
}

pub type SharedPlayer<'res> = Rc<RefCell<Player<'res>>>;
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

pub struct Entity {
    id: u32,
    spec: EntitySpec,
}

impl Entity {}

pub struct BattleController<'res, 'replay, G> {
    game_meta: &'replay ReplayMeta,
    game_resources: &'res G,
    players: Vec<SharedPlayer<'res>>,
    player_entities: HashMap<u32, SharedPlayer<'res>>,
    method_callbacks: HashMap<(ParamType, String), fn(&PacketType<'_, '_>)>,
    property_callbacks: HashMap<(ParamType, String), fn(&ArgValue<'_>)>,
    event_handler: Option<Rc<dyn EventHandler>>,
    game_chat: RefCell<Vec<GameMessage>>,
}

impl<'res, 'replay, G> BattleController<'res, 'replay, G>
where
    G: ResourceLoader,
{
    pub fn new(game_meta: &'replay ReplayMeta, game_resources: &'res G) -> Self {
        let players: Vec<SharedPlayer<'res>> = game_meta
            .vehicles
            .iter()
            .map(|vehicle| {
                Rc::new(RefCell::new(Player {
                    id: vehicle.id as u32,
                    name: vehicle.name.clone(),
                    relation: vehicle.relation,
                    vehicle: game_resources
                        .param_by_id(vehicle.shipId as u32)
                        .expect("could not find vehicle"),
                    loadout: Default::default(),
                }))
            })
            .collect();

        Self {
            game_meta,
            game_resources,
            players,
            player_entities: HashMap::default(),
            method_callbacks: Default::default(),
            property_callbacks: Default::default(),
            event_handler: None,
            game_chat: Default::default(),
        }
    }

    pub fn set_event_handler(&mut self, event_handler: Rc<dyn EventHandler>) {
        self.event_handler = Some(event_handler);
    }

    pub fn players(&self) -> &[SharedPlayer<'res>] {
        self.players.as_ref()
    }

    pub fn game_mode(&self) -> String {
        let id = format!("IDS_{}", self.game_meta.scenario.to_uppercase());
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

    fn handle_chat_message<'packet>(&self, entity_id: u32, args: &[ArgValue<'packet>]) {
        let sender = args[0].clone().int_32().unwrap();
        let mut sender_team = None;
        let channel = std::str::from_utf8(args[1].string_ref().unwrap()).unwrap();
        let message = std::str::from_utf8(args[2].string_ref().unwrap()).unwrap();

        let channel = match channel {
            "battle_common" => ChatChannel::Global,
            "battle_team" => ChatChannel::Team,
            other => panic!("unknown channel {}", other),
        };

        let mut sender_name = "Unknown".to_owned();
        for player in &self.game_meta.vehicles {
            if player.id == (sender as i64) {
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

        let mut chat = self.game_chat.borrow_mut();
        chat.push(message.clone());

        if let Some(event_handler) = self.event_handler.as_ref() {
            event_handler.on_chat_message(message);
        }
    }

    fn handle_entity_method<'packet>(&self, packet: &EntityMethodPacket<'packet>) {
        println!("\t {}", packet.method);

        match packet.method {
            "onChatMessage" => self.handle_chat_message(packet.entity_id, packet.args.as_slice()),
            other => println!("Unhandled packet method {other}"),
        }
    }

    fn handle_property_update<'packet>(&self, packet: &EntityPropertyPacket<'packet>) {}

    fn update_ship_config(&self, entity_id: u32, blob: &[u8]) {
        let (remainder, config) = parse_ship_config(blob).expect("failed to parse ship config");
        // assert!(remainder.is_empty());

        println!("{:#?}", config);

        // self.player_entities
        //     .get(&entity_id)
        //     .expect("failed to get player by entity id")
        //     .borrow_mut()
        //     .loadout
        //     .config = Some(config);
    }

    fn update_crew_modifiers<'packet>(
        &self,
        entity_id: u32,
        skills: &HashMap<&'packet str, ArgValue<'packet>>,
    ) {
        if let Some(ArgValue::Array(learned_skills)) = skills.get("learnedSkills") {
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

            // self.player_entities
            //     .get(&entity_id)
            //     .expect("failed to get player by entity id")
            //     .borrow_mut()
            //     .loadout
            //     .skills = Some(skills);
        } else {
            panic!("learnedSkills is not an array");
        }
    }

    fn update_property(&self, entity_id: u32, property_name: &str, value: &ArgValue) {
        match property_name {
            "shipConfig" => {
                self.update_ship_config(
                    entity_id,
                    value.blob_ref().expect("shipConfig is not a blob"),
                );
            }
            "crewModifiersCompactParams" => {
                self.update_crew_modifiers(
                    entity_id,
                    value
                        .fixed_dict_ref()
                        .expect("crewModifiersCompactParams is not a dict"),
                );
            }
            other => {
                println!("Unhandled property update: {other:?}")
            }
        }
    }

    fn handle_entity_create<'packet>(&self, packet: &EntityCreatePacket<'packet>) {
        println!("\t {:#?}", packet);
        if packet.entity_type != "Vehicle" {
            return;
        }

        for (prop, arg) in packet.props.iter() {
            self.update_property(packet.entity_id, prop, arg);
        }
    }

    pub fn game_chat(&self) -> Ref<[GameMessage]> {
        Ref::map(self.game_chat.borrow(), |r| r.as_slice())
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

impl<'res, 'replay, G> Analyzer for BattleController<'res, 'replay, G>
where
    G: ResourceLoader,
{
    fn process(&self, packet: &Packet<'_, '_>) {
        println!(
            "packet: {}, type: 0x{:x}, len: {}",
            packet.payload.kind(),
            packet.packet_type,
            packet.packet_size,
        );
        if !matches!(packet.payload.kind(), PacketTypeKind::Unknown) {
            println!("{:#?}", packet.payload);
        }

        if let PacketType::BattleResults(results) = &packet.payload {
            std::fs::write("battle_results.json", results);
        }
        if let PacketType::EntityCreate(packet) = &packet.payload {
            self.handle_entity_create(packet);
        }

        if let PacketType::BasePlayerCreate(packet) = &packet.payload {
            println!("\t {:?}", packet)
        }
        if let PacketType::CellPlayerCreate(packet) = &packet.payload {
            println!("\t {:?}", packet)
        }
        if let PacketType::PropertyUpdate(packet) = &packet.payload {
            println!("\t {:?}", packet)
        }
        if let PacketType::EntityProperty(packet) = &packet.payload {
            println!("\t {:?}", packet);
        }

        if let PacketType::EntityMethod(packet) = &packet.payload {
            self.handle_entity_method(packet);
        }
        if let PacketTypeKind::Invalid = packet.payload.kind() {
            println!("{:#?}", packet.payload);
        }
    }

    fn finish(&self) {}
}

impl<'res, 'replay, G> PacketProcessor for BattleController<'res, 'replay, G>
where
    G: ResourceLoader,
{
    fn process(&self, packet: Packet<'_, '_>) {
        Analyzer::process(self, &packet);
    }
}
