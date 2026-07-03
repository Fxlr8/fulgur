# Release パイプライン pivot: release-plz 採用と changelog PR ベース一本化

2026-07-04 記録。epic `fulgur-bg1n` (release-plz adoption) の締めとして、
2026-04-26 design (`2026-04-26-versioning-and-release-simplification-design.md`)
からの一連の方針転換を記録する。design 文書は履歴として残し、本ノートで現状を
上書きする。

## 背景: 2026-04-26 design が想定していたもの

当時の方針は「手動 `release-prepare.yml` pipeline を簡略化する」ことだった:

- `release-prepare.yml` に `version` input を渡して auto-bump → `CHANGELOG.md`
  生成 → Release PR を作成
- `skip_bindings` input で Release PR に `release:skip-bindings` ラベルを付与し、
  各 publish workflow がそれを参照して bindings publish を丸ごと skip
- git-cliff / `cliff.toml` で Conventional Commits をカテゴリ分類して changelog 生成

release-plz は 2026-04-26 当時は検討対象外だったため、以下は却下決定の覆しでは
なくクリーンな pivot である。

## 転換点 1: release-plz を automation layer として採用 (fulgur-bg1n)

手動 `release-prepare.yml` を廃止し、release-plz を Release PR / changelog /
crates.io publish / tag 作成の automation layer として採用した。ただし
**version authority にはしない**: ZeroVer ポリシー「毎回 minor 固定 / patch は
hotfix 専用」は `custom_minor_increment_regex = ".*"` で維持している (全 commit を
無条件 Minor 判定にし、Patch フォールスルーを封じる。major は 0 固定)。

bindings (PyPI / RubyGems / npm) は release-plz の範囲外 (crates.io 専用) だが、
release-plz が作る tag / GitHub Release が既存 `release-python.yml` /
`release-ruby.yml` / `release.yml` の npm publish の発火トリガーになるよう chain
させている。この chain は App token push で発火する (`fulgur-inp6` / `fulgur-n5tw`)。

## 転換点 2: skip-bindings ラベル機構の撤廃 (fulgur-f7o2)

2026-04-26 design の `skip_bindings` (core-only release) 機構を撤廃した。

理由: bindings は常に crates.io と **version lockstep** で同時 publish する方針に
確定したため、部分的に bindings だけ skip する運用は想定しない。ラベルによる
スキップ機構は存在意義を失った。

- `release-python.yml` / `release-ruby.yml` の `check-skip-label` job を削除
- `release.yml` の `publish-npm` を無条件 publish に (確定した正しい状態)
- `docs/RELEASE_SETUP.md` の Skip bindings / Core-only release 節を削除
- GitHub label `release:skip-bindings` は手動削除

## 転換点 3: changelog を PR ベースに一本化 (fulgur-b0nb)

release-plz 移行後、changelog が二重化していた:

- ルート `CHANGELOG.md` は PR ベース (`.github/release.yml` の `release-notes:*`
  ラベル分類) で生成されていたが `0.20.0` で停止
- release-plz が各 crate に commit ベースの `crates/*/CHANGELOG.md` を生成し
  `0.22.0` まで進行

両者はフォーマットも生成源も異なるため、PR ベースに一本化した:

- release-plz の commit ベース changelog を無効化 (`changelog_update = false` を
  `release-plz.toml` の `[workspace]` に設定。release-plz 0.3.159 で実在キーと確認済み)
- `crates/*/CHANGELOG.md` 5 個 (fulgur / fulgur-cli / fulgur-ruby / fulgur-wasm /
  pyfulgur) を削除
- ルート `CHANGELOG.md` は `release-plz.yml` の release-pr job (aux-sync step) で
  `gh api ... generate-notes` の出力を prepend する。旧 `release-prepare.yml` の
  generate-notes step (awk で既存セクションを保持しつつ prepend) を移植したもの
- GitHub Release 本文も `release.yml` の release job で同じ generate-notes 出力に
  上書きし、`CHANGELOG.md` と揃える
- git-cliff / `cliff.toml` は既に撤去済み (追加作業なし)

### なぜ prepend を release-pr job に置くか

changelog prepend を「release 公開後に main へ push」する形にすると、`release-plz.yml`
の `on: push: [main]` を再発火させる (release-pr job には `if` ゲートが無く、
`custom_minor_increment_regex = ".*"` で全 commit が releasable なため、changelog-only
commit が次バージョンの Release PR を勝手に開く)。これを避けるため、changelog の
prepend は **release-pr job の aux-sync step** に置き、changelog を Release PR の
tagged commit に含める。GitHub Release 本文の上書きは release job での
`gh release edit --notes` (commit / push 無し) なので再発火しない。

## 現在の release パイプライン (2026-07-04 時点)

```text
push to main
  → release-plz.yml release-pr job:
      release-plz が version bump (minor 固定) + Release PR 作成 (--no-changelog 相当:
      changelog_update=false で release-plz 自身は CHANGELOG を触らない)
      aux-sync step が version ファイル同期 + generate-notes を CHANGELOG.md に prepend
  → ① Release PR 承認 (release-pr-approval 必須ステータスチェック) + merge
  → release-plz.yml release job:
      ② crates-io environment 承認 → crates.io publish + vX.Y.Z tag (App token)
  → release.yml (tag 発火):
      binaries build → GitHub Release publish (本文 = generate-notes) → npm publish
      → release:published が release-python.yml / release-ruby.yml を連鎖発火
```

changelog の単一ソースは `.github/release.yml` の `release-notes:*` ラベル分類。
ラベル付けは PR author / reviewer の責務 (CI では強制しない)。

## 関連 issue

- `fulgur-bg1n` — epic: release-plz adoption
- `fulgur-inp6` — Release PR の App token 化 (CI / chain 発火)
- `fulgur-f7o2` — skip-bindings 撤廃 + chain 確認
- `fulgur-b0nb` — 本ノート (changelog PR ベース一本化 + docs 整合)
