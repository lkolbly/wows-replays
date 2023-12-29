use std::{collections::HashMap, rc::Rc};

use derive_builder::Builder;
use serde::{Deserialize, Serialize};
use strum_macros::{EnumString, IntoStaticStr};
use variantly::Variantly;

#[derive(
    Serialize,
    Deserialize,
    EnumString,
    Clone,
    Debug,
    Variantly,
    IntoStaticStr,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
)]
pub enum Species {
    AAircraft,
    AbilitiesUnit,
    AirBase,
    AirCarrier,
    Airship,
    AntiAircraft,
    Artillery,
    ArtilleryUnit,
    Auxiliary,
    Battleship,
    Bomb,
    Bomber,
    BuildingType,
    Camoboost,
    Camouflage,
    Campaign,
    CoastalArtillery,
    CollectionAlbum,
    CollectionCard,
    Complex,
    Cruiser,
    DCharge,
    DeathSettings,
    DepthCharge,
    Destroyer,
    Dive,
    DiveBomberTypeUnit,
    DogTagDoll,
    DogTagItem,
    DogTagSlotsScheme,
    DogTagUnique,
    Drop,
    DropVisual,
    EngineUnit,
    Ensign,
    Event,
    Fake,
    Fighter,
    FighterTypeUnit,
    #[strum(serialize = "Fire control")]
    FireControl,
    Flags,
    FlightControlUnit,
    Generator,
    GlobalWeather,
    Globalboost,
    Hull,
    HullUnit,
    IndividualTask,
    Laser,
    LocalWeather,
    MSkin,
    Main,
    MapBorder,
    Military,
    Mine,
    Mission,
    Modifier,
    Multiboost,
    NewbieQuest,
    Operation,
    Permoflage,
    PlaneTracer,
    PrimaryWeaponsUnit,
    RayTower,
    Rocket,
    Scout,
    Search,
    Secondary,
    SecondaryWeaponsUnit,
    SensorTower,
    Sinking,
    Skin,
    Skip,
    SkipBomb,
    SkipBomberTypeUnit,
    SonarUnit,
    SpaceStation,
    Submarine,
    SuoUnit,
    Task,
    Torpedo,
    TorpedoBomberTypeUnit,
    TorpedoesUnit,
    Upgrade,
    Wave,
    #[strum(serialize = "null")]
    Null,
    Unknown(String),
}

impl Species {
    pub fn translation_id(&self) -> String {
        let name: &'static str = self.into();
        format!("IDS_{}", name)
    }
}

#[derive(Serialize, Deserialize, Builder, Debug, Clone)]
pub struct Param {
    id: u32,
    index: String,
    name: String,
    species: Option<Species>,
    nation: String,
    data: ParamData,
}

impl Param {
    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn index(&self) -> &str {
        self.index.as_ref()
    }

    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    pub fn species(&self) -> Option<Species> {
        self.species.clone()
    }

    pub fn nation(&self) -> &str {
        self.nation.as_ref()
    }

    pub fn data(&self) -> &ParamData {
        &self.data
    }
}

#[derive(Serialize, Deserialize, EnumString, Hash, Debug, Variantly)]
pub enum ParamType {
    Ability,
    Achievement,
    AdjustmentShotActivator,
    Aircraft,
    BattleScript,
    Building,
    Campaign,
    Catapult,
    ClanSupply,
    Collection,
    Component,
    Crew,
    Director,
    DogTag,
    EventTrigger,
    Exterior,
    Finder,
    Gun,
    Modernization,
    Other,
    Projectile,
    Radar,
    RageModeProgressAction,
    Reward,
    RibbonActivator,
    Sfx,
    Ship,
    SwitchTrigger,
    SwitchVehicleVisualStateAction,
    TimerActivator,
    ToggleTriggerAction,
    Unit,
    VisibilityChangedActivator,
}

// #[derive(Serialize, Deserialize, Clone, Builder, Debug)]
// pub struct VehicleAbility {
//     typ: String,

// }

#[derive(Serialize, Deserialize, Clone, Builder, Debug)]
pub struct Vehicle {
    level: u32,
    group: String,
    abilities: Vec<Vec<(String, String)>>,
}

impl Vehicle {
    pub fn level(&self) -> u32 {
        self.level
    }

    pub fn group(&self) -> &str {
        self.group.as_ref()
    }
}

#[derive(Serialize, Deserialize, Clone, Builder, Debug)]
pub struct AbilityCategory {
    special_sound_id: String,
    consumable_type: String,
    description_id: String,
    group: String,
    icon_id: String,
    num_consumables: isize,
    preparation_time: f32,
    reload_time: f32,
    title_id: String,
    work_time: f32,
}

#[derive(Serialize, Deserialize, Clone, Builder, Debug)]
pub struct Ability {
    can_buy: bool,
    cost_credits: isize,
    cost_gold: isize,
    is_free: bool,
    categories: HashMap<String, AbilityCategory>,
}

#[derive(Serialize, Deserialize, Clone, Builder, Debug)]
pub struct CrewPersonalityShips {
    groups: Vec<String>,
    nation: Vec<String>,
    peculiarity: Vec<String>,
    ships: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Builder, Debug)]
pub struct CrewPersonality {
    can_reset_skills_for_free: bool,
    cost_credits: usize,
    cost_elite_xp: usize,
    cost_gold: usize,
    cost_xp: usize,
    has_custom_background: bool,
    has_overlay: bool,
    has_rank: bool,
    has_sample_voiceover: bool,
    is_animated: bool,
    is_person: bool,
    is_retrainable: bool,
    is_unique: bool,
    peculiarity: String,
    /// TODO: flags?
    permissions: u32,
    person_name: String,
    ships: CrewPersonalityShips,
    subnation: String,
    tags: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Builder, Debug)]
pub struct ConsumableReloadTimeModifier {
    aircraft_carrier: f32,
    auxiliary: f32,
    battleship: f32,
    cruiser: f32,
    destroyer: f32,
    submarine: f32,
}

impl ConsumableReloadTimeModifier {
    pub fn get_for_species(&self, species: Species) -> f32 {
        match species {
            Species::AirCarrier => self.aircraft_carrier,
            Species::Battleship => self.battleship,
            Species::Cruiser => self.cruiser,
            Species::Destroyer => self.destroyer,
            Species::Submarine => self.submarine,
            Species::Auxiliary => self.auxiliary,
            other => panic!("Unexpected species {:?}", other),
        }
    }

    pub fn aircraft_carrier(&self) -> f32 {
        self.aircraft_carrier
    }

    pub fn auxiliary(&self) -> f32 {
        self.auxiliary
    }

    pub fn battleship(&self) -> f32 {
        self.battleship
    }

    pub fn cruiser(&self) -> f32 {
        self.cruiser
    }

    pub fn destroyer(&self) -> f32 {
        self.destroyer
    }

    pub fn submarine(&self) -> f32 {
        self.submarine
    }
}

#[derive(Serialize, Deserialize, Clone, Builder, Debug)]
pub struct CrewSkillModifier {
    name: String,
    aircraft_carrier: f32,
    auxiliary: f32,
    battleship: f32,
    cruiser: f32,
    destroyer: f32,
    submarine: f32,
}

#[derive(Serialize, Deserialize, Clone, Builder, Debug)]
pub struct CrewSkillLogicTrigger {
    /// Sometimes this field isn't present?
    burn_count: Option<usize>,
    change_priority_target_penalty: f32,
    consumable_type: String,
    cooling_delay: f32,
    /// TODO: figure out type
    cooling_interpolator: Vec<()>,
    divider_type: Option<String>,
    divider_value: Option<f32>,
    duration: f32,
    energy_coeff: f32,
    flood_count: Option<usize>,
    health_factor: Option<f32>,
    /// TODO: figure out type
    heat_interpolator: Vec<()>,
    modifiers: Option<Vec<CrewSkillModifier>>,
    trigger_desc_ids: String,
    trigger_type: String,
}

#[derive(Serialize, Deserialize, Clone, Builder, Debug)]
pub struct CrewSkillTiers {
    aircraft_carrier: usize,
    auxiliary: usize,
    battleship: usize,
    cruiser: usize,
    destroyer: usize,
    submarine: usize,
}

impl CrewSkillTiers {
    pub fn get_for_species(&self, species: Species) -> usize {
        match species {
            Species::AirCarrier => self.aircraft_carrier,
            Species::Battleship => self.battleship,
            Species::Cruiser => self.cruiser,
            Species::Destroyer => self.destroyer,
            Species::Submarine => self.submarine,
            Species::Auxiliary => self.auxiliary,
            other => panic!("Unexpected species {:?}", other),
        }
    }

    pub fn aircraft_carrier(&self) -> usize {
        self.aircraft_carrier
    }

    pub fn auxiliary(&self) -> usize {
        self.auxiliary
    }

    pub fn battleship(&self) -> usize {
        self.battleship
    }

    pub fn cruiser(&self) -> usize {
        self.cruiser
    }

    pub fn destroyer(&self) -> usize {
        self.destroyer
    }

    pub fn submarine(&self) -> usize {
        self.submarine
    }
}

#[derive(Serialize, Deserialize, Clone, Builder, Debug)]
pub struct CrewSkill {
    name: String,
    logic_trigger: CrewSkillLogicTrigger,
    can_be_learned: bool,
    is_epic: bool,
    modifiers: Option<Vec<CrewSkillModifier>>,
    skill_type: usize,
    tier: CrewSkillTiers,
    ui_treat_as_trigger: bool,
}

impl CrewSkill {
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    pub fn logic_trigger(&self) -> &CrewSkillLogicTrigger {
        &self.logic_trigger
    }

    pub fn can_be_learned(&self) -> bool {
        self.can_be_learned
    }

    pub fn is_epic(&self) -> bool {
        self.is_epic
    }

    pub fn modifiers(&self) -> Option<&Vec<CrewSkillModifier>> {
        self.modifiers.as_ref()
    }

    pub fn skill_type(&self) -> usize {
        self.skill_type
    }

    pub fn tier(&self) -> &CrewSkillTiers {
        &self.tier
    }

    pub fn ui_treat_as_trigger(&self) -> bool {
        self.ui_treat_as_trigger
    }
}

#[derive(Serialize, Deserialize, Clone, Builder, Debug)]
pub struct Crew {
    money_training_level: usize,
    personality: CrewPersonality,
    skills: Option<Vec<CrewSkill>>,
}

impl Crew {
    pub fn skill_by_type(&self, typ: u32) -> Option<&CrewSkill> {
        self.skills
            .as_ref()
            .and_then(|skills| skills.iter().find(|skill| skill.skill_type == typ as usize))
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Variantly)]
pub enum ParamData {
    Vehicle(Vehicle),
    Crew(Crew),
    Ability(Ability),
    Modernization,
    Exterior,
    Unit,
}

pub trait GameParamProvider {
    fn game_param_by_id(&self, id: u32) -> Option<Rc<Param>>;
    fn game_param_by_index(&self, index: &str) -> Option<Rc<Param>>;
    fn game_param_by_name(&self, name: &str) -> Option<Rc<Param>>;
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GameParams {
    params: Vec<Rc<Param>>,
    #[serde(skip)]
    id_to_params: HashMap<u32, Rc<Param>>,
    #[serde(skip)]
    index_to_params: HashMap<String, Rc<Param>>,
    #[serde(skip)]
    name_to_params: HashMap<String, Rc<Param>>,
}

impl GameParamProvider for GameParams {
    fn game_param_by_id(&self, id: u32) -> Option<Rc<Param>> {
        self.id_to_params.get(&id).cloned()
    }

    fn game_param_by_index(&self, index: &str) -> Option<Rc<Param>> {
        self.index_to_params.get(index).cloned()
    }

    fn game_param_by_name(&self, name: &str) -> Option<Rc<Param>> {
        self.name_to_params.get(name).cloned()
    }
}

fn build_param_lookups(
    params: &[Rc<Param>],
) -> (
    HashMap<u32, Rc<Param>>,
    HashMap<String, Rc<Param>>,
    HashMap<String, Rc<Param>>,
) {
    let mut id_to_params = HashMap::with_capacity(params.len());
    let mut index_to_params = HashMap::with_capacity(params.len());
    let mut name_to_params = HashMap::with_capacity(params.len());
    for param in params {
        id_to_params.insert(param.id, param.clone());
        index_to_params.insert(param.index.clone(), param.clone());
        name_to_params.insert(param.name.clone(), param.clone());
    }

    (id_to_params, index_to_params, name_to_params)
}

impl<I> From<I> for GameParams
where
    I: IntoIterator<Item = Param>,
{
    fn from(value: I) -> Self {
        let params: Vec<Rc<Param>> = value.into_iter().map(Rc::new).collect();
        let (id_to_params, index_to_params, name_to_params) = build_param_lookups(params.as_ref());

        Self {
            params,
            id_to_params,
            index_to_params,
            name_to_params,
        }
    }
}
