use std::collections::HashMap;

use crate::packet2::Entity;

pub trait AnalyzerBuilder {
    fn build(&self, meta: &crate::ReplayMeta) -> Box<dyn Analyzer>;
}

pub trait AnalyzerMutBuilder {
    fn build(&self, meta: &crate::ReplayMeta) -> Box<dyn AnalyzerMut>;
}

pub trait Analyzer {
    fn process(&self, packet: &crate::packet2::Packet<'_, '_>);
    fn finish(&self);
}

pub trait AnalyzerMut {
    fn process_mut(&mut self, packet: &crate::packet2::Packet<'_, '_>);
    fn finish(&mut self);
}

pub struct AnalyzerAdapter {
    analyzers: Vec<Box<dyn AnalyzerMut>>,
}

impl AnalyzerAdapter {
    pub fn new(analyzers: Vec<Box<dyn AnalyzerMut>>) -> Self {
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

impl crate::packet2::PacketProcessorMut for AnalyzerAdapter {
    fn process_mut(&mut self, packet: crate::packet2::Packet<'_, '_>) {
        for a in self.analyzers.iter_mut() {
            a.process_mut(&packet);
        }
    }
}
