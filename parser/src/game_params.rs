use std::{collections::HashMap, rc::Rc};

use derive_builder::Builder;
use serde::{Deserialize, Serialize};
use strum_macros::EnumString;

use crate::resource_loader::Vehicle;

#[derive(Serialize, Deserialize, EnumString, Clone, Debug)]
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

#[derive(Serialize, Deserialize, Builder, Debug)]
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ParamData {
    Vehicle(Vehicle),
}

pub trait GameParamProvider {
    fn by_id(&self, id: u32) -> Option<&Param>;
    fn by_index(&self, index: &str) -> Option<&Param>;
    fn by_name(&self, name: &str) -> Option<&Param>;
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
    fn by_id(&self, id: u32) -> Option<&Param> {
        self.id_to_params.get(&id).map(Rc::as_ref)
    }

    fn by_index(&self, index: &str) -> Option<&Param> {
        self.index_to_params.get(index).map(Rc::as_ref)
    }

    fn by_name(&self, name: &str) -> Option<&Param> {
        self.name_to_params.get(name).map(Rc::as_ref)
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
