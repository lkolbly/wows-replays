use crate::analyzer::{Analyzer, AnalyzerBuilder};
use crate::packet2::Packet;

use super::analyzer::AnalyzerMut;

pub struct PacketDumpBuilder {}

impl PacketDumpBuilder {
    pub fn new() -> Self {
        Self {}
    }
}

impl AnalyzerBuilder for PacketDumpBuilder {
    fn build(&self, _: &crate::ReplayMeta) -> Box<dyn Analyzer> {
        Box::new(PacketDump {})
    }
}

struct PacketDump {}

impl Analyzer for PacketDump {
    fn finish(&self) {}

    fn process(&self, packet: &Packet<'_, '_>) {
        println!("{}", serde_json::to_string(packet).unwrap());
    }
}

impl AnalyzerMut for PacketDump {
    fn finish(&mut self) {}

    fn process_mut(&mut self, packet: &Packet<'_, '_>) {
        Analyzer::process(self, packet);
    }
}
