# `Engine::layout()` 公開 API と座標 newtype 基盤 — 設計

- Status: Draft
- Date: 2026-06-27
- Related: PR #459 (feat: HTML to image output), maintainer comment
  <https://github.com/fulgur-rs/fulgur/pull/459#issuecomment-4612646966>
- Coordinate rules: `.claude/rules/coordinate-system.md`

## 1. 背景と目的

fulgur のレイアウトエンジン（parse → style → layout → `Drawables`）を、コアを
lean に保ったまま外部 crate から再利用可能にする。PR #459 が画像出力をコアに
取り込もうとしたのに対し、メンテナは「コアは `layout()` を公開し、ラスタライズは
コア外」という crate レベルの分離（`wkhtmltopdf` / `wkhtmltoimage` と同じ思想）を
選んだ。

公開 `layout()` の想定 consumer は 2 つ:

1. **PDF→画像**: page-0 `Drawables` を SVG 化 → resvg/tiny-skia でラスタライズ →
   PNG / lossless WebP エンコード（PR #459 の `image_export/*` がほぼそのまま移植
   可能）。
2. **OCR 学習データ生成**: fulgur は各グリフの配置を正確に知っているため、
   **ピクセル完全なラベル（テキスト＋bbox）付きの合成画像**を生成できる。画像
   エンコードではなく、ラスタライズのテンソル（生ピクセル）とラベルを直接得る。
   これは fulgur 固有の強み。

両 consumer が必要とする公開面は実質 `(Drawables, PaginationGeometryTable)` のみ。
`gcpm` / `running_store` などは PDF 側 `render_v2` 専用で、margin box / running
element は render-side のため画像・OCR では**自動的に落ちる**（特別扱い不要）。

## 2. 決定事項サマリ

| 論点 | 決定 |
|---|---|
| 返り値の形 | 名前付き struct `LayoutOutput { drawables, geometry }`（タプルではなく。非破壊でフィールド追加可能） |
| `target-*` 2-pass | 公開 `layout()` は内部で 2-pass を回し、解決済み Drawables を返す（PDF と忠実一致） |
| シグネチャ | `pub fn layout(&self, html: &str) -> Result<LayoutOutput>`。canvas/page サイズは builder 経由 |
| 座標系 | **`Px`/`Pt` newtype 単位系**（演算子 impl ＋ ctor 糖衣 ＋ FFI 境界 `.to_f32()`）をコアに据える |
| 順序 | **newtype 先行**（型移行を private のうちに完了 → 公開は一度きり）。理由: §7 |
| ラスタライズ / エンコード / テンソル生成 | **すべて外部 crate**。コアは関与しない |
| OCR ラベル抽出（合成・グリフ配置） | **consumer（外部 crate）が所有**。コアは「単位変換」を所有し、「合成（配置）」は consumer |

設計原則の一行: **コアは「単位変換（pt↔px）」を所有する。consumer は「合成
（配置＝どこに置くか、グリフ配置）」を所有する。**

## 3. 公開 API

```rust
/// parse → style → layout → Drawables の成果物。
/// PDF 描画にも画像描画にも OCR にも使える、レンダラ非依存のレイアウト結果。
pub struct LayoutOutput {
    pub drawables: Drawables,
    pub geometry: PaginationGeometryTable,
}

impl Engine {
    /// HTML をレイアウトし、描画用の per-node ペイロードを返す。
    /// ページ形状（canvas サイズ・余白）は builder の設定を使う。
    /// target-* がある文書では内部で 2-pass を回し、解決済み Drawables を返す。
    pub fn layout(&self, html: &str) -> Result<LayoutOutput>;
}
```

`lib.rs` で `pub use engine::LayoutOutput;` を再エクスポート。

consumer（fulgur-image / OCR）の典型形:

```rust
let out = Engine::builder()
    .page_size(canvas)            // 画像 canvas = layout の page size
    .margin(Margin::uniform(Pt(0.0)))
    .landscape(false)
    .build()
    .layout(html)?;
// 以降は外部 crate が out.drawables / out.geometry を合成してラスタライズ or ラベル化
```

## 4. 内部リファクタ（PR #459 から流用）

PR が既に抽出済みの private ヘルパをそのまま採用する:

```rust
fn layout_to_drawables(&self, html, config, anchor_map) -> Result<LayoutArtifacts>
```

- `render_pass` は従来どおり `layout_to_drawables` → **無改変の** `render_v2`
  （PDF byte-identical）。
- 公開 `layout()` は**同じ** `layout_to_drawables` を呼ぶ薄いラッパ。`render_html`
  と同じ 2-pass ループ（pass1 → `needs_anchor_map_for_pass_two` なら pass2）を回し、
  最終 artifacts から `{ drawables, geometry }` だけ取り出す。
- → レイアウト経路は**1本**。PDF と画像/OCR でロジックが分岐しない。

`LayoutArtifacts`（private, 10 フィールド）は `render_v2` を駆動するための全
side-channel を持つが、公開 `LayoutOutput` はその部分集合（drawables, geometry）
のみを露出する。`gcpm` / `running_store` 等は非公開のまま。

## 5. 座標基盤: `Px` / `Pt` newtype 単位系

### 5.1 動機

現状、公開到達型は単位が混在している:

- `Drawables` の座標は **PDF pt**（`ParagraphSlice.origin_pt` 等、一部は suffix 済）。
- `PaginationGeometry::Fragment` は **CSS px**（Taffy ネイティブ、doc コメントに明記）。

混在自体は設計（Krilla=pt / Taffy=px）。これまでは `_pt`/`_px` の**命名規約**を
単位契約としてきたが、**未移行のフィールドが多く**、命名契約は穴だらけ。公開した
瞬間にこの不完全な契約が固定化される。

PR #459 の `svg_emit` は `const PX_TO_PT = 0.75` を**ローカル再定義**し、合成式
（`body_offset + frag * PX_TO_PT`）とグリフ配置（`baseline + advance*font_size`）を
**外部で再実装**していた。これは fulgur が最も恐れる 4/3・baseline 系バグの温床。

### 5.2 採用する型

```rust
#[repr(transparent)] #[derive(Clone, Copy, PartialEq, PartialOrd, Debug)]
pub struct Px(f32);
#[repr(transparent)] #[derive(Clone, Copy, PartialEq, PartialOrd, Debug)]
pub struct Pt(f32);

// 単位内の算術は演算子で（.0 不要）
impl core::ops::Add for Pt { type Output = Pt; fn add(self, o: Pt) -> Pt { Pt(self.0 + o.0) } }
impl core::ops::Sub for Pt { type Output = Pt; fn sub(self, o: Pt) -> Pt { Pt(self.0 - o.0) } }
impl core::ops::Mul<f32> for Pt { type Output = Pt; fn mul(self, s: f32) -> Pt { Pt(self.0 * s) } }
impl core::ops::Neg for Pt { type Output = Pt; fn neg(self) -> Pt { Pt(-self.0) } }
// Px も同様。Px + Pt は impl しない → 混ぜたらコンパイルエラー（=安全性）

// 変換（コアが所有する唯一の 0.75 定義）
pub const PX_TO_PT: f32 = 0.75;
impl Px { pub fn in_pt(self) -> Pt { Pt(self.0 * PX_TO_PT) } }
impl Pt { pub fn in_px(self) -> Px { Px(self.0 / PX_TO_PT) } }

// コンストラクタ糖衣（Px(..) を書かない）
pub trait F32Units { fn px(self) -> Px; fn pt(self) -> Pt; }
impl F32Units for f32 { fn px(self) -> Px { Px(self) } fn pt(self) -> Pt { Pt(self) } }

// FFI 脱出口（resvg / tiny-skia / krilla が f32 を要求する境界だけ）
impl Px { pub fn to_f32(self) -> f32 { self.0 } }
impl Pt { pub fn to_f32(self) -> f32 { self.0 } }
```

### 5.3 性質

- **実行時ゼロコスト**: `#[repr(transparent)]` で `f32` と同一レイアウト・ABI。演算子は
  monomorphize＋inline で消え、最適化後の機械語は素の `f32` 演算と同一。`.0` /
  `.to_f32()` はコンパイル時の見た目のみ。
- **決定性**: newtype 自体は値を変えないが、byte-neutral は**自動保証ではない**。
  演算子へ移行する際に `(a + b) * 0.75` を `a * 0.75 + b * 0.75` のように**再結合
  すると f32 の丸めが変わる**。演算順序の保存は各 call site の義務であり、§8 の
  per-Phase golden 再検証で担保する。
- **エルゴノミクス**: 演算子で `.0` は call site から消える。`.to_f32()` が出るのは
  FFI 境界だけ。
- **ラベリング完全性**: 単位が型なので、単位の無い座標フィールドは**構造的に存在
  し得ない**。命名規約の穴を恒久的に塞ぐ。
- **安全性**: `Px + Pt` 等の取り違えはコンパイルエラー。

### 5.4 call site 比較（PR svg_emit の合成）

```rust
// 現状（PR・ハードコード）: px+pt 取り違えを止める物が無い
let x = bx + frag.x * 0.75;

// 採用（newtype＋演算子）: Pt+Pt=Pt、px+pt はコンパイル不可
let x: Pt = drawables.body_offset.x + frag.x_px.in_pt();
emit_rect(x.to_f32(), ...);   // FFI 境界だけ .to_f32()
```

## 6. 公開到達性の前提（layout() 公開とセットで直す）

| ギャップ | 現状 | 対応 |
|---|---|---|
| 単位変換 | `convert::{px_to_pt,pt_to_px}` が `pub(crate)`、`PX_TO_PT` ローカル定義 | `Px::in_pt` / `Pt::in_px` として公開。0.75 はコア 1 箇所に集約 |
| `ColumnRuleSpec` | `column_css` が `pub(crate) mod` で公開フィールド `Drawables.multicol_rules` の型が外部から名前で呼べない（`private_interfaces` リーク） | `column_css` を `pub mod` 化、または型を re-export |
| `usvg::Tree` | `SvgEntry.tree: Arc<usvg::Tree>` が外部依存型 | consumer は同一 `usvg` バージョンに pin（要文書化） |

## 7. 順序（フェーズ分割）— newtype 先行（確定）

### なぜ newtype 先行か

これまで fulgur の公開面は **入力（HTML / CSS / アセット）と出力（PDF bytes）だけ**
で、`Drawables` / `geometry` は公開契約の外だった。`layout()` でこれらを公開する
瞬間、`Drawables` 配下の座標型**そのものが公開契約**になる。ここで newtype 移行を
後回しにすると、移行の各ステップが**公開済み `Drawables` への breaking change の
連発**になる。

移行 churn は**型が private のうちはタダ**（外部 consumer が存在しない）で、公開後は
破壊的。したがって **newtype 移行を private のうちに済ませ、公開は一度きり**にする。
クレート規模だが段階化する。

### フェーズ

- **Phase 0**: `Px`/`Pt` 型 ＋ 演算子 / ctor 糖衣 / FFI `.to_f32()` / 変換を
  `draw_primitives`（または新 `units` モジュール）に導入。移行はまだしない。
- **Phase 1**: 公開到達座標型（`Fragment`、`Drawables` 配下の座標、
  `ShapedLine` / `ShapedGlyphRun` の座標）と、それらを**生成・消費する内部経路**
  （convert / 各 fragmenter / render_v2 の座標扱い）を `Px`/`Pt` へ移行。
  byte-neutral（演算順序を保存）。
- **Phase 2**: 残りの内部 math を演算子へ移行し、f32↔newtype の一時境界を解消。
- **Phase 3（最後）**: 公開到達性ギャップ修正（§6）＋ `layout()` / `LayoutOutput`
  公開 ＋ `render_smoke.rs` に lib smoke test。**公開時点で公開座標契約は完全
  型付き**で、以後の単位移行による breaking change は発生しない。

### 不採用

- **公開面先行 / bare-f32 先行**: `Drawables` を公開してから newtype 化すると、
  公開契約への breaking change が連発するため不採用（上記理由）。ZeroVer で破壊は
  許容されるが、公開直後の API を壊し続けるのは consumer 体験として避ける。

### Spike outcome (fulgur-2map.2) — validated per-phase byte-neutral pattern

Migrated the multicol column-rule **pt carrier** end-to-end as the P0.5 spike
(PR/branch `fulgur-2map.2-multicol-px-pt`). Established recipe for P1a–P1d:

1. **Type the converted (pt) side with a dedicated carrier.** Do not reuse the
   px source struct for pt values. `drawables::ColumnRuleGeometry` (Pt fields)
   replaced the second `ColumnGroupGeometry` that held pt — closing the
   one-struct-two-units hole on the pt side.
2. **Convert at the boundary, one multiply.** `px_to_pt(v)` → `v.px().in_pt()`.
   Never distribute across `+` (no reassociation). `margin + px_to_pt(v)` →
   `margin.pt() + v.px().in_pt()`. Convert a sum once (`sum.px().in_pt()`), not
   per-term.
3. **`to_f32()` only at FFI** (the `stroke_line` krilla call).
4. **Clamps/folds need unit-preserving helpers** — added `Pt::max` / `Pt::min`
   mirroring `f32` exactly; `Pt(0.0)` is unconstructable outside `units`, so use
   `0.0_f32.pt()`.
5. **Proof = unchanged goldens.** `examples_determinism` + VRT stayed
   byte-identical; never `FULGUR_VRT_UPDATE=1`. Note `examples_determinism` is a
   run-twice self-comparison (catches nondeterminism, not value drift); the VRT
   golden-vs-baseline comparison is the load-bearing byte-neutrality proof.

Deferred to P1d (`fulgur-2map.6`): typing the px source
`multicol_layout::ColumnGroupGeometry` to `Px`, and
`column_css::ColumnRuleSpec.width` to `Pt`.

## 8. 決定性 / 受け入れ条件

- `layout_to_drawables` 抽出後・newtype 移行後とも `examples_determinism` golden と
  VRT golden が **byte-identical**（PR は branch で byte-neutral を主張 → 各 Phase で
  **再検証必須**）。
- 公開 `layout()` の lib smoke test を `render_smoke.rs` に追加
  （`Engine::builder().build().layout(html)` で `drawables`/`geometry` 非空。
  CLAUDE.md のカバレッジ方針）。
- newtype の単位混在（`Px + Pt` 等）がコンパイルエラーになることの型テスト
  （`trybuild` 等、任意）。

## 9. スコープ外 / 下流（別作業）

- `fulgur-image` crate 本体: SVG-emit → resvg → encode、`ImageOptions`。PR #459 の
  `image_export/*` がほぼそのまま移植可能。**rasterize → `Pixmap`（生ピクセル）を
  公開層として持ち、encode はその上の薄い層**にすると、OCR consumer が `Pixmap` を
  直接テンソル化でき、ラスタライザを共有できる。
- CLI `fulgur rasterize` サブコマンド（`render` を拡張しない。format 固有オプション
  の爆発を避ける）。
- OCR 学習データ crate: 公開 `LayoutOutput` ＋ 公開 `Px`/`Pt` 変換 API を使い、
  グリフ／単語／行レベルの bbox＋テキストを合成（consumer 所有）。device px =
  CSS px × scale（scale=1 で 1:1）。
- 画像での margin box / running element / target-* 対応（render-side のため画像では
  自動的に落ちる）。

## 10. リスク / 未決

- newtype 移行の波及範囲（特に Phase 1/2 の内部 call site 数）の見積りが必要。
  公開は Phase 3（最後）なので、この見積りが `layout()` 公開までのリードタイムを
  決める。
- builder の `PageSize` / `Margin` も型付き（`Pt`）にするか — config 層の単位整合と
  併せて Phase 1/2 で扱うか別途か。
- **グリフ合成は single-source 制約**（OCR の正しさに load-bearing）: OCR の価値は
  *ピクセル完全なラベル*。ラスタライズが画像 crate、ラベルが OCR crate に分かれる
  以上、グリフ配置（baseline + advance×font_size）の合成ルーチンは**両者で共有
  （single-source）されねばならない**。consumer ごとに再実装すると**ラベルがピクセル
  からずれ、学習データが静かに壊れる**。→ 「コア=変換 / consumer=合成」の線は保ちつつ、
  グリフ合成だけは薄い純関数ヘルパ（`glyph_boxes_px(&ShapedLine, origin) -> Vec<(Range, RectPx)>`
  等、Drawables を太らせない自由関数）をコアに置いて両 consumer で共有する案を有力
  候補とする（OCR 研究が固まり次第確定）。
