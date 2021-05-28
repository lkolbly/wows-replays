use crate::analyzer::*;
use crate::packet2::{EntityMethodPacket, Packet, PacketType};

pub struct SummaryBuilder;

impl SummaryBuilder {
    pub fn new() -> Self {
        Self
    }
}

impl AnalyzerBuilder for SummaryBuilder {
    fn build(&self, meta: &crate::ReplayMeta) -> Box<dyn Analyzer> {
        Box::new(Summary {
            meta: Some((*meta).clone()),
        })
    }
}

struct Summary {
    meta: Option<crate::ReplayMeta>,
}

impl Analyzer for Summary {
    fn finish(&self) {
        let meta = self.meta.as_ref().unwrap();
        println!("Username: {}", meta.playerName);
        println!("Date/time: {}", meta.dateTime);
        println!("Map: {}", meta.mapDisplayName);
        println!("Vehicle: {}", meta.playerVehicle);
        println!("Game mode: {} {}", meta.name, meta.gameLogic);
        println!("Game version: {}", meta.clientVersionFromExe);
        println!();
        // TODO: Banners, damage, etc.
    }

    fn process(&mut self, packet: &Packet<'_, '_>) {
        // Collect banners, damage reports, etc.
    }
}
