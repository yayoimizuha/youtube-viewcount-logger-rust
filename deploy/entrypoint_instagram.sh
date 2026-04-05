#!/bin/bash
# instagram ジョブ:
#   1. GCS から misc.duckdb.zst をダウンロード
#   2. zstd 展開
#   3. Rust バイナリで Instagram フォロワー数取得 → DuckDB 更新
#   4. zstd 圧縮
#   5. 変更があれば GCS へアップロード
set -euo pipefail

: "${GCS_BUCKET:?GCS_BUCKET is required}"

echo "[instagram] Downloading misc.duckdb.zst from GCS..."
gcloud storage cp "gs://${GCS_BUCKET}/misc.duckdb.zst" .

echo "[instagram] Extracting zstd archive..."
zstd -dk -f misc.duckdb.zst

# 処理前のチェックサムを記録
CHECKSUM_BEFORE=$(sha256sum misc.duckdb | awk '{print $1}')

echo "[instagram] Running instagram_scraper..."
./instagram_scraper || echo "[instagram] instagram_scraper exited with error, continuing..."

CHECKSUM_AFTER=$(sha256sum misc.duckdb | awk '{print $1}')

if [ "$CHECKSUM_BEFORE" = "$CHECKSUM_AFTER" ]; then
    echo "[instagram] No changes detected. Skipping upload."
else
    echo "[instagram] Compressing misc.duckdb..."
    zstd misc.duckdb --ultra -22 -f -o misc.duckdb.zst

    echo "[instagram] Uploading misc.duckdb.zst to GCS..."
    gcloud storage cp misc.duckdb.zst "gs://${GCS_BUCKET}/misc.duckdb.zst"
fi

echo "[instagram] Done."
