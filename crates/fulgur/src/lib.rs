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
/// Kept deliberately conservative for untrusted server-side rendering (a
/// shared multi-tenant renderer is the primary deployment): a tiny
/// pathological input must not amplify into one render iteration and PDF
/// page per fragment. The most common amplifier — a childless block with a
/// huge height, which prints only blank pages — is additionally collapsed to
/// a single page in `pagination_layout` (fulgur-ezst), so this ceiling only
/// bounds the residual content-bearing case. Content past the cap is
/// truncated (clamp-and-warn). Sibling of the `MAX_DOM_DEPTH` / background
/// `MAX_TILES` defensive bounds.
pub(crate) const MAX_PAGES: u32 = 10_000;

/// Maximum number of columns a single multi-column container is laid out
/// across, regardless of the CSS `column-count` / `column-width` the input
/// requests. CSS accepts `column-count` up to `i32::MAX`, and a small
/// `column-width` on a wide container derives an equally large used count —
/// neither is bounded by the spec.
///
/// The used count `n` is a **multiplier on retained layout metadata**: the
/// multicol hook allocates one `vec![0.0; n]` per column group (`col_heights`)
/// and, for every paragraph that splits across columns, an `n`-length
/// `column_slices` vector padded with placeholders for unused columns (its
/// length equals `n` by the `ParagraphSplitEntry` contract). A single
/// `<div style="column-count:2000000000">x</div>` therefore forces one ~8 GB
/// `col_heights` allocation, and the split path amplifies to
/// O(`column_count` × paragraph_count) — an unbounded, attacker-controllable
/// memory-exhaustion DoS on a server-side renderer of untrusted HTML/CSS.
///
/// Clamping the resolved count at the single choke point
/// ([`multicol_layout::resolve_column_layout`]) bounds every downstream `n`
/// sink at once (`col_heights`, `column_slices`, `slice_lines_by_budget`) and
/// keeps the total metadata input-proportional (`n` is a small constant, so
/// the split path stays O(paragraph_count)). 64 is far above any legitimate
/// print layout — real multi-column text rarely exceeds a handful of columns —
/// so no realistic document is affected; a pathological explicit or
/// width-derived count is merely clamped (the columns collapse to zero width,
/// as they already would past the container's capacity). Sibling of the
/// [`MAX_DOM_DEPTH`] / [`MAX_PAGES`] defensive bounds.
pub(crate) const MAX_COLUMN_COUNT: u32 = 64;

/// Maximum number of CSS gradient color stops retained per background layer.
/// CSS accepts an unbounded stop list (`linear-gradient(c0, c1, …, cN)`), and
/// convert clones the resolved `Vec<GradientStop>` into every element's
/// [`crate::draw_primitives::BackgroundLayer`] — so a rule like
/// `* { background: linear-gradient(<N stops>) }` retains O(elements × N).
/// For repeating gradients the draw-time period expansion in
/// `background::expand_repeating_stops` further materializes
/// `total_copies × stops.len()` entries (`total_copies` is already bounded by
/// its own `MAX_PERIODS`), so bounding `stops.len()` here keeps that product
/// bounded too.
///
/// Clamping the stop count at the convert-side choke points that build the
/// vector (`resolve_color_stops` for linear/radial, plus the conic loop)
/// bounds both the retained per-layer vector and the downstream repeating
/// expansion at once. 256 stops is far beyond any legitimate gradient — real
/// designs use a handful — so no realistic document changes output; excess
/// stops are truncated (clamp-and-warn). Sibling of the [`MAX_COLUMN_COUNT`]
/// multicol bound.
pub(crate) const MAX_GRADIENT_STOPS: usize = 256;

/// Upper bound on the **output bytes** materialized for a single resolved
/// counter chain (`counters(name, sep, style)`), applied inside
/// [`gcpm::counter::format_counter_chain`] so it covers every call site
/// (`::before` / `::after` resolution, margin-box, and `target-counters()`).
///
/// `counters()` joins the active counter chain with an attacker-controlled
/// separator, so a single resolution is `separator_len * chain_len`. This caps
/// the *separator* axis: even a multi-kilobyte separator cannot push one
/// `counters()` output past this ceiling. The orthogonal *length* axis is
/// bounded by [`MAX_COUNTER_CHAIN_ENTRIES`]. Real documents (deepest legitimate
/// `counters()`, e.g. nested `<ol>`) stay in the tens of bytes, far below this.
/// Sibling of the total-output [`MAX_GENERATED_CSS_BYTES`] budget.
pub(crate) const MAX_COUNTER_CHAIN_BYTES: usize = 4 * 1024;

/// Upper bound on the **number of entries** kept for a single counter chain
/// (its length), applied when a chain is cloned for later resolution
/// (`CounterState::chain` / `chain_snapshot`).
///
/// A chain grows one entry per in-scope `counter-reset`. The *nesting*
/// contribution is already bounded by [`MAX_DOM_DEPTH`] — the counter walk
/// stops recursing there, so no reset below that depth is even processed —
/// which is why this cap is tied to it: any chain longer than
/// `MAX_DOM_DEPTH` can only come from following-sibling `counter-reset`
/// accumulation (unbounded breadth, i.e. adversarial), and that surplus cannot
/// affect a realistic document. Bounding the stored length keeps the retained
/// per-node snapshots from being O(N²) and is the *length* companion to the
/// *separator*-axis [`MAX_COUNTER_CHAIN_BYTES`]; the two are independent knobs
/// on `separator_len * chain_len`.
pub(crate) const MAX_COUNTER_CHAIN_ENTRIES: usize = MAX_DOM_DEPTH;

/// Total-output budget for the generated CSS that
/// [`blitz_adapter::CounterPass`] injects for resolved pseudo-element
/// content. Once the accumulated generated CSS reaches this size, further
/// per-element rules are skipped (the pseudo-element simply does not render).
///
/// Each element with matching `::before` / `::after` counter content emits
/// its own generated rule, and the element (sibling) count is bounded only by
/// input size — so without a total cap the aggregate is input-proportional in
/// N with a per-rule constant, still a resource-exhaustion vector even after
/// [`MAX_COUNTER_CHAIN_BYTES`] bounds each single rule. This budget makes the
/// aggregate output absolutely bounded regardless of N, mirroring the
/// additive-not-multiplicative rationale of [`MAX_PAGES`]. Kept generous so no
/// realistic document is clipped (a document would need on the order of 10^5
/// counter-bearing pseudo-elements to reach it).
pub(crate) const MAX_GENERATED_CSS_BYTES: usize = 8 * 1024 * 1024;

/// Total-output budget for the resolved `bookmark-label` strings that
/// [`blitz_adapter::BookmarkPass`] accumulates into the PDF outline. Once the
/// aggregate resolved-label size reaches this bound, further outline entries
/// are skipped.
///
/// `bookmark-label: counters(name, sep)` resolves through the same
/// [`MAX_COUNTER_CHAIN_BYTES`]-capped path, so a single label can be up to
/// that cap while its originating element is only a few bytes — a per-element
/// amplification that plain text labels (whose bytes come from the input)
/// cannot produce. The outline is a separate sink from the generated CSS, so
/// [`MAX_GENERATED_CSS_BYTES`] does not bound it; this budget caps the
/// N-element accumulation there, mirroring that guard. Kept generous so no
/// realistic document (headings / figures with short labels) is clipped.
pub(crate) const MAX_OUTLINE_LABEL_BYTES: usize = 8 * 1024 * 1024;

/// Total-storage budget (in counter values) for the per-node counter-chain
/// snapshots that [`blitz_adapter::CounterPass`] harvests for `BookmarkPass` /
/// anchor resolution. Once the accumulated stored-value count reaches this
/// bound, no further node snapshots are recorded.
///
/// Snapshot recording clones the *full* active counter chain
/// (`chain_snapshot`) at every visited element. Sibling `counter-reset`s make
/// the chain length ≈ the sibling index, so storing it at N nodes is O(N²)
/// memory — a resource-exhaustion sink independent of any output budget
/// (`MAX_GENERATED_CSS_BYTES` / `MAX_OUTLINE_LABEL_BYTES` bound output bytes,
/// not this retained storage). Per-node length is separately clamped to
/// [`MAX_COUNTER_CHAIN_ENTRIES`] inside `chain_snapshot` (lossless for realistic
/// counter state — nesting depth is bounded by [`MAX_DOM_DEPTH`], and anything
/// longer is adversarial sibling accumulation); this budget then bounds the
/// aggregate
/// across all nodes, mirroring the additive-not-multiplicative rationale of
/// [`MAX_PAGES`]. ~8M values ≈ 32 MiB — far above any realistic document.
pub(crate) const MAX_COUNTER_SNAPSHOT_ENTRIES: usize = 8 * 1024 * 1024;

/// Total-storage budget (in bytes) for the per-node named-string snapshots that
/// [`blitz_adapter::StringSetPass`] harvests for `BookmarkPass` `string(name)`
/// resolution inside `bookmark-label`. Once the accumulated stored string bytes
/// reach this bound, no further node snapshots are recorded.
///
/// Snapshot recording clones the *full* running `name -> value` map at every
/// visited element, and the values are attacker-controlled strings resolved
/// from element text / attributes (`string-set: title content(text)`). A single
/// large value snapshotted at N nodes is O(N × value_size) retained memory — a
/// resource-exhaustion sink independent of any output budget, and the string
/// twin of the counter-chain [`MAX_COUNTER_SNAPSHOT_ENTRIES`] guard. This budget
/// bounds the aggregate across all nodes regardless of N, mirroring the
/// additive-not-multiplicative rationale of [`MAX_PAGES`]. Kept generous (8 MiB)
/// so no realistic bookmark-bearing document (short named strings × headings) is
/// clipped; adversarial input degrades `string()` to the CSS default ("") for
/// nodes past the cap, which is acceptable.
pub(crate) const MAX_STRING_SNAPSHOT_BYTES: usize = 8 * 1024 * 1024;

/// Approximate per-record heap + struct overhead charged against the
/// [`MAX_STRING_SNAPSHOT_BYTES`] / [`MAX_STRING_SET_STORE_BYTES`] budgets in
/// addition to each record's `name` + `value` payload bytes.
///
/// Both budgets otherwise counted only string payload, so empty / tiny named
/// strings (`p { string-set: x "" }` matched by N elements, or a per-node
/// snapshot recorded at every visited element while the running map is still
/// empty) accumulated near-zero *counted* bytes while retaining millions of
/// `StringSetEntry` / `BTreeMap` records — each of which costs `String` headers,
/// a node id, and B-tree / `Vec` slot overhead that dwarfs a zero-length value.
/// Charging this per-record constant makes the byte budget bound the record
/// *count* as well (ceiling ≈ `budget / this`, ~131 K records at 8 MiB), matching
/// the entry-count guard of the sibling [`MAX_COUNTER_SNAPSHOT_ENTRIES`]. Sized
/// to conservatively cover two `String` structs (24 B each) plus a node id and
/// container slot; realistic bookmark documents stay far below the ceiling.
pub(crate) const STRING_ENTRY_OVERHEAD_BYTES: usize = 64;

/// Total-storage budget (in bytes) for the resolved `string-set` values that
/// [`gcpm::string_set::StringSetStore`] accumulates during
/// [`blitz_adapter::StringSetPass`]. Once the accumulated stored value bytes
/// would exceed this bound, further assignments are dropped.
///
/// Unlike the [`MAX_STRING_SNAPSHOT_BYTES`] snapshot (a bookmark-only aux gated
/// on `record_node_snapshots`), the store is populated *unconditionally* on the
/// default render path — one entry per (element, matching `string-set` rule) —
/// and its values are cloned again downstream into the per-node render map
/// (`Engine::render_html`'s `string_set_by_node` / `string_set_for_render`). A
/// single repeated literal (`p { string-set: x "BIG" }` matched by N sibling
/// elements) pushes N copies of `BIG` from a single-copy input — an
/// O(N × value_size) breadth-unbounded amplification independent of any output
/// budget. Capping the aggregate at the source push bounds the downstream clones
/// dependently (peak ≈ 3× this budget). Kept generous (8 MiB) so no realistic
/// document (headings / figures with short named strings, even thousands of
/// them) is clipped; past the cap, `string()` reads the last recorded value —
/// a stale/absent-value degradation acceptable under adversarial input. Store
/// sibling of [`MAX_STRING_SNAPSHOT_BYTES`].
pub(crate) const MAX_STRING_SET_STORE_BYTES: usize = 8 * 1024 * 1024;

/// Per-call cap on the bytes materialized while resolving a single content /
/// label list (`::before` / `::after`, `bookmark-label`, margin-box running
/// content) into one string.
///
/// [`MAX_COUNTER_CHAIN_BYTES`] bounds a *single* `counters()` / `counter()` /
/// `target-counters()` item, but a content list may hold an unbounded number
/// of such items (`content: counters(x) counters(x) …` — the parser caps
/// neither item count nor list length). The aggregate output budgets
/// ([`MAX_GENERATED_CSS_BYTES`], [`MAX_OUTLINE_LABEL_BYTES`]) are checked once
/// per element *before* the list is resolved, so a single element could
/// otherwise materialize `item_count × MAX_COUNTER_CHAIN_BYTES` in one
/// allocation and overshoot them. This bounds that per-call materialization.
/// Kept generous — no realistic single element resolves anywhere near this —
/// so only adversarial multi-item lists are clipped.
pub(crate) const MAX_RESOLVED_CONTENT_BYTES: usize = 1024 * 1024;

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
pub use engine::{Engine, EngineBuilder, LayoutOutput};
pub use error::{Error, Result};
pub use outline::build_outline;

/// Convert HTML to PDF with default settings.
pub fn convert_html(html: &str) -> Result<Vec<u8>> {
    let engine = Engine::builder().build();
    engine.render(html)
}
