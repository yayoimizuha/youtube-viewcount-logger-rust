#!/bin/bash
# youtube ジョブ:
#   1. GCS から data.duckdb.zst をダウンロード
#   2. zstd 展開
#   3. Rust バイナリでビューカウント取得 → DuckDB 更新
#   4. zstd 圧縮
#   5. GCS へアップロード
set -euo pipefail

: "${GCS_BUCKET:?GCS_BUCKET is required}"

echo "[youtube] Downloading data.duckdb.zst from GCS..."
gcloud storage cp "gs://${GCS_BUCKET}/data.duckdb.zst" .

echo "[youtube] Extracting zstd archive..."
zstd -dk -f data.duckdb.zst

echo "[youtube] Running youtube-viewcount-logger-rust..."
./youtube-viewcount-logger-rust

echo "[youtube] Compressing data.duckdb..."
zstd data.duckdb --ultra -22 -f -o data.duckdb.zst

echo "[youtube] Uploading data.duckdb.zst to GCS..."
gcloud storage cp data.duckdb.zst "gs://${GCS_BUCKET}/data.duckdb.zst"

echo "[youtube] Done."
