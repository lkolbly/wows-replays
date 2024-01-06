pub mod analyzer;
mod error;
pub mod game_params;
mod nested_property_path;
pub mod packet2;
pub mod resource_loader;
pub mod rpc;
pub mod version;
mod wowsreplay;

pub use error::*;
pub use rpc::entitydefs::parse_scripts;
pub use strum;
pub use wowsreplay::*;

#[cfg(feature = "arc")]
pub type Rc<T> = std::sync::Arc<T>;

#[cfg(not(feature = "arc"))]
pub type Rc<T> = std::rc::Rc<T>;
