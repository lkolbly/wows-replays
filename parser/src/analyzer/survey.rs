use crate::analyzer::*;
use crate::packet2::{EntityMethodPacket, Packet, PacketType};
use std::collections::HashMap;

pub struct SurveyBuilder;

impl SurveyBuilder {
    pub fn new() -> Self {
        Self
    }
}

impl AnalyzerBuilder for SurveyBuilder {
    fn build(&self, meta: &crate::ReplayMeta) -> Box<dyn Analyzer> {
        Box::new(Survey {
            meta: Some((*meta).clone()),
            decoder: decoder::DecoderBuilder::new(true, None).build(meta),
        })
    }
}

struct Survey {
    meta: Option<crate::ReplayMeta>,
    decoder: Box<dyn Analyzer>,
}

impl Analyzer for Survey {
    fn finish(&self) {
        /*let meta = self.meta.as_ref().unwrap();
        println!("Username: {}", meta.playerName);
        println!("Date/time: {}", meta.dateTime);
        println!("Map: {}", meta.mapDisplayName);
        println!("Vehicle: {}", meta.playerVehicle);
        println!("Game mode: {} {}", meta.name, meta.gameLogic);
        println!("Game version: {}", meta.clientVersionFromExe);
        println!();
        for (ribbon, count) in self.ribbons.iter() {
            println!("{:?}: {}", ribbon, count);
        }
        println!();
        /*for ((a, b), (c, d)) in self.damage.iter() {
            println!("{} {}: {} {}", a, b, c, d);
        }*/
        println!(
            "Total damage: {:.0}",
            self.damage.get(&(1, 0)).unwrap_or(&(0, 0.)).1
                + self.damage.get(&(2, 0)).unwrap_or(&(0, 0.)).1
                + self.damage.get(&(17, 0)).unwrap_or(&(0, 0.)).1
        );*/
        self.decoder.finish();
    }

    fn process(&mut self, packet: &Packet<'_, '_>) {
        // Do stuff and such
        self.decoder.process(packet);
    }
}
