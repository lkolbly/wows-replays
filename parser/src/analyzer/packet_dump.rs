use crate::analyzer::{Analyzer, AnalyzerBuilder};
use crate::packet2::Packet;

pub struct PacketDumpBuilder {
    time_offset: f32,
}

impl PacketDumpBuilder {
    pub fn new(time_offset: f32) -> Self {
        Self { time_offset }
    }
}

impl AnalyzerBuilder for PacketDumpBuilder {
    fn build(&self, _: &crate::ReplayMeta) -> Box<dyn Analyzer> {
        Box::new(PacketDump {
            time_offset: self.time_offset,
        })
    }
}

struct PacketDump {
    time_offset: f32,
}

impl Analyzer for PacketDump {
    fn finish(&self) {}

    fn process(&mut self, packet: &Packet<'_, '_>) {
        let time = packet.clock + self.time_offset;
        let minutes = (time / 60.0).floor() as i32;
        let seconds = (time - minutes as f32 * 60.0).floor() as i32;
        //println!("{:02}:{:02}: {:?}", minutes, seconds, packet.payload);
        println!("{}", serde_json::to_string(packet).unwrap());
    }
}
