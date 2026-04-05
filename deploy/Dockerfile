# ============================================================
# Stage 1: Rust build
# ============================================================
FROM rust:latest AS builder

WORKDIR /build

# 依存クレートのキャッシュレイヤーを先に作る
COPY Cargo.toml Cargo.lock ./
COPY build.rs ./
# ダミーの src を作ってビルドし、依存クレートをキャッシュする
RUN mkdir -p src/bin && \
    echo 'fn main() {}' > src/main.rs && \
    for b in migrate_db migrate_duckdb duckdb_test excel_exporter instagram_scraper youtube_v3_api_test gemini_test; do \
        echo 'fn main() {}' > src/bin/${b}.rs; \
    done && \
    cargo build --profile relwithdebinfo 2>&1 | tail -5 || true && \
    rm -rf src

# 本番ソースをコピーしてビルド
COPY src ./src
COPY assets ./assets
RUN cargo build --profile relwithdebinfo

# ============================================================
# Stage 2: Runtime image
# ============================================================
FROM debian:bookworm-slim AS runtime

# システムツール + zstd + gcloud CLI に必要なもの
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        unzip \
        zstd \
        apt-transport-https \
        gnupg \
    && rm -rf /var/lib/apt/lists/*

# Google Cloud CLI
RUN curl -sSL https://dl.google.com/dl/cloudsdk/channels/rapid/downloads/google-cloud-cli-linux-x86_64.tar.gz \
    | tar -xz -C /usr/local && \
    /usr/local/google-cloud-sdk/install.sh --quiet --path-update false && \
    ln -s /usr/local/google-cloud-sdk/bin/gcloud /usr/local/bin/gcloud && \
    ln -s /usr/local/google-cloud-sdk/bin/gsutil /usr/local/bin/gsutil

# Deno
RUN curl -fsSL https://deno.land/install.sh | sh
ENV DENO_INSTALL="/root/.deno"
ENV PATH="${DENO_INSTALL}/bin:${PATH}"

# Typst（最新リリースを GitHub API で取得）
RUN TYPST_URL=$(curl -fsSL "https://api.github.com/repos/typst/typst/releases/latest" \
        | grep '"browser_download_url"' \
        | grep 'x86_64-unknown-linux-musl' \
        | sed 's/.*"browser_download_url": "\(.*\)"/\1/' | tr -d ' ') && \
    curl -fsSL "$TYPST_URL" -o /tmp/typst.tar.xz && \
    tar -xJf /tmp/typst.tar.xz -C /tmp && \
    mv /tmp/typst-*/typst /usr/local/bin/typst && \
    rm -rf /tmp/typst*

WORKDIR /app

# Rust バイナリ
COPY --from=builder /build/target/relwithdebinfo/youtube-viewcount-logger-rust ./
COPY --from=builder /build/target/relwithdebinfo/instagram_scraper ./
COPY --from=builder /build/target/relwithdebinfo/excel_exporter ./

# TypeScript スクリプト・アセット・クエリ
COPY render_graph.ts render_group_report.ts ./
COPY assets ./assets
COPY group_list.json ./
COPY workdir/.gitkeep workdir/

# entrypoint スクリプト
COPY deploy/entrypoint_youtube.sh \
     deploy/entrypoint_instagram.sh \
     deploy/entrypoint_tweet.sh \
     deploy/entrypoint_release.sh \
     ./
RUN chmod +x entrypoint_youtube.sh entrypoint_instagram.sh entrypoint_tweet.sh entrypoint_release.sh

# RUN_JOB 環境変数でジョブを切り替える
COPY deploy/entrypoint.sh ./
RUN chmod +x entrypoint.sh

ENTRYPOINT ["./entrypoint.sh"]
