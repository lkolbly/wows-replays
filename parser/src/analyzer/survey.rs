use crate::analyzer::*;
use crate::packet2::{EntityMethodPacket, Packet, PacketType};
use std::cell::{RefCell, RefMut};
use std::collections::HashMap;
use std::rc::Rc;

pub struct SurveyStats {
    pub total_packets: usize,
    pub invalid_packets: usize,
}

impl SurveyStats {
    pub fn new() -> Self {
        Self {
            total_packets: 0,
            invalid_packets: 0,
        }
    }
}

pub struct SurveyBuilder {
    stats: Rc<RefCell<SurveyStats>>,
    skip_decoder: bool,
}

impl SurveyBuilder {
    pub fn new(stats: Rc<RefCell<SurveyStats>>, skip_decoder: bool) -> Self {
        Self {
            stats,
            skip_decoder,
        }
    }
}

impl AnalyzerBuilder for SurveyBuilder {
    fn build(&self, meta: &crate::ReplayMeta) -> Box<dyn Analyzer> {
        Box::new(Survey {
            meta: Some((*meta).clone()),
            skip_decoder: self.skip_decoder,
            decoder: decoder::DecoderBuilder::new(true, None).build(meta),
            stats: self.stats.clone(),
        })
    }
}

struct Survey {
    meta: Option<crate::ReplayMeta>,
    skip_decoder: bool,
    decoder: Box<dyn Analyzer>,
    stats: Rc<RefCell<SurveyStats>>,
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
        if !self.skip_decoder {
            self.decoder.process(packet);
        }

        let mut stats: RefMut<_> = self.stats.borrow_mut();
        match &packet.payload {
            crate::packet2::PacketType::Invalid(_) => {
                stats.invalid_packets += 1;
            }
            _ => {}
        }
        stats.total_packets += 1;
    }
}
