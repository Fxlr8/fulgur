# Return the Item on Failure — Avoid Clone-on-Success

Rust で「引数を by-value で消費する関数」を書くときの pattern の 1 つ。
gemini code assist の Rust review が繰り返し指摘してくるので、書く時点で preempt する。

## Rule

「値を消費するかもしれない関数」で、以下 3 条件が揃ったら
`fn f(x: T) -> bool` ではなく `fn f(x: T) -> Result<(), T>` を選ぶ。

1. 引数 `x: T` を **by-value** で取り、成功時に `x` を内部で消費する
2. 失敗パスがある（何らかの理由で `x` を使えないケース）
3. 呼び出し元は失敗時に **同じ `x` をフォールバックで再利用したい**

戻り値のセマンティクスは:

- `Ok(())` = 成功。関数が `x` を消費した。呼び出し元はもう `x` を持たない。
- `Err(x)` = 失敗。関数は `x` を触らず（あるいはロールバックして）呼び出し元に返却した。

これにより呼び出し元は `if let Err(x) = f(x) { ...use x... }` の形で書け、
**成功パスの `x.clone()` が完全に消える**。fulgur は `Result<(), T>` を採用する
（`Ok`=成功が intuition に沿う）。`Option<T>` (None=成功) を使うと意味論反転で
読み手が引っかかるため、避ける。

## Why

gemini code assist は Rust review でこの refactor を頻繁に medium-priority で指摘する。
成功パスが hot path で、`T` が `Vec` / `String` / 大きな Arc payload を含む場合、
1 clone あたり複数の heap allocation を伴うので、指摘は妥当。指摘 → 修正 → thread
返信の round trip が発生するので、初回に書き切っておくのが最も効率的。

### 具体例（fulgur PR #605）

**Before:**

```rust
fn inject_marker_into_first_paragraph(
    out: &mut Drawables,
    mark: DrawMark,
    item: LineItem,
) -> bool { /* ... */ }

// caller
if !inject_marker_into_first_paragraph(out, mark, item.clone()) {
    // failure fallback — uses item
    let lines = vec![ShapedLine { items: vec![item], .. }];
    /* ... */
}
```

`LineItem::Text` は `Vec<ShapedGlyph>` + `String` を持つので、成功パスで毎回
2 heap allocation が無駄に発生する。

**After:**

```rust
/// Returns `Ok(())` on success (item consumed and inserted).
/// Returns `Err(item)` on failure (item handed back unchanged) — caller
/// reuses it for the fallback path without paying a clone on success.
#[must_use = "returns Err(item) on failure; the handed-back item must be reused or explicitly dropped"]
fn inject_marker_into_first_paragraph(
    out: &mut Drawables,
    mark: DrawMark,
    item: LineItem,
) -> Result<(), LineItem> { /* ... */ }

// caller
if let Err(item) = inject_marker_into_first_paragraph(out, mark, item) {
    // failure fallback — uses handed-back item
    let lines = vec![ShapedLine { items: vec![item], .. }];
    /* ... */
}
```

- gemini review comment: [PR #605 discussion r3538890283](https://github.com/fulgur-rs/fulgur/pull/605#discussion_r3538890283)
  （gemini 提案は `Option<T>` だったが、意味論反転を避けて `Result<(), T>` に切り替え）
- 対応 commits: `946319d8` (perf(convert): return item from inject_marker instead of cloning) →
  さらに Result 化 + `#[must_use]` 追加の follow-up

## How to Apply

- **新規コード:** 上の 3 条件がそろったら最初から `Result<(), T>` 戻り値で書く。
  `T` の clone コストが軽い (`Copy` / `Arc` のみ、`usize` 数個の struct など) 場合は
  保留可 — gemini も指摘してこない可能性が高い。
- **`#[must_use]` を必ず付ける:**

  ```rust
  #[must_use = "returns Err(item) on failure; the handed-back item must be reused or explicitly dropped"]
  fn f(x: T) -> Result<(), T> { /* ... */ }
  ```

  この付与を忘れると、caller が戻り値を握りつぶした瞬間に「消費したつもりのアイテムが
  無言で drop される」バグになる。地味だが致命的なので **How to Apply の必須項目**。
  `Result<(), T>` は clippy `unused_must_use` lint に元々 hook されるので、
  `#[must_use = "..."]` はメッセージ強化目的（`Option<T>` を使う場合は `#[must_use]`
  なしだと lint も鳴らないのでより必須）。
- **Doc コメントで必ず明記:** 「`Ok(())` = 成功で consumed / `Err(t)` = 失敗で t を返却」。
  `Result<(), T>` は `Ok(())`=成功で intuition に沿うため負担は軽いが、
  「なぜ `Err` が値を持つか」を一行足すと親切。
- **`Result<(), T>` vs `Option<T>` の選び方（fulgur は `Result` 標準）:**
  - **`Result<(), T>` (推奨)**: `Ok(())` = 成功、`Err(t)` = 失敗で t を返却、と
    intuition に沿う。 `?` 演算子との相性もよい。fulgur では PR #605 でこちらを採用。
  - **`Option<T>`**: gemini review が suggest してきがちだが、`None = 成功` は
    意味論反転で読み手を裏切る。避ける（採用しても Doc + `#[must_use]` で厳重にカバーが必要）。
  - レビュアーの慣れ・関数のリターン哲学が既に `Option<T>` になっているモジュールに
    合わせる、といった一貫性理由がなければ `Result<(), T>`。
- **既存コードの refactor:** `if !func(x.clone())` 相当のパターンを検出したいとき、
  素朴な `rg` だと `x.clone()` の位置がフォーマッタで改行分割されていて拾い逃す
  ことが多い。実務では次の順:
  1. まず `rg -n '\.clone\(\)' <path>` で候補全部出して目視
  2. または [ast-grep](https://ast-grep.github.io/) で
     `pattern: 'if !$FN($$$, $ARG.clone(), $$$) { $$$ }'` 等の AST パターンで検索

  Tests の `assert!(func(x))` → `.is_ok()` / `assert!(!func(x))` → `.is_err()` 移行も同時に。

## Judgment Calls

- **保留可（refactor しなくてよい）**
  - `T: Copy`（clone がゼロコスト）
  - `T = Arc<...>`（clone が単なる ref bump）
  - 失敗パスが cold で hot path が失敗側
  - **`T` の `Drop` に観測可能な副作用がある場合**（例: file handle を close する型、
    logging を吐く型、Mutex guard など）。成功パス（関数内で consumed される）と
    失敗パス（caller に返却され後で drop される）で **副作用の発生タイミングが変わる**
    ので、consumed 前提のリソース管理が壊れる可能性がある。テストが通っても
    prod で挙動が変わることがあるので、`Drop` impl を確認してから判断する。
- **レビュアーの慣れ**: fulgur は `Result<(), T>` 標準にした（PR #605、意味論反転
  回避が理由）。もしモジュール内で先に `Option<T>` パターンが確立している場合は
  一貫性優先で合わせてもよい。新規モジュールなら常に `Result<(), T>`。

## Related

- `feedback_plan_doc_no_llm_directive.md`（同じく gemini review 由来の学び — plan doc に
  `For Claude: REQUIRED SUB-SKILL` 等の LLM 向け directive を書かない）
