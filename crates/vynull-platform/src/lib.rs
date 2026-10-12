//! Platform traits (`Store`, audio, event stream) with web and native backends.

pub mod audio;
pub mod events;
pub mod store;
pub mod time;

#[cfg(not(target_arch = "wasm32"))]
pub use audio::NativeAudio;
#[cfg(target_arch = "wasm32")]
pub use audio::WebAudio;
pub use audio::{AudioPlayer, AudioState, FakeAudio, PlayStatus};
#[cfg(not(target_arch = "wasm32"))]
pub use events::state_stream_with;
pub use events::{state_stream, StreamItem};
#[cfg(not(target_arch = "wasm32"))]
pub use store::FileStore;
#[cfg(target_arch = "wasm32")]
pub use store::WebStore;
pub use store::{MemoryStore, Store, StoreError, StoreExt};
