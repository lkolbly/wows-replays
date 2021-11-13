use crate::analyzer::*;
use crate::packet2::Packet;
use std::cell::{RefCell, RefMut};
use std::rc::Rc;

pub struct SurveyStats {
    pub total_packets: usize,
    pub invalid_packets: usize,
    pub audits: Vec<String>,
}

impl SurveyStats {
    pub fn new() -> Self {
        Self {
            total_packets: 0,
            invalid_packets: 0,
            audits: vec![],
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
        let version = crate::version::Version::from_client_exe(&meta.clientVersionFromExe);
        Box::new(Survey {
            skip_decoder: self.skip_decoder,
            decoder: decoder::DecoderBuilder::new(true, true, None).build(meta),
            stats: self.stats.clone(),
            version: version,
        })
    }
}

struct Survey {
    skip_decoder: bool,
    decoder: Box<dyn Analyzer>,
    stats: Rc<RefCell<SurveyStats>>,
    version: crate::version::Version,
}

impl Analyzer for Survey {
    fn finish(&self) {
        self.decoder.finish();
    }

    fn process(&mut self, packet: &Packet<'_, '_>) {
        // Do stuff and such
        let mut stats: RefMut<_> = self.stats.borrow_mut();
        if !self.skip_decoder {
            //let decoded = self.decoder.process(packet);
            let decoded = decoder::DecodedPacket::from(&self.version, true, packet);
            match &decoded.payload {
                crate::analyzer::decoder::DecodedPacketPayload::Audit(s) => {
                    stats.audits.push(s.to_string());
                }
                _ => {}
            }
        }

        match &packet.payload {
            crate::packet2::PacketType::Invalid(_) => {
                stats.invalid_packets += 1;
            }
            _ => {}
        }
        stats.total_packets += 1;
    }
}
