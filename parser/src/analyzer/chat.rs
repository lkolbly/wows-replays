use crate::analyzer::decoder::{DecodedPacket, DecodedPacketPayload};
use crate::analyzer::{Analyzer, AnalyzerBuilder};
use crate::packet2::{EntityMethodPacket, Packet, PacketType};
use std::collections::HashMap;
use std::convert::TryInto;

pub struct ChatLoggerBuilder;

impl ChatLoggerBuilder {
    pub fn new() -> ChatLoggerBuilder {
        ChatLoggerBuilder
    }
}

impl AnalyzerBuilder for ChatLoggerBuilder {
    fn build(&self, meta: &crate::ReplayMeta) -> Box<dyn Analyzer> {
        let version = crate::version::Version::from_client_exe(&meta.clientVersionFromExe);
        Box::new(ChatLogger {
            usernames: HashMap::new(),
            version,
        })
    }
}

pub struct ChatLogger {
    usernames: HashMap<i32, String>,
    version: crate::version::Version,
}

impl Analyzer for ChatLogger {
    fn finish(&self) {}

    fn process(&mut self, packet: &Packet<'_, '_>) {
        let decoded = DecodedPacket::from(&self.version, false, packet);
        match decoded.payload {
            DecodedPacketPayload::Chat {
                entity_id,
                sender_id,
                audience,
                message,
            } => {
                println!(
                    "{}: {}: {} {}",
                    decoded.clock,
                    self.usernames.get(&sender_id).unwrap(),
                    audience,
                    message
                );
            }
            DecodedPacketPayload::VoiceLine {
                sender_id,
                is_global,
                message,
            } => {
                println!(
                    "{}: {}: voiceline {:#?}",
                    decoded.clock,
                    self.usernames.get(&sender_id).unwrap(),
                    message
                );
            }
            DecodedPacketPayload::OnArenaStateReceived { players, .. } => {
                for player in players.iter() {
                    self.usernames
                        .insert(player.avatarid.try_into().unwrap(), player.username.clone());
                }
            }
            _ => {}
        }
    }
}
