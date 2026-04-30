import {createHmac, randomBytes} from 'node:crypto';
import {Client as XClient, OAuth1, type Posts} from 'npm:@xdevplatform/xdk';

const required = [
    'TWITTER_APP_KEY',
    'TWITTER_APP_SECRET',
    'TWITTER_ACCESS_TOKEN',
    'TWITTER_ACCESS_SECRET',
] as const;

for (const key of required) {
    if (!Deno.env.get(key)) {
        throw new Error(`${key} is not set.`);
    }
}

const percentEncode = (value: string) =>
    encodeURIComponent(value)
        .replace(/[!'()*]/g, (char) => `%${char.charCodeAt(0).toString(16).toUpperCase()}`);

const oauthHeader = (
    method: string,
    rawUrl: string,
    extraSignatureParams: Record<string, string> = {},
) => {
    const url = new URL(rawUrl);
    const oauthParams: Record<string, string> = {
        oauth_consumer_key: Deno.env.get('TWITTER_APP_KEY')!,
        oauth_nonce: randomBytes(16).toString('hex'),
        oauth_signature_method: 'HMAC-SHA1',
        oauth_timestamp: Math.floor(Date.now() / 1000).toString(),
        oauth_token: Deno.env.get('TWITTER_ACCESS_TOKEN')!,
        oauth_version: '1.0',
    };
    const signatureParams = new URLSearchParams(url.search);
    for (const [key, value] of Object.entries(extraSignatureParams)) {
        signatureParams.append(key, value);
    }
    for (const [key, value] of Object.entries(oauthParams)) {
        signatureParams.append(key, value);
    }
    const parameterString = [...signatureParams.entries()]
        .sort(([ak, av], [bk, bv]) => ak === bk ? av.localeCompare(bv) : ak.localeCompare(bk))
        .map(([key, value]) => `${percentEncode(key)}=${percentEncode(value)}`)
        .join('&');
    const baseUrl = `${url.protocol}//${url.host}${url.pathname}`;
    const signatureBase = [
        method.toUpperCase(),
        percentEncode(baseUrl),
        percentEncode(parameterString),
    ].join('&');
    const signingKey = `${percentEncode(Deno.env.get('TWITTER_APP_SECRET')!)}&${percentEncode(Deno.env.get('TWITTER_ACCESS_SECRET')!)}`;
    oauthParams.oauth_signature = createHmac('sha1', signingKey)
        .update(signatureBase)
        .digest('base64');

    return 'OAuth ' + Object.entries(oauthParams)
        .sort(([a], [b]) => a.localeCompare(b))
        .map(([key, value]) => `${percentEncode(key)}="${percentEncode(value)}"`)
        .join(', ');
}

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
}

const uploadPng = async (imagePath: string) => {
    const image = await Deno.readFile(imagePath);
    const uploadUrl = 'https://upload.x.com/1.1/media/upload.json';
    const initParams = {
        command: 'INIT',
        total_bytes: image.byteLength.toString(),
        media_type: 'image/png',
        media_category: 'tweet_image',
    };
    const initResponse = await fetch(uploadUrl, {
        method: 'POST',
        headers: {
            authorization: oauthHeader('POST', uploadUrl, initParams),
            'content-type': 'application/x-www-form-urlencoded',
        },
        body: new URLSearchParams(initParams),
    });
    const initJson = await expectJson(initResponse) as { media_id_string?: string };
    const mediaId = initJson.media_id_string;
    if (!mediaId) {
        throw new Error(`INIT returned no media_id_string: ${JSON.stringify(initJson)}`);
    }

    const form = new FormData();
    form.append('command', 'APPEND');
    form.append('media_id', mediaId);
    form.append('segment_index', '0');
    form.append('media', new Blob([image], {type: 'image/png'}), 'image.png');
    const appendResponse = await fetch(uploadUrl, {
        method: 'POST',
        headers: {
            authorization: oauthHeader('POST', uploadUrl),
        },
        body: form,
    });
    if (!appendResponse.ok) {
        await expectJson(appendResponse);
    } else {
        await appendResponse.arrayBuffer();
    }

    const finalizeParams = {
        command: 'FINALIZE',
        media_id: mediaId,
    };
    const finalizeResponse = await fetch(uploadUrl, {
        method: 'POST',
        headers: {
            authorization: oauthHeader('POST', uploadUrl, finalizeParams),
            'content-type': 'application/x-www-form-urlencoded',
        },
        body: new URLSearchParams(finalizeParams),
    });
    const finalizeJson = await expectJson(finalizeResponse) as { media_id_string?: string };
    return finalizeJson.media_id_string ?? mediaId;
}

const oauth1 = new OAuth1({
    apiKey: Deno.env.get('TWITTER_APP_KEY')!,
    apiSecret: Deno.env.get('TWITTER_APP_SECRET')!,
    accessToken: Deno.env.get('TWITTER_ACCESS_TOKEN')!,
    accessTokenSecret: Deno.env.get('TWITTER_ACCESS_SECRET')!,
    callback: 'oob',
});
const client = new XClient({oauth1});

const withMedia = Deno.env.get('WITH_MEDIA') === 'true' || Deno.args.includes('--with-media');
const defaultText = `XDK post test ${new Date().toISOString()} run=${Deno.env.get('GITHUB_RUN_ID') ?? 'local'}`;
const text = Deno.env.get('POST_TEXT') || defaultText;
const imagePath = Deno.env.get('IMAGE_PATH') || 'scripts/fixtures/xdk-tweet-test.png';
const mediaIds = withMedia ? [await uploadPng(imagePath)] : [];
const body: Posts.CreateRequest = {
    text,
    ...(mediaIds.length ? {media: {mediaIds}} : {}),
};

console.log(`Posting via @xdevplatform/xdk (${withMedia ? 'with media' : 'text only'})`);
console.log(JSON.stringify({text, mediaIds}));
const response = await client.posts.create(body);
console.log(JSON.stringify(response));
