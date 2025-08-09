FROM denoland/deno:ubuntu
LABEL authors="yayoimizuha"

ENV HOME=/github/home
RUN deno run --allow-all npm:playwright@latest install chromium --with-deps
RUN mkdir /.cache && ln -s /.cache ~/.cache
# remove all apt chaches
RUN apt-get clean && rm -rf /var/lib/apt/lists/*

COPY instagram_follower.ts .
RUN deno cache instagram_follower.ts && rm instagram_follower.ts

ENTRYPOINT ["/usr/bin/deno"]