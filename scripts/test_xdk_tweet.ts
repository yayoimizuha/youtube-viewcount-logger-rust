import {Buffer} from 'node:buffer';
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

const mediaIdFromResponse = (response: {data?: Record<string, unknown>}): string => {
    const data = response.data ?? {};
    const mediaId = data.id ?? data.media_id_string ?? data.media_id ?? data.mediaId;
    if (!mediaId) {
        throw new Error(`Media upload returned no media id: ${JSON.stringify(response)}`);
    }
    return String(mediaId);
}

const uploadPng = async (client: XClient, imagePath: string) => {
    const image = await Deno.readFile(imagePath);
    const response = await client.media.upload({
        body: {
            media: Buffer.from(image).toString('base64'),
            mediaType: 'image/png',
            mediaCategory: 'tweet_image',
        },
    });
    return mediaIdFromResponse(response);
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
const mediaIds = withMedia ? [await uploadPng(client, imagePath)] : [];
const body: Posts.CreateRequest = {
    text,
    ...(mediaIds.length ? {media: {mediaIds}} : {}),
};

console.log(`Posting via @xdevplatform/xdk (${withMedia ? 'with media' : 'text only'})`);
console.log(JSON.stringify({text, mediaIds}));
const response = await client.posts.create(body);
console.log(JSON.stringify(response));
