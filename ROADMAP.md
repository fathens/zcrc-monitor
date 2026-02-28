# zcrc-monitor ロードマップ

Zaciraci の gRPC API を監視する Slint デスクトップ・モバイルアプリケーション。

## 技術スタック

| カテゴリ | 技術 |
|----------|------|
| UI フレームワーク | Slint |
| gRPC クライアント | tonic |
| Proto コード生成 | prost / tonic-prost-build |
| 非同期ランタイム | tokio |
| ログ | tracing |
| Rust エディション | 2024 |

## Proto ファイル共有

サーバー側（`zaciraci/crates/web/proto/`）の proto ファイルを git submodule で参照し、`tonic-prost-build` で Rust コードを生成する。

```
zcrc-monitor/
├── proto/           # git submodule → zaciraci リポジトリの proto ディレクトリ
├── build.rs         # tonic-prost-build によるコード生成
└── src/
```

将来的に proto が増えた場合は独立リポジトリ `zaciraci-proto` への分離も可能。

## Phase 0: プロジェクト基盤構築

### 目標

空の Slint ウィンドウを表示し、tokio ランタイム上で動作する最小構成を確立する。

### タスク

- Cargo.toml 作成（slint, tonic, prost, tokio, tracing）
- rust-toolchain.toml 作成
- proto submodule 設定
- build.rs（Slint コンパイル + tonic-prost-build による proto コード生成）
- 空の Slint ウィンドウ表示 + tokio ランタイム起動

### 想定ソース構成

```
src/
├── main.rs          # エントリポイント（tokio ランタイム + Slint 起動）
├── grpc.rs          # gRPC 接続管理
├── grpc/
│   └── ...          # サービス別クライアントラッパー
├── ui.rs            # Slint コールバック・データバインディング
└── ui/
    └── ...          # 画面別モジュール
```

## Phase 1: ヘルスチェック & 設定管理

サーバー Phase 1（Health & Config API）に対応。

### gRPC 接続基盤

- 接続先 URL の設定（環境変数 or 設定ファイル）
- tonic チャネル生成・接続状態管理
- 接続エラー時のリトライ・UI フィードバック

### HealthService

- `Check` を定期ポーリング（インターバル設定可能）
- ステータスインジケーター表示（正常 / 異常 / 未接続）
- DB 接続状態の表示

### ConfigService

- **設定一覧**（`GetAll`）: インスタンスの全設定エントリをリスト表示
- **設定詳細**（`GetOne`）: 選択した設定の詳細表示
- **設定編集**（`Upsert`）: 設定値の作成・更新フォーム
- **設定削除**（`Delete`）: 確認ダイアログ付き削除

## Phase 2: ポートフォリオ & 取引閲覧

サーバー Phase 2（ポートフォリオ・取引閲覧）に対応。

### 評価期間ビュー

- 評価期間一覧（`GetEvaluationPeriods`）
- 評価期間詳細（`GetEvaluationPeriod`）

### 取引履歴

- 取引一覧（`GetTrades`）: ページネーション対応
- バッチ単位の取引詳細（`GetTradesByBatch`）
- 最新バッチ表示（`GetLatestBatch`）

### レート表示

- 全トークン最新レート（`GetLatestRates`）
- レート履歴（`GetRateHistory`）

## Phase 3: アクション系機能

サーバー Phase 3（アクション系 API）に対応。

### ハーベスト実行 UI

- 金額入力・実行ボタン（`Execute`）
- 実行状態の表示（`GetStatus`）

### シミュレーション UI

- シミュレーション開始（`Start`）
- ストリーミングによる進捗表示（server streaming）
- 完了済み結果の表示（`GetResult`）

## Future: モバイル対応

- Slint のモバイルターゲット（Android / iOS）調査
- モバイル向け UI レイアウト調整
- tonic-web（HTTP/1.1）経由での接続確認

## フェーズ依存関係

```
Phase 0 ──→ Phase 1 ──┬──→ Phase 2
                       │
                       └──→ Phase 3
Phase 1+ ─────────────────→ Future (モバイル)
```

- Phase 2 と Phase 3 は Phase 1 完了後に並行着手可能
- モバイル対応は Phase 1 以降いつでも着手可能

## サーバー側ロードマップとの対応表

| サーバー (zaciraci/crates/web) | クライアント (zcrc-monitor) | 対応サービス |
|------|------|------|
| Phase 1: Health & Config API | Phase 1: ヘルスチェック & 設定管理 | HealthService, ConfigService |
| Phase 2: ポートフォリオ・取引閲覧 | Phase 2: ポートフォリオ & 取引閲覧 | PortfolioService |
| Phase 3: アクション系 API | Phase 3: アクション系機能 | HarvestService, SimulationService |
