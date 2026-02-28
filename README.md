# zcrc-monitor

Zaciraci の gRPC API（`zaciraci/crates/web`）に接続し、サービスの稼働状況や設定を監視するWebアプリケーションです。

## 機能

- **ヘルスチェック監視**: `HealthService.Check` を定期的に呼び出し、サービスの稼働状況とデータベース接続状態を表示
- **設定管理**: `ConfigService` を通じて設定エントリの一覧表示・詳細確認

## 接続先 API

Zaciraci Web クレートが提供する gRPC-Web API に接続します。

### HealthService

| RPC | 説明 |
|-----|------|
| `Check` | サービスの正常性とデータベース接続状態を返す |

### ConfigService

| RPC | 説明 |
|-----|------|
| `GetAll` | 指定インスタンスの全設定エントリを取得 |
| `GetOne` | 指定インスタンス・キーの設定値を取得 |
| `Upsert` | 設定エントリの作成・更新 |
| `Delete` | 設定エントリの削除 |

## 開発

開発の詳細は [CONTRIBUTING.md](CONTRIBUTING.md) を参照してください。
