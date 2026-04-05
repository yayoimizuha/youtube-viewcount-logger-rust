#!/bin/bash
# release ジョブ:
#   1. GCS から data.duckdb.zst / misc.duckdb.zst をダウンロード・展開
#   2. Deno で render_group_report.ts を実行 → PDF 生成
#   3. Rust の excel_exporter を実行 → Excel 生成
#   4. GitHub Release を作成し成果物をアップロード
set -euo pipefail

: "${GCS_BUCKET:?GCS_BUCKET is required}"
: "${GH_TOKEN:?GH_TOKEN is required}"
: "${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is required}"

echo "[release] Downloading DB files from GCS..."
gcloud storage cp "gs://${GCS_BUCKET}/data.duckdb.zst" .
gcloud storage cp "gs://${GCS_BUCKET}/misc.duckdb.zst" .

echo "[release] Extracting zstd archives..."
zstd -dk -f data.duckdb.zst
zstd -dk -f misc.duckdb.zst

echo "[release] Running render_group_report.ts..."
deno --allow-all render_group_report.ts || echo "[release] render_group_report.ts exited with error, continuing..."

echo "[release] Running excel_exporter..."
./excel_exporter || echo "[release] excel_exporter exited with error, continuing..."

echo "[release] Creating GitHub Release..."
RELEASE_TAG="daily-$(date +'%Y-%m-%d')"
RELEASE_TITLE="Daily Report $(date +'%Y-%m-%d')"

# 同名リリースが既にあれば削除
gh release delete "$RELEASE_TAG" --yes --repo "$GITHUB_REPOSITORY" || true
git ls-remote --tags "https://x-access-token:${GH_TOKEN}@github.com/${GITHUB_REPOSITORY}" "$RELEASE_TAG" \
    | grep -q . && \
    git push "https://x-access-token:${GH_TOKEN}@github.com/${GITHUB_REPOSITORY}" \
        --delete "refs/tags/$RELEASE_TAG" || true

gh release create "$RELEASE_TAG" \
    --title "$RELEASE_TITLE" \
    --notes-file workdir/github_release.md \
    --repo "$GITHUB_REPOSITORY" \
    workdir/group_report.pdf \
    data.duckdb.zst \
    misc.duckdb.zst \
    workdir/export.xlsx

echo "[release] Done."
