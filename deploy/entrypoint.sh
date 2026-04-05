#!/bin/bash
# ジョブ種別を RUN_JOB 環境変数で切り替えるディスパッチャ
set -euo pipefail

: "${RUN_JOB:?RUN_JOB environment variable is required (youtube|instagram|tweet|release)}"

case "$RUN_JOB" in
    youtube)   exec ./entrypoint_youtube.sh ;;
    instagram) exec ./entrypoint_instagram.sh ;;
    tweet)     exec ./entrypoint_tweet.sh ;;
    release)   exec ./entrypoint_release.sh ;;
    *)
        echo "Unknown RUN_JOB value: $RUN_JOB" >&2
        echo "Valid values: youtube, instagram, tweet, release" >&2
        exit 1
        ;;
esac
