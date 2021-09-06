use crate::analyzer::{Analyzer, AnalyzerBuilder};
use crate::packet2::Packet;

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

    fn process(&mut self, packet: &Packet<'_, '_>) {
        println!("{}", serde_json::to_string(packet).unwrap());
    }
}
