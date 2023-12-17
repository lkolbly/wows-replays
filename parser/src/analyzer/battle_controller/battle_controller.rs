use std::{cell::RefCell, collections::HashMap, rc::Rc};

use derive_builder::Builder;
use nom::{multi::count, number::complete::le_u32, sequence::pair};
use serde::{Deserialize, Serialize};

use crate::{
    analyzer::Analyzer,
    packet2::{Packet, PacketType, PacketTypeKind},
    resource_loader::{EntityType, ResourceLoader, Vehicle},
    rpc::typedefs::ArgValue,
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

#[derive(Debug)]
pub struct Skills {
    aircraft_carrier: Vec<u8>,
    battleship: Vec<u8>,
    cruiser: Vec<u8>,
    destroyer: Vec<u8>,
    auxiliary: Vec<u8>,
    submarine: Vec<u8>,
}

#[derive(Debug)]
pub struct ShipLoadout {
    config: ShipConfig,
    skills: Skills,
}

struct Player<'res> {
    name: String,
    relation: u32,
    vehicle: &'res Vehicle,
    loadout: Option<ShipLoadout>,
}

type SharedPlayer<'res> = Rc<RefCell<Player<'res>>>;
type MethodName = String;

pub struct BattleController<'res, G> {
    game_meta: ReplayMeta,
    game_resources: &'res G,
    players: Vec<SharedPlayer<'res>>,
    player_entities: HashMap<u32, SharedPlayer<'res>>,
    method_callbacks: HashMap<(EntityType, String), fn(&PacketType<'_, '_>)>,
    property_callbacks: HashMap<(EntityType, String), fn(&ArgValue<'_>)>,
}

impl<'res, G> BattleController<'res, G>
where
    G: ResourceLoader,
{
    pub fn new(game_meta: ReplayMeta, game_resources: &'res G) -> Self {
        let players: Vec<SharedPlayer<'res>> = game_meta
            .vehicles
            .iter()
            .map(|vehicle| {
                Rc::new(RefCell::new(Player {
                    name: vehicle.name.clone(),
                    relation: vehicle.relation,
                    vehicle: game_resources
                        .vehicle_by_id(vehicle.shipId)
                        .expect("could not find vehicle"),
                    loadout: None,
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

#[derive(Serialize, Deserialize)]
pub struct GameMessage {
    pub sender_relation: u32,
    pub sender_name: String,
    pub channel: ChatChannel,
    pub message: String,
}

impl<'res, G> Analyzer for BattleController<'res, G>
where
    G: ResourceLoader,
{
    fn process(&mut self, packet: &Packet<'_, '_>) {
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
            println!("\t {:#?}", packet);
            if packet.entity_type != "Vehicle" {
                return;
            }

            let config = if let Some(ArgValue::Blob(ship_config)) = packet.props.get("shipConfig") {
                let config =
                    parse_ship_config(ship_config.as_slice()).expect("failed to parse ship config");
                println!("{:#?}", config);

                config.1
            } else {
                panic!("ship config is not a blob")
            };

            let skills = if let Some(ArgValue::FixedDict(crew_modifiers)) =
                packet.props.get("crewModifiersCompactParams")
            {
                if let Some(ArgValue::Array(learned_skills)) = crew_modifiers.get("learnedSkills") {
                    let skills_from_idx = |idx: usize| -> Vec<u8> {
                        learned_skills[idx]
                            .array_ref()
                            .unwrap()
                            .iter()
                            .map(|idx| *(*idx).uint_8_ref().unwrap())
                            .collect()
                    };

                    Skills {
                        aircraft_carrier: skills_from_idx(0),
                        battleship: skills_from_idx(1),
                        cruiser: skills_from_idx(2),
                        destroyer: skills_from_idx(3),
                        auxiliary: skills_from_idx(4),
                        submarine: skills_from_idx(5),
                    }
                } else {
                    panic!("learnedSkills is not an array");
                }
            } else {
                panic!("crew modifiers is not a dictionary");
            };

            let loadout = ShipLoadout { config, skills };
            println!("{:#?}", loadout);
            self.player_entities
                .get(&packet.entity_id)
                .expect("failed to get player by entity id")
                .borrow_mut()
                .loadout = Some(loadout);
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
            println!("\t {}", packet.method);
            if packet.method == "onBattleEnd" {
                println!("{:?}", packet);
            }
            if packet.method == "onChatMessage" {
                // let sender = packet.args[0].clone().int_32().unwrap();
                // let mut sender_team = None;
                // let channel = std::str::from_utf8(packet.args[1].string_ref().unwrap()).unwrap();
                // let message = std::str::from_utf8(packet.args[2].string_ref().unwrap()).unwrap();

                // let channel = match channel {
                //     "battle_common" => ChatChannel::Global,
                //     "battle_team" => ChatChannel::Team,
                //     other => panic!("unknown channel {channel}"),
                // };

                // let mut sender_name = "Unknown".to_owned();
                // for player in &self.game_meta.vehicles {
                //     if player.id == (sender as i64) {
                //         sender_name = player.name.clone();
                //         sender_team = Some(player.relation);
                //     }
                // }

                // println!(
                //     "chat message from sender {sender_name} in channel {channel:?}: {message}"
                // );

                // self.replay_tab_state
                //     .lock()
                //     .unwrap()
                //     .game_chat
                //     .push(GameMessage {
                //         sender_relation: sender_team.unwrap(),
                //         sender_name,
                //         channel,
                //         message: message.to_string(),
                //     });
            }
        }
        if let PacketTypeKind::Invalid = packet.payload.kind() {
            println!("{:#?}", packet.payload);
        }
    }

    fn finish(&self) {}
}
