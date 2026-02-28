# zcrc-monitor 開発ガイドライン

## 開発ルール

### コードスタイル
- `cargo fmt --all -- --check` でRustコードフォーマットをチェック
- `cargo clippy --all-targets --all-features -- -D warnings` でlintをチェック（警告はエラーとして扱う）
- `#[allow(clippy::...)]` による clippy 警告の抑制は禁止。警告が出た場合はコードを修正して根本対応すること
- `cargo test` ですべてのテストが通ることを確認

#### モジュール構成
**モダンなRustコードスタイル**: `mod.rs`ファイルの使用を避け、ディレクトリ同名のファイルを使用する

```rust
// 推奨されるモダンな構成
src/
├── main.rs
├── utils.rs          // utils/ ディレクトリ内の pub mod を定義
├── utils/
│   ├── config.rs     // pub mod config;
│   └── validation.rs // pub mod validation;
├── api.rs            // api/ ディレクトリ内の pub mod を定義
└── api/
    ├── handlers.rs   // pub mod handlers;
    └── routes.rs     // pub mod routes;

// utils.rs の内容例
pub mod config;
pub mod validation;

// api.rs の内容例
pub mod handlers;
pub mod routes;
```

```rust
// 避けるべき従来の構成
src/
├── main.rs
├── utils/
│   ├── mod.rs        // ← 避けるべき
│   ├── config.rs
│   └── validation.rs
└── api/
    ├── mod.rs        // ← 避けるべき
    ├── handlers.rs
    └── routes.rs
```

この構成により、モジュールの構造がより明確になり、ファイルの役割が理解しやすくなります。

### ログ出力の方針
**重要**: `println!` マクロの使用は禁止です。適切なログマクロを使用してください。
- **例外**: テストコード（`#[cfg(test)]`モジュールや`tests.rs`ファイル）では、デバッグ出力として`println!`の使用を許可します。

### CI/CDチェック項目
開発時は以下のコマンドでCIと同じチェックを実行可能:

1. **フォーマットチェック**
   ```bash
   cargo fmt --all -- --check
   ```

2. **Clippy（静的解析）**
   ```bash
   cargo clippy --all-targets --all-features -- -D warnings
   ```

3. **テスト実行**
   ```bash
   cargo test
   ```

### テスト
- 新機能には単体テストを作成
- `cargo test` でテストを実行
- テストカバレッジを維持

### テストコードの分離

以下の **両方** を満たすファイルは、テストコードを別ファイルに分離する。

1. テストコード（`#[cfg(test)] mod tests { ... }` ブロック）がファイル全体の **1/4 超**
2. テストコードが **100 行超**

#### 分離方法

`foo.rs` を `foo.rs` + `foo/tests.rs` に分割する。`mod.rs` は使わない。

**変更前:**

```
src/
  foo.rs          # プロダクションコード + テスト
```

**変更後:**

```
src/
  foo.rs          # プロダクションコード + #[cfg(test)] mod tests;
  foo/
    tests.rs      # テストモジュールの中身（mod tests { } の内側だけ）
```

**`foo.rs` の末尾:**

```rust
#[cfg(test)]
mod tests;
```

**`foo/tests.rs`:**

```rust
use super::*;

#[test]
fn test_example() {
    // ...
}
```

#### 大規模テストファイルの分割

テストファイル（`tests.rs` や `tests/` 配下のファイル）が **2000 行**を超える場合は、テストの関心事ごとにサブモジュールへ分割すること。

**変更前:**

```
src/
  foo.rs
  foo/
    tests.rs      # 2000 行超の大規模テストファイル
```

**変更後（サブモジュール名は一例）:**

```
src/
  foo.rs
  foo/
    tests.rs      # pub use + mod 宣言のみ
    tests/
      helpers.rs
      basic.rs
      advanced.rs
```

**`tests.rs`（分割後）:**

```rust
pub use super::*;
// テスト共通の use 宣言

mod helpers;
pub use helpers::*;

mod basic;
mod advanced;
```

**各サブモジュール:**

```rust
use super::*;

#[test]
fn test_example() {
    // ...
}
```

### コミット粒度
- コミットは独立した変更ごとに分けること（1コミット = 1つの論理的変更）
- 1つのコミットに複数の独立した変更を混ぜない
- 例: 3つの独立したテスト追加 → 3つの個別コミット

### コミットメッセージ
- 明確で説明的なコミットメッセージを使用
- 可能であれば conventional commit 形式に従う

### ブランチ戦略
- Git Flow を採用
- `develop` ブランチが開発の中心
- `main` ブランチは本番リリース用
- 機能開発は `feature/*` ブランチで行う
- リリースは `release/*` ブランチで準備
- 緊急修正は `hotfix/*` ブランチで対応

### プルリクエスト
- develop ブランチから機能ブランチを作成
- レビュー依頼前にCIが通ることを確認
- 変更内容とテスト方法の説明を含める

## プロジェクト概要

zcrc-monitor は、Zaciraci の gRPC-Web API に接続し、サービスの稼働状況や設定を監視するアプリケーションです。

### 接続先

Zaciraci Web クレート（`zaciraci/crates/web`）が提供する以下の gRPC サービスに接続します:

- **HealthService**: サービスの正常性・データベース接続状態の監視
- **ConfigService**: 設定エントリの CRUD 操作

## 開発環境セットアップ

### 前提条件
- Rust（バージョンは rust-toolchain.toml を参照）
- 接続先の Zaciraci サービスが起動していること
