#!/bin/bash
# tweet ジョブ:
#   1. GCS から data.duckdb.zst をダウンロード・展開
#   2. Deno で render_graph.ts を実行 → グラフ生成 + Twitter 投稿
set -euo pipefail

: "${GCS_BUCKET:?GCS_BUCKET is required}"

echo "[tweet] Downloading data.duckdb.zst from GCS..."
gcloud storage cp "gs://${GCS_BUCKET}/data.duckdb.zst" .

echo "[tweet] Extracting zstd archive..."
zstd -dk -f data.duckdb.zst

echo "[tweet] Running render_graph.ts..."
deno --allow-all render_graph.ts || echo "[tweet] render_graph.ts exited with error, continuing..."

echo "[tweet] Done."
