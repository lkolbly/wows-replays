use derive_builder::Builder;
use serde::{Deserialize, Serialize};
use strum_macros::EnumString;
use variantly::Variantly;

use crate::game_params::Param;

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

#[derive(Serialize, Deserialize, Clone, Builder, Debug)]
pub struct Vehicle {
    level: u32,
    group: String,
}

impl Vehicle {
    pub fn level(&self) -> u32 {
        self.level
    }

    pub fn group(&self) -> &str {
        self.group.as_ref()
    }
}

pub trait ResourceLoader {
    fn localized_name_from_param(&self, param: &Param) -> Option<&str>;
    fn localized_name_from_id(&self, id: &str) -> Option<&str>;
    fn param_by_id(&self, id: u32) -> Option<&Param>;
}
