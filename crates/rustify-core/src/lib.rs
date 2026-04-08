pub mod art;
pub mod error;
pub mod metadata;
pub mod mixer;
pub mod player;
pub mod playlist;
pub mod scanner;
pub mod tracklist;
pub mod types;

// Re-export primary types at crate root for convenience.
pub use error::{Result, RustifyError};
pub use player::{Player, PlayerConfig};
pub use types::{PlaybackState, PlayerCommand, PlayerEvent, Playlist, RepeatMode, Track};
