/// Maximum DOM tree depth before recursion is cut off. Prevents stack overflow
/// from pathologically deep HTML input.
pub(crate) const MAX_DOM_DEPTH: usize = 512;

/// Per-element page-amplification bound (NOT a strict total-page ceiling —
/// see the note below). Bounds the per-page-strip slicing of an oversized
/// block in `pagination_layout` (and the absolute-positioning page-extension
/// path) so a tiny input with a pathologically tall CSS height / offset
/// (e.g. `height: 99999999px`, `position:absolute; top:99999999px`) cannot
/// force unbounded fragment / page generation — which downstream inflates a
/// `vec![Vec::new(); page_count]` allocation and a per-page render loop into
/// a CPU/memory-exhaustion DoS.
///
/// The cap is keyed off the running `page_index`, not a per-element budget.
/// That is deliberate: it makes many oversized siblings **additive** rather
/// than **multiplicative** — each extra oversized block contributes only its
/// own first fragment once the slice loop is capped, so the document total
/// settles at `MAX_PAGES + N` for `N` body children instead of
/// `N * MAX_PAGES`. A per-element budget would re-open that multi-element
/// amplification, so the cap must stay keyed off the absolute page index
/// (Codex review on PR #501).
///
/// Consequence: the total page count can slightly **exceed** `MAX_PAGES`
/// (it is `MAX_PAGES + N`, still input-proportional and rendered in bounded
/// time). This constant therefore caps single-element amplification, not the
/// absolute page count — code must not assume `page_count <= MAX_PAGES`.
///
/// Set high enough that legitimate large documents (batch report generation
/// is a primary use case) do not hit it — at this value the truncation only
/// fires for attacker-amplified input, and rendering the bound is still
/// bounded (~100k pages ≈ a few seconds). Content past the per-element cap is
/// truncated (clamp-and-warn). Sibling of the `MAX_DOM_DEPTH` / background
/// `MAX_TILES` defensive bounds.
pub(crate) const MAX_PAGES: u32 = 100_000;

pub mod asset;
pub mod background;
pub mod blitz_adapter;
pub mod column_css;
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
pub mod units;

pub use asset::AssetBundle;
pub use config::{Config, ConfigBuilder, Margin, PageSize};
pub use engine::{Engine, EngineBuilder};
pub use error::{Error, Result};
pub use outline::build_outline;

/// Convert HTML to PDF with default settings.
pub fn convert_html(html: &str) -> Result<Vec<u8>> {
    let engine = Engine::builder().build();
    engine.render(html)
}
