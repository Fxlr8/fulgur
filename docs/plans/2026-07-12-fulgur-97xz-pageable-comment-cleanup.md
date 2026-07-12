# fulgur-97xz: Pageable Comment Cleanup Plan

**Goal:** Pageable v1 アーキテクチャ廃止後の残存参照コメント 128 箇所 / 27 ファイルを `crates/fulgur/{src,tests}/` から一掃する。behavior 変更なし、byte-identical。

**Approach:** 5 モジュール束の PR に分割し、小→大の順で実行 (低リスク PR を先に流通させ、判断コスト高な `render.rs` (42) / `pagination_layout.rs` (29) は後半に集約)。各 PR は独立してレビュー可能、全 PR 共通の分類プロトコルを適用。

**Doc-comment (doctest) 対策:** `///` および `//!` の doc comment 内にも Pageable 参照あり (`drawables.rs` の module doc、`pagination_layout.rs` の複数箇所、`tests/opacity_visibility_test.rs`)。全 PR で `cargo test --doc` を検証に追加する。

**関連:** beads fulgur-97xz (design/acceptance 保存済)、前提: fulgur-4cbc (Pageable 廃止)。

---

## 分類プロトコル (全 PR 共通)

各コメントに対し 1 回だけ判定し、以下いずれかに割り当てる。

### Category 1: Pure v1 parity comment → **削除**

対象消滅で意味を失った comment。他モジュール参照の parity 説明が典型。

```rust
// Before
// Mirrors BlockPageable::draw ordering at pageable.rs:412
draw_background(...);

// After
draw_background(...);
```

### Category 2: v1→v2 移行進行中前提 → **削除 or 現状表現に書き換え**

移行過渡期の TODO / plan コメント。現在は移行完了しているので削除、または現状事実に書き換え。

```rust
// Before
// TODO: migrate one Pageable type at a time and the dispatcher grows
fn render_v2(...) { ... }

// After
fn render_v2(...) { ... }
```

### Category 3: v1 順序制約が behavior の WHY → **CSS spec / 不変式に書き換え**

順序や不変式の理由を語るコメントは残す価値がある。参照先を v1 Pageable/pageable.rs ではなく CSS spec または不変式そのものに差し替える。深追いはしない (1-2 行で十分)。

```rust
// Before
// v1's BlockPageable::draw paints outside the clip (pageable.rs:1796-1827),
// then pushes the clip path, then dispatches self's inner content.
draw_background(...);
draw_borders(...);
push_clip(...);
draw_content(...);

// After
// Paint bg/border outside the clip, then push clip, then dispatch contents
// (overflow: hidden/clip painting order per CSS 2.1 §11.1.1).
draw_background(...);
draw_borders(...);
push_clip(...);
draw_content(...);
```

**render.rs の Cat 3 典型パターン** (先行サンプル):

- `v1's BlockPageable::draw for root paints bg on EVERY page` → `Root <html>/<body> background repeats per page (CSS Paged Media §5.4).`
- `Mirrors BookmarkMarkerWrapperPageable's is_first_page_for slice semantics` → `Emit bookmark on the page where the node's first fragment lands.`
- `MulticolRulePageable::draw runs AFTER child.draw` → `Column rules paint after column content (CSS Multi-column §4.5).`
- `Mirrors v1's nested TransformWrapperPageable::draw call chain` → `Nested transforms compose right-to-left; each level pushes its own matrix (CSS Transforms §11).`

**pagination_layout.rs の Cat 3 典型パターン**:

- `Pageable accumulates pc.y + child_h during convert; fragmenter must match` → `Include child margin gaps in body's normal-flow height (CSS 2.1 §10.6.3).`
- `Pageable's total_height > page_height override at pageable.rs:1165` → `break-inside: avoid is overridden when subtree is oversized (CSS Fragmentation §4.2).`

### 判定に迷ったら

- **Category 1 に倒す** (削除)。読み手のノイズを減らす方が優先。
- WHY を語り直す価値があるか自信が持てない comment は削除。

---

## 全 PR 共通の検証

各 PR 完成後、以下を全部通す。

```bash
cargo build --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p fulgur --lib
cargo test -p fulgur --tests
cargo test -p fulgur --doc
FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf" cargo test -p fulgur-vrt
cargo fmt --check
```

**VRT が byte-identical であること** が最重要 tripwire。コメントは compile 時に stripped されるので PDF 出力を変えることは技術的に不可能 → VRT に差分が出た場合、subagent がコード行を触った or doc comment 内の ```rust コード塊を破壊したことを意味する。

**手動 diff 確認**: 各 PR で `git diff main --stat` と該当ファイルの `git diff main` を目視し、コメント/doc 行のみ変更されていることを確認する (2-stage adversarial review は skip、diff は trivially eyeballable)。

---

## PR 1: orchestration + tests + misc

**Branch:** `task/fulgur-97xz-pageable-1-misc` (このブランチで作業中)

**Files (33 箇所):**

```text
crates/fulgur/src/engine.rs                  (4)
crates/fulgur/src/blitz_adapter.rs           (3)
crates/fulgur/src/gcpm/string_set.rs         (2)
crates/fulgur/src/svg.rs                     (1)
crates/fulgur/src/image.rs                   (1)
crates/fulgur/src/draw_primitives.rs         (1)
crates/fulgur/src/column_css.rs              (1)
crates/fulgur/tests/abs_positioned_pagination.rs (5)
crates/fulgur/tests/style_test.rs            (2)
crates/fulgur/tests/render_smoke.rs          (2)
crates/fulgur/tests/opacity_visibility_test.rs (2)
crates/fulgur/tests/unit_semantics.rs        (1)
crates/fulgur/tests/transform_integration.rs (1)
crates/fulgur/tests/svg_test.rs              (1)
crates/fulgur/tests/pseudo_only_break_before.rs (1)
crates/fulgur/tests/page_break_wiring.rs     (1)
crates/fulgur/tests/multicol_span_all.rs     (1)
crates/fulgur/tests/break_inside_avoid.rs    (1)
```

**手順:**

1. `rg -n 'Pageable\b' <file>` で各ファイルの該当行を出す
2. 上下 3 行の context を読んで Category 1/2/3 を判定
3. Category 1/2 は削除、Category 3 は書き換え
4. 全ファイル処理後、`rg 'Pageable\b' crates/fulgur/src crates/fulgur/tests` で **0 件** を確認
5. 検証コマンド全部通す
6. commit
7. PR 作成 (`gh pr create`, base: main)

**コミットメッセージ:**

```text
chore(comments): remove Pageable v1 references (orchestration + tests + misc)

fulgur-97xz PR 1/5. Removes 33 historical comments referencing the
removed v1 Pageable architecture (fulgur-4cbc) across engine, adapter,
misc render helpers, and integration tests. Behavior byte-identical
(VRT green).
```

---

## PR 2: convert bundle

**Branch base:** main (`git checkout main && git pull && git checkout -b task/fulgur-97xz-pageable-2-convert`)

**Files (10 箇所):**

```text
crates/fulgur/src/convert/mod.rs          (4)
crates/fulgur/src/convert/inline_root.rs  (4)
crates/fulgur/src/convert/list_marker.rs  (1)
crates/fulgur/src/convert/block.rs        (1)
```

**手順:** PR 1 と同じ (分類 → 削除/書き換え → 検証 → commit → PR)。

**コミットメッセージ:**

```text
chore(comments): remove Pageable v1 references (convert bundle)

fulgur-97xz PR 2/5. Removes 10 historical comments in convert/*
referencing the removed v1 Pageable architecture. Behavior
byte-identical.
```

---

## PR 3: draw path bundle

**Branch base:** main (`git checkout main && git pull && git checkout -b task/fulgur-97xz-pageable-3-draw`)

**Files (16 箇所):**

```text
crates/fulgur/src/drawables.rs        (7)
crates/fulgur/src/paragraph.rs        (5)
crates/fulgur/src/multicol_layout.rs  (4)
```

**手順:** PR 1 と同じ。

**コミットメッセージ:**

```text
chore(comments): remove Pageable v1 references (draw path bundle)

fulgur-97xz PR 3/5. Removes 16 historical comments in drawables /
paragraph / multicol_layout referencing the removed v1 Pageable
architecture. Behavior byte-identical.
```

---

## PR 4: pagination bundle

**Branch base:** main (`git checkout main && git pull && git checkout -b task/fulgur-97xz-pageable-4-pagination`)

**Files (29 箇所):**

```text
crates/fulgur/src/pagination_layout.rs  (29)
```

**手順:** PR 1 と同じ。pagination_layout.rs は v2 パスの本体なので、Category 3 (WHY を語り直す) の comment が多めに残る想定。CSS spec 参照 (fragmentation §3, §4 系) を活用。

**コミットメッセージ:**

```text
chore(comments): remove Pageable v1 references (pagination_layout)

fulgur-97xz PR 4/5. Removes/rewrites 29 historical comments in
pagination_layout.rs referencing the removed v1 Pageable architecture.
CSS Fragmentation spec references replace v1 parity notes where the
"why" is still worth stating. Behavior byte-identical.
```

---

## PR 5: render bundle

**Branch base:** main (`git checkout main && git pull && git checkout -b task/fulgur-97xz-pageable-5-render`)

**Files (42 箇所):**

```text
crates/fulgur/src/render.rs  (42)
```

**手順:** PR 1 と同じ。render.rs は draw 順序制約の説明が多いため Category 3 が支配的な想定。

**コミットメッセージ:**

```text
chore(comments): remove Pageable v1 references (render)

fulgur-97xz PR 5/5. Removes/rewrites 42 historical comments in
render.rs referencing the removed v1 Pageable architecture. CSS 2.1
painting-order references replace v1 parity notes. Behavior
byte-identical. Closes fulgur-97xz.
```

---

## 完了判定

- `rg 'Pageable\b' crates/fulgur/src crates/fulgur/tests` が 0 件
- 5 PR 全て merge 済 (main で `rg 'Pageable\b' crates/fulgur/src crates/fulgur/tests` = 0 件を確認)
- fulgur-97xz を `bd close` で閉じる (PR 5 の commit message に "Closes fulgur-97xz" を含める)
