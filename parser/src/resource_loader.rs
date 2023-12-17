use strum_macros::EnumString;

use crate::game_params::Param;

#[derive(EnumString, Hash)]
pub enum EntityType {
    Avatar,
    BattleLogic,
    Building,
    Vehicle,
}

pub struct Vehicle;

pub trait ResourceLoader {
    fn localized_name(&self, param: &Param) -> Option<String>;
    fn vehicle_by_id(&self, id: u64) -> Option<&Vehicle>;
}
