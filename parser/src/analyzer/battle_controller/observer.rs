use crate::{analyzer::decoder::DecodedPacket, resource_loader::ResourceLoader};

use super::BattleController;

trait BattleObserver {
    fn on_tick<G: ResourceLoader>(
        &mut self,
        controller: &BattleController<'_, '_, G>,
        event: &DecodedPacket,
    );
}
