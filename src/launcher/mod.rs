mod args;
mod fullscreen;
mod inline;
mod media;
mod media_meta;
mod toggle;

pub use fullscreen::launch_fullscreen;
pub use inline::launch_inline;
pub use media::{launch_media, launch_media_inline};
pub use toggle::launch_toggle;
