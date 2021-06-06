use crate::analyzer::*;
use crate::packet2::Packet;
use std::cell::{RefCell, RefMut};
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
            skip_decoder: self.skip_decoder,
            decoder: decoder::DecoderBuilder::new(true, true, None).build(meta),
            stats: self.stats.clone(),
        })
    }
}

struct Survey {
    skip_decoder: bool,
    decoder: Box<dyn Analyzer>,
    stats: Rc<RefCell<SurveyStats>>,
}

impl Analyzer for Survey {
    fn finish(&self) {
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
