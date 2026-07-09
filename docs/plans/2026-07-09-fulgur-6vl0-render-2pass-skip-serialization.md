# fulgur-6vl0: Skip Wasted Pass-1 render_v2 in Engine::render Implementation Plan

**Goal:** `Engine::render()` の pass 1 で不要な `render_v2` (PDF シリアライズ) を実行しない。`target-counter` / `target-counters` / `target-text` を使う 2-pass ドキュメントで PDF シリアライズ呼び出しが 1 回減る。バイト同一の出力を維持。

**Architecture:** `render()` を `layout()` の 2-pass パターンに揃える。`render_pass` と `RenderPassOutput` は `render()` 内部からしか使われていないので、両方削除し、`render_v2` 呼び出しを `render_artifacts(artifacts, anchor_map)` ヘルパに切り出す。pass 1 は `layout_to_drawables` だけ、pass 2 (存在すれば) も `layout_to_drawables`、最終ステップだけ `render_artifacts` を叩く。

**Tech Stack:** Rust, cargo, `crates/fulgur/src/engine.rs` 中心。関連: `crates/fulgur/src/render.rs` の doc 参照 1 箇所。

**Beads issue:** `fulgur-6vl0` (design/acceptance は issue に保存済)

---

## Baseline

- worktree: `.worktrees/fulgur-6vl0-render-2pass` (branch `task/fulgur-6vl0-render-2pass`)
- baseline: `cargo test -p fulgur --lib` → 1674 passed, 0 failed

---

## Task 1: Extract `render_artifacts` helper (behavior-preserving)

**Files:**
- Modify: `crates/fulgur/src/engine.rs:702-740` (`fn render_pass`)

**Rationale:** まず `render_pass` の中身から `render_v2` 呼び出しを `render_artifacts` に切り出し、`render_pass` はそれを呼ぶ薄いラッパにする。この段階では挙動は変わらず、`render()` のリファクタと分離してテストできる。

**Step 1: `render_artifacts` を engine.rs に追加**

`layout_to_drawables` と `render_pass` の間 (engine.rs:697 の空行後、`render_pass` の直前) に以下を追加:

```rust
    /// Serialize a fully-laid-out [`LayoutArtifacts`] into a PDF via
    /// [`render::render_v2`]. Callers pass `anchor_map = Some(&pass1_map)`
    /// on pass 2 of a 2-pass render so `target-*` resolvers substitute
    /// real values; on the 1-pass path (no `target-*`) they pass `None`.
    fn render_artifacts(
        &self,
        artifacts: LayoutArtifacts,
        anchor_map: Option<&AnchorMap>,
    ) -> Result<Vec<u8>> {
        let LayoutArtifacts {
            drawables,
            pagination_geometry,
            gcpm,
            running_store,
            string_set_for_render,
            counter_ops_for_render,
            html_title,
            implicit_href_map,
            ..
        } = artifacts;
        let fonts = self.fonts();
        crate::render::render_v2(
            &self.config,
            &pagination_geometry,
            &drawables,
            &gcpm,
            &running_store,
            fonts,
            self.system_fonts,
            &string_set_for_render,
            &counter_ops_for_render,
            html_title,
            self.serialize_settings.clone(),
            anchor_map,
            &implicit_href_map,
        )
    }
```

**Step 2: `render_pass` の中身を書き換えて `render_artifacts` を経由させる**

現行の `render_pass` (engine.rs:702-740) を以下に置き換え:

```rust
    /// Single render pass: lay out via [`layout_to_drawables`], then serialize
    /// via [`render_artifacts`]. `render`'s 2-pass loop is untouched. See
    /// [`RenderPassOutput`] for the returned fields.
    fn render_pass(&self, html: &str, anchor_map: Option<&AnchorMap>) -> Result<RenderPassOutput> {
        let artifacts = self.layout_to_drawables(html, anchor_map)?;
        let needs_pass_two = artifacts.needs_pass_two;
        let collected_anchor_map = std::mem::take(&mut { artifacts }.collected_anchor_map);
        // NOTE: rewrite below to avoid double-take — see Step 3
    }
```

これは書きにくいので、代わりに以下の形にする (Step 3 参照)。

**Step 3: `render_pass` を実際に置き換える**

```rust
    /// Single render pass: lay out via [`layout_to_drawables`], then serialize
    /// via [`render_artifacts`]. See [`RenderPassOutput`] for the returned fields.
    fn render_pass(&self, html: &str, anchor_map: Option<&AnchorMap>) -> Result<RenderPassOutput> {
        let mut artifacts = self.layout_to_drawables(html, anchor_map)?;
        let needs_pass_two = artifacts.needs_pass_two;
        let collected_anchor_map = std::mem::take(&mut artifacts.collected_anchor_map);
        let pdf = self.render_artifacts(artifacts, anchor_map)?;
        Ok(RenderPassOutput {
            pdf,
            anchor_map: collected_anchor_map,
            needs_pass_two,
        })
    }
```

`std::mem::take` を使うのは、`artifacts` を `render_artifacts` に move する前に `collected_anchor_map` だけ抜き出すため。`AnchorMap: Default` は既存 (`AnchorMap::default()` が engine.rs:605 で使われている) なので `mem::take` 可。

**Step 4: `cargo test -p fulgur --lib` を走らせて 1674 pass を確認**

Run: `cargo test -p fulgur --lib`
Expected: 1674 passed; 0 failed (Task 1 は behavior-preserving なので変わらない)

**Step 5: commit**

```bash
git add crates/fulgur/src/engine.rs
git commit -m "refactor(engine): extract render_artifacts helper from render_pass (fulgur-6vl0)"
```

---

## Task 2: Rewrite `render()` and delete `render_pass` / `RenderPassOutput`

**Files:**
- Modify: `crates/fulgur/src/engine.rs:141-159` (`fn render`)
- Delete: `crates/fulgur/src/engine.rs:22-38` (`struct RenderPassOutput` + doc comment)
- Delete: `crates/fulgur/src/engine.rs:698-716` (`fn render_pass` in its Task-1 form)

**Step 1: `render()` を `layout()` パターンに書き換える**

現行の `render()` (engine.rs:141-159) を以下に置き換え:

```rust
    pub fn render(&self, html: &str) -> Result<Vec<u8>> {
        // Pass 1: layout only. `layout_to_drawables` parses the full GCPM
        // context (AssetBundle, <link>-loaded stylesheets, inline <style>)
        // and reports `needs_pass_two` based on that parsed view, so
        // `target-counter()` / `target-counters()` / `target-text()`
        // declared in any of those locations is detected reliably.
        //
        // Unlike a prior `render_pass`-based implementation, we do NOT run
        // `render_v2` here — its output would be discarded on the 2-pass
        // path (fulgur-6vl0). This mirrors `layout()`'s 2-pass loop.
        let pass1 = self.layout_to_drawables(html, None)?;
        if !pass1.needs_pass_two {
            return self.render_artifacts(pass1, None);
        }
        // Pass 2: re-lay-out with the pass-1 AnchorMap so `target-*`
        // resolvers substitute resolved values, and serialize once.
        let LayoutArtifacts {
            collected_anchor_map: anchor_map,
            ..
        } = pass1;
        let pass2 = self.layout_to_drawables(html, Some(&anchor_map))?;
        self.render_artifacts(pass2, Some(&anchor_map))
    }
```

**Step 2: `render_pass` と `RenderPassOutput` を削除**

- `engine.rs:22-38` (`RenderPassOutput` struct + その上の doc comment) を丸ごと削除。
- Task 1 で書き換えた `render_pass` (engine.rs:697-716 相当、行番号は Task 1 後の状態) を丸ごと削除。
- `RenderPassOutput` を import しているものがあれば削る (現状ではモジュール内 struct なので import なし)。

**Step 3: cargo check を走らせて未使用参照を確認**

Run: `cargo check -p fulgur 2>&1 | tail -20`
Expected: warnings なし、error なし

**Step 4: `cargo test -p fulgur --lib` を走らせて 1674 pass を確認**

Run: `cargo test -p fulgur --lib`
Expected: 1674 passed; 0 failed (中でも `render_target_counter_in_margin_box_triggers_two_pass` が 2-pass 経路を叩くので pass することが重要)

**Step 5: commit**

```bash
git add crates/fulgur/src/engine.rs
git commit -m "perf(engine): skip wasted pass-1 render_v2 in Engine::render (fulgur-6vl0)"
```

---

## Task 3: Update stale doc references to `render_pass`

**Files:**
- Modify: `crates/fulgur/src/render.rs:3429` (doc comment)
- Modify: `crates/fulgur/src/engine.rs` — Task 1/2 の書き換え中に既に更新済のはずだが、grep で残存確認する

**Step 1: `crates/fulgur/src/render.rs:3429` を更新**

現行:
```
/// boxes. Built once per render pass by `engine::render_pass`.
```

を以下に置き換え:
```
/// boxes. Built once per render pass by `engine::render_artifacts`.
```

**Step 2: 全ソースツリーで `render_pass` 残存を検索**

Run: `rg -n "render_pass|RenderPassOutput" crates/fulgur/src/`
Expected: 何も出力されない (完全削除の確認)

もし何か残っていたらそのファイルを開いて `render_artifacts` (該当箇所によっては `render` / `layout_to_drawables`) に修正。

**Step 3: `cargo check -p fulgur` で doc test も含めて警告なしを確認**

Run: `cargo check -p fulgur 2>&1 | tail -10`
Expected: warnings なし

**Step 4: commit**

```bash
git add crates/fulgur/src/render.rs
git commit -m "docs(engine): update stale render_pass references to render_artifacts (fulgur-6vl0)"
```

---

## Task 4: Byte-identical determinism verification

**Purpose:** リファクタが本当に byte-identical output を保っているか確認する最終ゲート。

**Step 1: fulgur unit + integration test 全走**

Run: `cargo test -p fulgur`
Expected: すべて pass

**Step 2: fulgur-cli determinism goldens**

Run: `cargo test -p fulgur-cli --test examples_determinism`
Expected: すべて pass (byte-wise 比較なので、リファクタで golden が変わっていたら失敗)

**Step 3: fulgur-vrt reftests (VRT golden byte 比較)**

Run: `FONTCONFIG_FILE="$PWD/examples/.fontconfig/fonts.conf" cargo test -p fulgur-vrt`
Expected: すべて pass

**Step 4: 手動 sanity — target-counter を使う HTML を新旧ブランチで render し diff**

以下の最小 HTML を用意して byte-diff 比較:

```html
<!DOCTYPE html><html><head><style>
  @page { @bottom-center { content: "Page " counter(page) " of " target-counter(url("#end"), page); } }
</style></head>
<body>
  <p>Page 1 content</p>
  <div style="break-before: page"></div>
  <p id="end">End</p>
</body></html>
```

Run:
```bash
cd /home/ubuntu/fulgur/.worktrees/fulgur-6vl0-render-2pass
# 現ブランチで render
cargo run --release --bin fulgur -- render /tmp/target-counter.html -o /tmp/new.pdf
# main で render (別 worktree または一時的に checkout せず、事前に生成しておく)
cmp /tmp/new.pdf /tmp/old.pdf && echo "byte-identical" || echo "DIFFERS"
```

Expected: `byte-identical`

`/tmp/old.pdf` は main で事前生成しておく (実際の手順は実行時に決める。もし面倒なら Step 1-3 の自動テスト成功で十分と判断してもよい)。

**Step 5: 完了報告**

すべて pass なら、この plan の実装は完了。beads acceptance criteria がすべて満たされたことを確認。

---

## Rollback Plan

問題があれば `task/fulgur-6vl0-render-2pass` ブランチを破棄:

```bash
git worktree remove /home/ubuntu/fulgur/.worktrees/fulgur-6vl0-render-2pass
git branch -D task/fulgur-6vl0-render-2pass
```

`main` は無傷。

---

## References

- Beads issue: `fulgur-6vl0`
- Related PR: #566 (`layout_to_drawables` extraction that made this optimization possible)
- Files: `crates/fulgur/src/engine.rs`, `crates/fulgur/src/render.rs`
