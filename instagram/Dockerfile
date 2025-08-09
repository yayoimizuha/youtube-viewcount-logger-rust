FROM denoland/deno:ubuntu
LABEL authors="yayoimizuha"

RUN deno run --allow-all npm:playwright@latest install --with-deps
# remove all apt chaches
RUN apt-get clean && rm -rf /var/lib/apt/lists/*

COPY instagram_follower.ts .
RUN deno cache instagram_follower.ts

ENTRYPOINT ["/bin/bash"]