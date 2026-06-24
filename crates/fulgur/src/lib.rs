/// Maximum DOM tree depth before recursion is cut off. Prevents stack overflow
/// from pathologically deep HTML input.
pub(crate) const MAX_DOM_DEPTH: usize = 512;

/// Maximum number of pages a single render may produce. Bounds the
/// per-page-strip slicing of an oversized block in `pagination_layout`
/// so a tiny input with a pathologically tall CSS height (e.g.
/// `height: 99999999px`) cannot force unbounded fragment/page generation
/// — which downstream inflates a `vec![Vec::new(); page_count]` allocation
/// and a per-page render loop into a CPU/memory-exhaustion DoS. Content
/// beyond this many pages is truncated (clamp-and-warn), matching the
/// `MAX_DOM_DEPTH` / background `MAX_TILES` defensive-bound precedent.
pub(crate) const MAX_PAGES: u32 = 10_000;

pub mod asset;
pub mod background;
pub mod blitz_adapter;
pub(crate) mod column_css;
pub mod config;
pub mod convert;
pub mod draw_primitives;
#[doc(hidden)]
pub mod drawables;
pub mod engine;
pub mod error;
pub mod gcpm;
pub mod image;
pub mod inspect;
pub(crate) mod link;
#[doc(hidden)]
pub mod multicol_layout;
pub(crate) mod net;
pub mod outline;
#[doc(hidden)]
pub mod pagination_layout;
pub mod paragraph;
pub mod render;
pub mod schema;
pub mod svg;
pub mod tagging;
pub mod template;

pub use asset::AssetBundle;
pub use config::{Config, ConfigBuilder, Margin, PageSize};
pub use engine::{Engine, EngineBuilder};
pub use error::{Error, Result};
pub use outline::build_outline;

/// Convert HTML to PDF with default settings.
pub fn convert_html(html: &str) -> Result<Vec<u8>> {
    let engine = Engine::builder().build();
    engine.render_html(html)
}
