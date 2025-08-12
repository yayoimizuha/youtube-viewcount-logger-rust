FROM denoland/deno:ubuntu
LABEL authors="yayoimizuha"

ENV HOME=/github/home
RUN mkdir /.cache && ln -s /.cache ~/.cache
RUN deno run --allow-all npm:playwright@latest install chromium --with-deps && apt-get clean && rm -rf /var/lib/apt/lists/*

COPY instagram_follower.ts .
RUN deno cache instagram_follower.ts && rm instagram_follower.ts

ENTRYPOINT ["/usr/bin/deno"]