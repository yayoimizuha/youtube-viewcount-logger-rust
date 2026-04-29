import { createHmac, randomBytes } from "node:crypto";
import { basename } from "node:path";
import { Buffer } from "node:buffer";
import { TwitterApi } from "npm:twitter-api-v2";

type TestResult = {
  name: string;
  ok: boolean;
  detail: string;
  elapsedMs: number;
};

let hadUnhandledRejection = false;

globalThis.addEventListener("unhandledrejection", (event) => {
  hadUnhandledRejection = true;
  event.preventDefault();
  console.error(`UNHANDLED_REJECTION ${summarizeError(event.reason)}`);
});

const requiredEnv = [
  "TWITTER_APP_KEY",
  "TWITTER_APP_SECRET",
  "TWITTER_ACCESS_TOKEN",
  "TWITTER_ACCESS_SECRET",
] as const;

const percentEncode = (value: string) =>
  encodeURIComponent(value)
    .replace(/[!'()*]/g, (char) =>
      `%${char.charCodeAt(0).toString(16).toUpperCase()}`);

const oauthHeader = (
  method: string,
  rawUrl: string,
  extraSignatureParams: Record<string, string> = {},
) => {
  const url = new URL(rawUrl);
  const oauthParams: Record<string, string> = {
    oauth_consumer_key: Deno.env.get("TWITTER_APP_KEY")!,
    oauth_nonce: randomBytes(16).toString("hex"),
    oauth_signature_method: "HMAC-SHA1",
    oauth_timestamp: Math.floor(Date.now() / 1000).toString(),
    oauth_token: Deno.env.get("TWITTER_ACCESS_TOKEN")!,
    oauth_version: "1.0",
  };
  const signatureParams = new URLSearchParams(url.search);
  for (const [key, value] of Object.entries(extraSignatureParams)) {
    signatureParams.append(key, value);
  }
  for (const [key, value] of Object.entries(oauthParams)) {
    signatureParams.append(key, value);
  }
  const parameterString = [...signatureParams.entries()]
    .sort(([ak, av], [bk, bv]) =>
      ak === bk ? av.localeCompare(bv) : ak.localeCompare(bk)
    )
    .map(([key, value]) => `${percentEncode(key)}=${percentEncode(value)}`)
    .join("&");
  const baseUrl = `${url.protocol}//${url.host}${url.pathname}`;
  const signatureBase = [
    method.toUpperCase(),
    percentEncode(baseUrl),
    percentEncode(parameterString),
  ].join("&");
  const signingKey = `${percentEncode(Deno.env.get("TWITTER_APP_SECRET")!)}&${
    percentEncode(Deno.env.get("TWITTER_ACCESS_SECRET")!)
  }`;
  oauthParams.oauth_signature = createHmac("sha1", signingKey)
    .update(signatureBase)
    .digest("base64");

  return "OAuth " + Object.entries(oauthParams)
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([key, value]) => `${percentEncode(key)}="${percentEncode(value)}"`)
    .join(", ");
};

const summarizeError = (error: unknown) => {
  if (error instanceof Error) {
    const maybeError = error as Error & {
      code?: number;
      data?: unknown;
      error?: unknown;
      type?: unknown;
    };
    return JSON.stringify({
      name: error.name,
      message: error.message,
      code: maybeError.code,
      type: maybeError.type,
      data: maybeError.data,
      error: maybeError.error,
    });
  }
  return String(error);
};

const runTest = async (
  name: string,
  fn: () => Promise<string>,
): Promise<TestResult> => {
  const started = performance.now();
  try {
    return {
      name,
      ok: true,
      detail: await fn(),
      elapsedMs: Math.round(performance.now() - started),
    };
  } catch (error) {
    return {
      name,
      ok: false,
      detail: summarizeError(error),
      elapsedMs: Math.round(performance.now() - started),
    };
  }
};

const expectJson = async (response: Response) => {
  const text = await response.text();
  let json: unknown;
  try {
    json = JSON.parse(text);
  } catch {
    json = text.slice(0, 500);
  }
  if (!response.ok) {
    throw new Error(JSON.stringify({
      status: response.status,
      statusText: response.statusText,
      body: json,
    }));
  }
  return json;
};

const directV2SimpleUpload = async (image: Uint8Array) => {
  const response = await fetch("https://api.x.com/2/media/upload", {
    method: "POST",
    headers: {
      authorization: oauthHeader("POST", "https://api.x.com/2/media/upload"),
      "content-type": "application/json",
    },
    body: JSON.stringify({
      media: Buffer.from(image).toString("base64"),
      media_category: "tweet_image",
      media_type: "image/png",
    }),
  });
  return JSON.stringify(await expectJson(response));
};

const directV1ChunkedUpload = async (image: Uint8Array) => {
  const initUrl = "https://upload.x.com/1.1/media/upload.json";
  const initParams = {
    command: "INIT",
    total_bytes: image.byteLength.toString(),
    media_type: "image/png",
    media_category: "tweet_image",
  };
  const initBody = new URLSearchParams(initParams);
  const initResponse = await fetch(initUrl, {
    method: "POST",
    headers: {
      authorization: oauthHeader("POST", initUrl, initParams),
      "content-type": "application/x-www-form-urlencoded",
    },
    body: initBody,
  });
  const initJson = await expectJson(initResponse) as {
    media_id_string?: string;
  };
  const mediaId = initJson.media_id_string;
  if (!mediaId) {
    throw new Error(
      `INIT returned no media_id_string: ${JSON.stringify(initJson)}`,
    );
  }

  const appendUrl = "https://upload.x.com/1.1/media/upload.json";
  const form = new FormData();
  form.append("command", "APPEND");
  form.append("media_id", mediaId);
  form.append("segment_index", "0");
  const imagePart = image.buffer.slice(
    image.byteOffset,
    image.byteOffset + image.byteLength,
  ) as ArrayBuffer;
  form.append(
    "media",
    new Blob([imagePart], { type: "image/png" }),
    "image.png",
  );
  const appendResponse = await fetch(appendUrl, {
    method: "POST",
    headers: {
      authorization: oauthHeader("POST", appendUrl),
    },
    body: form,
  });
  if (!appendResponse.ok) {
    await expectJson(appendResponse);
  } else {
    await appendResponse.arrayBuffer();
  }

  const finalizeUrl = "https://upload.x.com/1.1/media/upload.json";
  const finalizeParams = {
    command: "FINALIZE",
    media_id: mediaId,
  };
  const finalizeResponse = await fetch(finalizeUrl, {
    method: "POST",
    headers: {
      authorization: oauthHeader("POST", finalizeUrl, finalizeParams),
      "content-type": "application/x-www-form-urlencoded",
    },
    body: new URLSearchParams(finalizeParams),
  });
  return JSON.stringify(await expectJson(finalizeResponse));
};

for (const key of requiredEnv) {
  if (!Deno.env.get(key)) {
    throw new Error(
      `${key} is not set. Dot-source env.ps1 before running this script.`,
    );
  }
}

const argImagePath = Deno.args.find((arg) => !arg.startsWith("--"));
const failOnAnyFailure = Deno.args.includes("--fail-on-any-failure") ||
  Deno.env.get("FAIL_ON_ANY_FAILURE") === "true";
const imagePath = argImagePath ?? "scripts/fixtures/twitter-media-upload.png";
const image = await Deno.readFile(imagePath);
console.log(
  `Testing media upload with ${
    basename(imagePath)
  } (${image.byteLength} bytes)`,
);

const client = new TwitterApi({
  appKey: Deno.env.get("TWITTER_APP_KEY")!,
  appSecret: Deno.env.get("TWITTER_APP_SECRET")!,
  accessToken: Deno.env.get("TWITTER_ACCESS_TOKEN")!,
  accessSecret: Deno.env.get("TWITTER_ACCESS_SECRET")!,
});

const tests: Array<[string, () => Promise<string>]> = [
  ["twitter-api-v2 v2.uploadMedia", async () => {
    const mediaId = await client.v2.uploadMedia(Buffer.from(image), {
      media_type: "image/png",
      media_category: "tweet_image",
    });
    return `media_id=${mediaId}`;
  }],
  ["twitter-api-v2 v1.uploadMedia", async () => {
    const mediaId = await client.v1.uploadMedia(Buffer.from(image), {
      mimeType: "image/png",
      target: "tweet",
    });
    return `media_id=${mediaId}`;
  }],
  ["twitter-api-v2 v1.uploadMedia without target", async () => {
    const mediaId = await client.v1.uploadMedia(Buffer.from(image), {
      mimeType: "image/png",
    });
    return `media_id=${mediaId}`;
  }],
  ["direct v2 simple JSON upload", async () => directV2SimpleUpload(image)],
  ["direct v1 chunked upload", async () => directV1ChunkedUpload(image)],
];

for (const [name, test] of tests) {
  const result = await runTest(name, test);
  console.log(
    `${result.ok ? "PASS" : "FAIL"} ${result.name} (${result.elapsedMs}ms)`,
  );
  console.log(result.detail);
  if (!result.ok && failOnAnyFailure) {
    Deno.exitCode = 1;
  }
}

if (hadUnhandledRejection && failOnAnyFailure) {
  Deno.exitCode = 1;
}
