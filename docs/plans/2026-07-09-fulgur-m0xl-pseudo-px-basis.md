# fulgur-m0xl: Pseudo image sizing on Px basis (drop abs-pseudo round-trip)

**Goal:** `::before` / `::after` pseudo-element の `content: url(...)` 画像寸法を CSS px basis で end-to-end 保持し、abs-positioned 経路の `Px → Pt → Px` round-trip (× 0.75 / 0.75 の ULP 誤差) を削除する。

**Architecture:** `resolve_pseudo_size` / `build_pseudo_image_entry` / `build_inline_pseudo_image` の 3 関数を `Pt` basis から `Px` basis に切り替え、内部の `parent_width.in_px()` を削除する。abs-pseudo (Path A) は `AbsCb.padding_box_size: (Px, Px)` を直渡ししてラウンドトリップを消滅させ、block-pseudo (Path B) と inline-pseudo (Path C) は caller 側で `.as_pt().in_px()` に切り替えて算術位置を移動する。

**Tech Stack:** Rust, `crates/fulgur/src/units::{Px, Pt}` newtypes、Stylo `LengthPercentage::resolve`、VRT (byte-wise PDF golden)。

**Related:** beads `fulgur-m0xl` (design + acceptance)、follow-up `fulgur-bfu9` (ContentBox の Px 化、本 issue に依存)。

---

## Task 1: 3 関数の signature を `Px` basis に切り替え (atomic)

**Files:**

- Modify: `crates/fulgur/src/convert/pseudo.rs` (`resolve_pseudo_size`, `build_pseudo_image_entry`, `build_inline_pseudo_image`, `build_block_pseudo_image_entries`)
- Modify: `crates/fulgur/src/convert/positioned.rs` (`try_build_absolute_pseudo_image`)
- Modify: `crates/fulgur/src/convert/inline_root.rs:60,71` (inline pseudo call sites)
- Modify: `crates/fulgur/src/convert/list_item.rs:355,371` (list-item pseudo call sites)

**Rationale:** 部分 commit だと途中でビルドが通らないため atomic。既存 unit test は そのまま合格することを期待 (Path A は `.in_pt()` 削除で bit 差、しかし unit test は `assert!(is_some())` 中心で寸法 assertion なし → pass 予定)。VRT/example の byte-wise 比較は Task 3 で確認。

### Step 1.1: `resolve_pseudo_size` の basis を `Pt → Px` に変更

`crates/fulgur/src/convert/pseudo.rs:258` を編集:

```rust
fn resolve_pseudo_size(size: &::style::values::computed::Size, parent_width: Px) -> Option<f32> {
    use ::style::values::computed::Length;
    use ::style::values::generics::length::GenericSize;
    match size {
        GenericSize::LengthPercentage(lp) => Some(
            lp.0.resolve(Length::new(parent_width.to_f32()))
                .px()
                .as_px()
                .in_pt()
                .to_f32(),
        ),
        _ => None,
    }
}
```

変更点は 2 つのみ: `parent_width: Pt` → `parent_width: Px`、そして内部の `parent_width.in_px()` を `parent_width` (既に Px) に置換。返り値は Pt f32 のまま (下流の `make_image_entry` / `resolve_image_dimensions` は無変更)。

### Step 1.2: `build_pseudo_image_entry` の basis を `Pt → Px` に変更

`crates/fulgur/src/convert/pseudo.rs:11-32`:

```rust
pub(super) fn build_pseudo_image_entry(
    pseudo_node: &Node,
    parent_content_width: Px,
    parent_content_height: Px,
    assets: Option<&AssetBundle>,
) -> Option<crate::drawables::ImageEntry> {
    // ... body 中の parent_content_width/height の消費側 (resolve_pseudo_size 呼び出し) はそのまま
}
```

### Step 1.3: `build_inline_pseudo_image` の basis を `f32 → Px` に変更

`crates/fulgur/src/convert/pseudo.rs:167`:

```rust
pub(super) fn build_inline_pseudo_image(
    pseudo_node: &Node,
    parent_content_width: Px,
    parent_content_height: Px,
    assets: Option<&AssetBundle>,
) -> Option<InlineImage> {
    // ...
    let css_w = resolve_pseudo_size(&styles.clone_width(), parent_content_width);
    let css_h = resolve_pseudo_size(&styles.clone_height(), parent_content_height);
    // ...
}
```

`.as_pt()` の呼び出し 2 行 (line 180-181) を削除。

### Step 1.4: Path A caller (abs-pseudo) の update

`crates/fulgur/src/convert/positioned.rs:450-459` の `try_build_absolute_pseudo_image`:

```rust
    let (basis_w, basis_h) = if let Some(cb) = cb {
        cb.padding_box_size
    } else {
        (
            parent.final_layout.size.width.as_px(),
            parent.final_layout.size.height.as_px(),
        )
    };
    pseudo::build_pseudo_image_entry(pseudo, basis_w, basis_h, assets)
```

`in_pt()` を両ブランチとも削除。**この変更が Path A の round-trip 消滅点で byte-changing の直接原因**。

### Step 1.5: Path B caller (block-pseudo) の update

`crates/fulgur/src/convert/pseudo.rs:153-158` の `build_block_pseudo_image_entries`:

```rust
        let entry = build_pseudo_image_entry(
            pseudo,
            parent_cb.width.as_pt().in_px(),
            parent_cb.height.as_pt().in_px(),
            assets,
        )?;
```

`.as_pt()` (Pt tag) → `.as_pt().in_px()` (Pt tag + Px 変換)。`resolve_pseudo_size` 内から出ていた変換を caller に持ち出しただけで数値は同一 (byte-neutral 期待)。

### Step 1.6: Path C caller (inline pseudo) の update

`crates/fulgur/src/convert/inline_root.rs:60,71` (`content_box.width` は f32 Pt):

```rust
            pseudo::build_inline_pseudo_image(
                p,
                content_box.width.as_pt().in_px(),
                content_box.height.as_pt().in_px(),
                ctx.assets,
            )
```

同じ pattern を `crates/fulgur/src/convert/list_item.rs:355-360, 371-376` にも適用。

### Step 1.7: ビルドと lib test を実行

```bash
cargo build -p fulgur 2>&1 | tail -5
cargo test -p fulgur --lib 2>&1 | tail -20
cargo clippy -p fulgur --all-targets -- -D warnings 2>&1 | tail -10
cargo fmt --check 2>&1 | tail -5
```

Expected: 全 pass (~340 lib test)。もし unit test が寸法 assert で失敗した場合は Path A のケースかどうか確認し、byte-changing の意図を test に反映するか、テストデータを Px 直接使用に修正する。

### Step 1.8: Commit

```bash
git add -A
git commit -m "refactor(convert): thread Px basis into pseudo image sizing (fulgur-m0xl)

resolve_pseudo_size / build_pseudo_image_entry / build_inline_pseudo_image
now take a units::Px basis instead of Pt.

- Path A (abs-positioned pseudo via AbsCb.padding_box_size): the caller
  used to do Px -> Pt then resolve_pseudo_size did Pt -> Px. Both hops
  are removed and the Px value is passed straight through. This is NOT
  byte-neutral: the 0.75 * (1/0.75) round-trip is deleted.
- Path B (block pseudo via ContentBox { width: f32 in Pt }): the
  Pt -> Px conversion is moved from inside resolve_pseudo_size to the
  caller. Same arithmetic, same result -> byte-neutral.
- Path C (inline pseudo via f32 in Pt from inline_root / list_item):
  same treatment as Path B.

Refs: fulgur-m0xl
"
```

---

## Task 2: VRT / determinism / smoke の byte 差分を計測

### Step 2.1: 現状の VRT を実行し diff を確認

```bash
FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf" cargo test -p fulgur-vrt 2>&1 | tail -30
```

Expected: 事前調査より VRT fixtures には `position: absolute` × `content: url(...)` の組み合わせが不在なので、golden の byte 差分は原理上 **ゼロ**。差分が出た場合は Path B/C の byte-neutral 仮説が破れているか、想定外の code path (e.g. list-marker) が影響しているサインなので、`git diff crates/fulgur-vrt/goldens/` で差分ファイルを特定し原因を調査する。

もし unexpected な golden 差分が出た場合:

- `pdftocairo` で差分 golden を PNG 化して比較 (`crates/fulgur-vrt` の diff 生成機構を利用)
- 差分が abs-pseudo image golden **以外** に及んでいたら Path B/C 想定と実測の齟齬 → 実装を見直す

### Step 2.2: CLI examples_determinism を確認

```bash
FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf" cargo test -p fulgur-cli --test examples_determinism 2>&1 | tail -30
```

Expected: `examples/pseudo-content-url/` は abs positioned pseudo image を含まないため pass。差分が出た場合は `mise run examples:update` (もしくは同等スクリプト) で再生成し、diff が abs-pseudo 系に限定されていることを確認。

### Step 2.3: smoke test を full run

```bash
cargo test -p fulgur --tests 2>&1 | tail -10
```

Expected: 全 pass。

### Step 2.4: (必要な場合のみ) VRT golden を re-bless

VRT の差分が Path A 関連の golden に限定されていることを目視確認した上で:

```bash
FULGUR_VRT_UPDATE=1 FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf" cargo test -p fulgur-vrt
git diff --stat crates/fulgur-vrt/goldens/
```

差分は本タスクの scope 内の再生成のみのはず。commit は Task 1 とは分離する:

```bash
git add crates/fulgur-vrt/goldens/
git commit -m "test(fulgur-vrt): re-bless abs-pseudo image goldens after fulgur-m0xl

Path A pseudo image sizing dropped the Px->Pt->Px round-trip, causing
ULP-level dimension shifts in abs-positioned pseudo-image goldens.
Only <listed golden paths> are affected.

Refs: fulgur-m0xl
"
```

### Step 2.5: (差分ゼロの場合) VRT re-bless を skip

VRT/examples とも差分ゼロなら Task 2 の commit は不要。plan note にその旨を記載してタスク終了。

---

## Task 3: abs-pseudo image の smoke test を追加 (regression safety net)

**Files:**

- Modify: `crates/fulgur/tests/render_smoke.rs`

**Rationale:** VRT/examples が abs-positioned pseudo image を覆っていない現状、Path A の round-trip 除去がリグレッションで再燃したときに検出できるように、`render_v2_smoke_abs_pseudo_image_no_round_trip` に類する smoke test を追加する。寸法 assertion まで踏み込むと fragile なので、Engine::render_html が非空 PDF を返すことのみ確認する CLAUDE.md 記載の smoke test 型に合わせる。

### Step 3.1: 既存 smoke で覆われているか探す

`crates/fulgur/tests/render_smoke.rs` で `position:\s*absolute.*content:\s*url|content:\s*url.*position:\s*absolute` に該当する test がないことを確認 (事前調査済み、なし)。

### Step 3.2: 新規 smoke test を追加

`crates/fulgur/tests/render_smoke.rs` の pseudo image 関連セクション (line 1800 付近) に追加:

```rust
#[test]
fn render_v2_smoke_abs_positioned_pseudo_image() {
    // Targets `convert/positioned.rs::try_build_absolute_pseudo_image`
    // (Path A of pseudo image sizing). Ensures that the Px-basis rewrite
    // in fulgur-m0xl -- which removed the Px→Pt→Px round-trip -- keeps
    // rendering non-empty PDFs for abs-positioned pseudo elements whose
    // computed content resolves to a single content: url() image.
    let mut bundle = AssetBundle::default();
    bundle.add_image("dot.png", PR290_PSEUDO_PNG.to_vec());
    let html = r#"<!DOCTYPE html><html><head><style>
        .parent { position: relative; width: 100pt; height: 40pt; }
        .parent::before {
            content: url("dot.png");
            position: absolute;
            top: 4pt;
            left: 4pt;
            width: 8pt;
            height: 8pt;
        }
    </style></head><body>
        <div class="parent"></div>
    </body></html>"#;
    let pdf = Engine::builder()
        .assets(bundle)
        .build()
        .render(html)
        .expect("v2 render");
    assert!(!pdf.is_empty());
}
```

### Step 3.3: 実行して pass 確認

```bash
cargo test -p fulgur --test render_smoke render_v2_smoke_abs_positioned_pseudo_image 2>&1 | tail -10
```

Expected: pass。

### Step 3.4: Commit

```bash
git add crates/fulgur/tests/render_smoke.rs
git commit -m "test(render_smoke): add abs-positioned pseudo image smoke (fulgur-m0xl)

VRT fixtures don't currently cover position: absolute pseudo elements
with content: url(...) image. Add a lib-level smoke test so that the
Path A code path (try_build_absolute_pseudo_image with Px basis) has a
regression net independent of golden re-blessing.

Refs: fulgur-m0xl
"
```

---

## Verification checklist (before PR)

- [ ] `cargo test -p fulgur --lib`, `cargo test -p fulgur --tests`, `cargo clippy -p fulgur --all-targets -- -D warnings`, `cargo fmt --check` すべて pass
- [ ] `cargo test -p fulgur-vrt` pass (差分が出た場合は re-bless commit が abs-pseudo 系のみを触っていること)
- [ ] `cargo test -p fulgur-cli --test examples_determinism` pass
- [ ] `bd show fulgur-m0xl` の acceptance criteria が全て満たされている
- [ ] commit 分割: (1) refactor (Task 1)、(2) VRT re-bless (Task 2、差分ゼロなら省略)、(3) smoke test (Task 3)
