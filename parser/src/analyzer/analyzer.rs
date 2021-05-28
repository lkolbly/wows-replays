pub trait AnalyzerBuilder {
    fn build(&self, meta: &crate::ReplayMeta) -> Box<dyn Analyzer>;
}

pub trait Analyzer {
    fn process(&mut self, packet: &crate::packet2::Packet<'_, '_>);
    fn finish(&self);
}

pub struct AnalyzerAdapter {
    analyzers: Vec<Box<dyn Analyzer>>,
}

impl AnalyzerAdapter {
    pub fn new(analyzers: Vec<Box<dyn Analyzer>>) -> Self {
        Self { analyzers }
    }
}

impl AnalyzerAdapter {
    pub fn finish(&mut self) {
        for a in self.analyzers.iter_mut() {
            a.finish();
        }
    }
}

impl crate::packet2::PacketProcessor for AnalyzerAdapter {
    fn process(&mut self, packet: crate::packet2::Packet<'_, '_>) {
        for a in self.analyzers.iter_mut() {
            //self.process(packet);
            a.process(&packet);
        }
    }
}
