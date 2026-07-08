# fulgur-bfu9: ContentBox width/height as units::Px end-to-end

**Goal:** `convert::ContentBox` の 4 フィールドを全て `units::Px` に型付けし、`compute_content_box` を Px-native に書き直す。fulgur-m0xl の consumer 側に残った `.as_pt().in_px()` の付け足し変換を全部削除する。

**Architecture:** ContentBox の内部単位を Pt f32 → Px に切り替える。compute は Taffy の final_layout.size を `.as_px()` で直取り、insets は `border_widths[i].in_px() + padding[i].in_px()` で Px 化してから減算する。消費側 (`inline_root`, `list_item`, `pseudo::build_block_pseudo_image_entries`) は `content_box.width` / `.height` を Px として直渡し。

**Tech Stack:** Rust, `crates/fulgur/src/units::{Px, Pt}` newtypes、Stylo `LengthPercentage::resolve`、VRT (PDF byte-wise golden) と `examples_determinism` (CLI 例の PDF snapshot)。

**Related:** beads `fulgur-bfu9` (design + acceptance)、prerequisite `fulgur-m0xl` (merged in PR #614)。

---

## Task 1: ContentBox の Px 化 (atomic refactor)

**Files:**

- Modify: `crates/fulgur/src/convert/mod.rs` (`ContentBox` 定義、`compute_content_box`)
- Modify: `crates/fulgur/src/convert/pseudo.rs` (`build_block_pseudo_image_entries`)
- Modify: `crates/fulgur/src/convert/inline_root.rs` (2 pseudo call sites)
- Modify: `crates/fulgur/src/convert/list_item.rs` (2 pseudo call sites)

**Rationale:** 部分 commit だと intermediate state がコンパイル失敗するため atomic。

### Step 1.1: `ContentBox` の 4 フィールドを `Px` に

`crates/fulgur/src/convert/mod.rs:988-995`:

```rust
#[derive(Clone, Copy)]
#[allow(dead_code)]
struct ContentBox {
    origin_x: Px,
    origin_y: Px,
    width: Px,
    height: Px,
}
```

### Step 1.2: `compute_content_box` を Px-native に書き直し

`crates/fulgur/src/convert/mod.rs:998-1009`:

```rust
fn compute_content_box(node: &Node, style: &BlockStyle) -> ContentBox {
    let left_inset = style.border_widths[3].in_px() + style.padding[3].in_px();
    let top_inset = style.border_widths[0].in_px() + style.padding[0].in_px();
    let right_inset = style.border_widths[1].in_px() + style.padding[1].in_px();
    let bottom_inset = style.border_widths[2].in_px() + style.padding[2].in_px();
    let border_w = node.final_layout.size.width.as_px();
    let border_h = node.final_layout.size.height.as_px();
    ContentBox {
        origin_x: left_inset,
        origin_y: top_inset,
        width: (border_w - left_inset - right_inset).max(Px::ZERO),
        height: (border_h - top_inset - bottom_inset).max(Px::ZERO),
    }
}
```

`BlockStyle.content_inset()` の f32-in-Pt 経路は本経路では未使用に (他 caller は無変更)。`size_in_pt` は本 fn からのみ使用外に。他所で残るので削除しない。

### Step 1.3: `pseudo.rs::build_block_pseudo_image_entries` の `.as_pt().in_px()` 削除

`crates/fulgur/src/convert/pseudo.rs:153-158`:

```rust
        let entry = build_pseudo_image_entry(
            pseudo,
            parent_cb.width,
            parent_cb.height,
            assets,
        )?;
```

fulgur-m0xl で追加した `.as_pt().in_px()` を消して直渡し。

### Step 1.4: `inline_root.rs` の pseudo call sites を直渡しに

`crates/fulgur/src/convert/inline_root.rs:60,71` (現在 4 行 `.as_pt().in_px()`):

```rust
            pseudo::build_inline_pseudo_image(
                p,
                content_box.width,
                content_box.height,
                ctx.assets,
            )
```

before/after 両方の呼び出しで同様に。

### Step 1.5: `list_item.rs` の pseudo call sites を直渡しに

`crates/fulgur/src/convert/list_item.rs:355-360, 371-376`: 同じパターンで `.as_pt().in_px()` 削除。

### Step 1.6: ビルドと lib test

```bash
cargo build -p fulgur
cargo test -p fulgur --lib
cargo clippy -p fulgur --all-targets -- -D warnings
cargo fmt --check
```

Expected: 全 pass (fulgur-m0xl と同様、unit test は寸法 shape の assertion 中心なので pass 予定)。

### Step 1.7: Commit

```
refactor(convert): type ContentBox width/height as units::Px (fulgur-bfu9)

ContentBox now holds units::Px for origin_x / origin_y / width / height.
compute_content_box is rewritten in Px space:

- final_layout.size is tagged Px directly (was tagged Px then converted
  to Pt via size_in_pt)
- insets are converted Pt->Px individually and summed in Px

Consumers (build_inline_pseudo_image callers in inline_root/list_item
and build_block_pseudo_image_entries in pseudo.rs) drop the
`.as_pt().in_px()` stopgap added in fulgur-m0xl and pass the Px values
straight through.

This is NOT byte-neutral: float operation order changes from
  (Px * 0.75 - Pt - Pt) / 0.75    (old Pt-space compute, Px cast at
                                    consumer)
to
  Px - Pt / 0.75 - Pt / 0.75      (new Px-native compute)

so ULP-level dimension diffs are expected in pseudo image / list marker
paths that flow through ContentBox. VRT / examples goldens re-blessed
in the following commit.

Refs: fulgur-bfu9
```

---

## Task 2: VRT / examples の byte diff を計測して re-bless

### Step 2.1: VRT を実行、diff scope を確認

```bash
FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf" cargo test -p fulgur-vrt 2>&1 | tail -30
```

Expected: fail (goldens が更新される必要あり)。`git diff --stat crates/fulgur-vrt/goldens/` で差分ファイルを列挙し、pseudo image / list marker / block padding 系に限定されていることを確認する。もし範囲外の golden に diff が及んでいたら意図しない side effect の可能性 → 停止して調査。

### Step 2.2: VRT goldens を re-bless

```bash
FULGUR_VRT_UPDATE=1 FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf" cargo test -p fulgur-vrt 2>&1 | tail -10
git diff --stat crates/fulgur-vrt/goldens/
git diff --stat --numstat crates/fulgur-vrt/goldens/ | head -20
```

### Step 2.3: examples_determinism を確認、失敗すれば regenerate

```bash
FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf" cargo test -p fulgur-cli --test examples_determinism 2>&1 | tail -30
```

Expected: `examples/pseudo-content-url/` / `examples/list-style-image/` の snapshot が動く可能性。失敗すれば `mise run examples:update` (相当のスクリプト) で PDF を再生成する。

### Step 2.4: (任意) pdftocairo で目視差分を確認

代表 golden で PDF を PNG 化し、before/after の見た目に差が無いことを目視:

```bash
pdftocairo -png crates/fulgur-vrt/goldens/fulgur/<affected>.pdf /tmp/after.png
git show HEAD:crates/fulgur-vrt/goldens/fulgur/<affected>.pdf > /tmp/before.pdf
pdftocairo -png /tmp/before.pdf /tmp/before.png
# 目視 diff — ULP レベルなら PNG は同一 or 極小差
```

### Step 2.5: Commit re-bless

```bash
git add crates/fulgur-vrt/goldens/
git add examples/*/  # examples PDF が更新された場合
git commit -m "test(fulgur-vrt): re-bless goldens after ContentBox → Px (fulgur-bfu9)

ContentBox のプロパティを Px-native 計算にした結果、pseudo image と
list marker 系の goldens が ULP レベルで更新される。差分は <listed
paths> に限定。目視上の見た目は変わっていない。

Refs: fulgur-bfu9
"
```

範囲外の golden 差分が出た場合は commit 前に **必ず** 調査すること。

---

## Task 3: Full verification

### Step 3.1: 全 test suite

```bash
cargo test -p fulgur --lib
cargo test -p fulgur --tests
cargo test -p fulgur-vrt
FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf" cargo test -p fulgur-cli --test examples_determinism
cargo clippy -p fulgur --all-targets -- -D warnings
cargo fmt --check
```

### Step 3.2: fulgur-m0xl の smoke test も pass 継続を確認

```bash
cargo test -p fulgur --test render_smoke render_v2_smoke_abs_positioned_pseudo_image
```

Expected: pass (m0xl で追加した Path A 用 smoke)。ContentBox 変更が abs pseudo 経路に副作用を起こしていないことの確認。

---

## Verification checklist (before PR)

- [ ] `cargo test -p fulgur --lib` / `cargo test -p fulgur --tests` / `cargo clippy` / `cargo fmt --check` 全 pass
- [ ] `cargo test -p fulgur-vrt` pass (re-bless commit 後)
- [ ] `cargo test -p fulgur-cli --test examples_determinism` pass
- [ ] VRT / examples の re-bless diff scope が pseudo image / list marker 系に限定 (block padding 経路も可)
- [ ] pdftocairo で代表 golden の見た目差が無いこと確認済み
- [ ] `bd show fulgur-bfu9` の acceptance criteria が全て満たされている
- [ ] commit 分割: (1) refactor (Task 1)、(2) VRT / examples re-bless (Task 2)

## Post-implementation note

Task 2 の byte diff 計測は **実測でゼロ**。原因:

- VRT fixtures には `content: url()` を持つ pseudo-element の fixture が現状不在
- `examples/pseudo-content-url/` の全 `::before` / `::after` は `width: NNpx` / `height: NNpx` の絶対長指定で、`LengthPercentage::resolve(basis)` が basis に依存しないため、basis を Pt→Px に切り替えても解決結果が bit-identical
- `examples/list-style-image/` は `list-style-image` プロパティ経由 (別コードパス) で ContentBox の consumer ではない

結果として Task 2 の VRT re-bless / examples 再生成の commit は不要となり、Task 1 の refactor commit + doc polish のみで完了。基底切替の byte-changing 潜在はコード上には残るが (percentage-based pseudo sizing で顕在化する)、`smoke tests` / roborev で保護される。
