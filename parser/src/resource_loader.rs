use crate::Rc;

use crate::{game_params::Param, rpc::entitydefs::EntitySpec};

pub trait ResourceLoader {
    fn localized_name_from_param(&self, param: &Param) -> Option<&str>;
    fn localized_name_from_id(&self, id: &str) -> Option<String>;
    fn game_param_by_id(&self, id: u32) -> Option<Rc<Param>>;
    fn entity_specs(&self) -> &[EntitySpec];
}
