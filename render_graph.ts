// noinspection SqlNoDataSourceInspection,SqlDialectInspection

import {DuckDBConnection, DuckDBInstance, DuckDBTimestampTZValue} from 'npm:@duckdb/node-api@1.5.0-r.1';
import * as echarts from 'npm:echarts';
import {EChartsOption, LineSeriesOption} from 'npm:echarts';
import dayjs from 'npm:dayjs';
import * as fs from 'node:fs';
import * as path from 'node:path';
import {createCanvas, GlobalFonts} from 'npm:@napi-rs/canvas';
import {Resvg} from 'npm:@resvg/resvg-js'
import {spawnSync} from 'node:child_process';
import * as process from 'node:process';
import {Client as XClient, OAuth1, type Posts} from 'npm:@xdevplatform/xdk';
import {createHmac, randomBytes} from 'node:crypto';
import twitterText from 'npm:twitter-text@3.1.0';


const duckdb_instance: DuckDBInstance = await DuckDBInstance.create('data.duckdb');
const duckdb_connection: DuckDBConnection = await duckdb_instance.connect();

GlobalFonts.loadFontsFromDir('assets');

const bgColor: echarts.Color = '#ffffff';
const defaultFont = {
    fontFamily: 'BIZ UDPGothic',
    fontSize: 20,
    fontWeight: 'Regular'
}

const graph_limit = 35;

const echarts_instance = echarts.init(null, null, {
    renderer: 'svg',
    ssr: true,
    width: 1920,
    height: 1080
});

const is_debug = (process.env.DEBUG || 'false').trim().toLowerCase() == 'true'

const truncateToByteLength = (text: string) => {
    const parsed = twitterText.parseTweet(text);
    if (parsed.valid) {
        return text;
    } else {
        return text.slice(
            parsed.validDisplayRangeStart,
            parsed.validDisplayRangeEnd + 1
        )
    }
}

type TweetMediaIds = [string] | [string, string] | [string, string, string] | [string, string, string, string];

const toTweetMediaIds = (mediaIds: string[]): TweetMediaIds | undefined => {
    if (mediaIds.length >= 1 && mediaIds.length <= 4) {
        return mediaIds as TweetMediaIds;
    }
    return undefined;
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
        oauth_consumer_key: process.env.TWITTER_APP_KEY as string,
        oauth_nonce: randomBytes(16).toString('hex'),
        oauth_signature_method: 'HMAC-SHA1',
        oauth_timestamp: Math.floor(Date.now() / 1000).toString(),
        oauth_token: process.env.TWITTER_ACCESS_TOKEN as string,
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
    const signingKey = `${percentEncode(process.env.TWITTER_APP_SECRET as string)}&${percentEncode(process.env.TWITTER_ACCESS_SECRET as string)}`;
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

const uploadMedia = async (image: Uint8Array): Promise<string> => {
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
    const imagePart = image.buffer.slice(
        image.byteOffset,
        image.byteOffset + image.byteLength,
    ) as ArrayBuffer;
    form.append('media', new Blob([imagePart], {type: 'image/png'}), 'image.png');
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

const postTweet = async (client: XClient, text: string, mediaIds: string[] = []) => {
    const media = toTweetMediaIds(mediaIds);
    const body: Posts.CreateRequest = {
        text,
        ...(media ? {media: {mediaIds: media}} : {})
    };
    await client.posts.create(body);
}

const twitterClient = await (async () => {
    const required = ['TWITTER_APP_KEY', 'TWITTER_APP_SECRET', 'TWITTER_ACCESS_TOKEN', 'TWITTER_ACCESS_SECRET'] as const;
    if (!required.every(k => process.env[k])) {
        console.warn('Twitter credentials not fully set in env; tweeting will be skipped.');
        return null;
    }

    const oauth1 = new OAuth1({
        apiKey: process.env.TWITTER_APP_KEY as string,
        apiSecret: process.env.TWITTER_APP_SECRET as string,
        accessToken: process.env.TWITTER_ACCESS_TOKEN as string,
        accessTokenSecret: process.env.TWITTER_ACCESS_SECRET as string,
        callback: 'oob',
    });
    const client = new XClient({oauth1});

    // URL付き投稿は Content: Create (with URL) 扱いで高価なので停止する。
    // try {
    //     await postTweet(client,
    //         "毎日の最新データはこちらから👉https://github.com/yayoimizuha/youtube-viewcount-logger-python/releases/latest\n" +
    //         "以下のサイトでグループごとの再生回数のグラフを見られます！\n" +
    //         "拡大縮小したり、表示したい曲を選択して表示できたりして、毎日の画像ツイートより見やすくなっています！\n" +
    //         "https://viewcount-logger-20043.web.app/"
    //     )
    // } catch (e) {
    //     console.error('Tweet failed for:', e);
    //
    // }

    return client;
})();

for (const [table_name] of (await (await duckdb_connection.run('SELECT t1.table_name FROM information_schema.tables AS t1 LEFT JOIN (SELECT db_key,MIN(rowid) AS min_rowid FROM __source__ GROUP BY db_key) AS t2 ON t1.table_name = t2.db_key WHERE NOT STARTS_WITH(t1.table_name, \'__\') AND NOT ENDS_WITH(t1.table_name, \'__\') ORDER BY CASE WHEN t2.min_rowid IS NULL THEN 1 ELSE 0 END,t2.min_rowid;')).getRows())) {
    // if (table_name != '小片リサ') continue
    // if ((table_name != 'BEYOOOOONDS') && (table_name != 'モーニング娘。') && (table_name != 'ochanorma')) continue

    // if ((table_name != '鈴木愛理') && (table_name != 'Buono!')) continue
    // if (table_name != 'アンジュルム') continue
    const is_tweet: boolean = (await (await duckdb_connection.run('SELECT COALESCE(BOOL_OR(is_tweet::BOOLEAN),FALSE) FROM __source__ WHERE db_key = ?;', [table_name])).getRows()).map(([v]) => v as boolean)[0];
    if (!is_debug) {
        if (!is_tweet) {
            // console.log(table_name, is_tweet);
            console.log(`Skipping ${table_name} ...`);
            continue
        }
    }
    if (((((await (await duckdb_connection.run(fs.readFileSync('assets/max_daily_viewcount.sql', {
        encoding: 'utf-8',
        flag: 'r'
    }), [table_name])).getRows()).at(0) || [0]).at(0) || 0) as number) < 1000) {
        console.log(`Too few to tweet. Skipping ${table_name} ...`);
        continue
    }

    const title = (((await (await duckdb_connection.run('SELECT DISTINCT screen_name FROM __source__ WHERE db_key = ? ORDER BY playlist_key;', [table_name])).getRows()).at(0) || [table_name as string]).at(0) || table_name as string).toString() || table_name as string;
    const hashtag = (((await (await duckdb_connection.run('SELECT DISTINCT hashtag FROM __source__ WHERE db_key = ? ORDER BY playlist_key;', [table_name])).getRows()).at(0) || [table_name as string]).at(0) || table_name as string).toString() || table_name as string;

    const column_names = (await (await duckdb_connection.run('SELECT name FROM pragma_table_info(?);', [table_name])).getRows()).map(([v]) => v as string)
    console.log(`Table: ${table_name}`);
    // console.log(JSON.stringify(column_names));
    let max_count: number = -Infinity;
    const data = (await (await duckdb_connection.run('SELECT * FROM query_table(?)', [table_name])).getRows()).map(row => row.map(v => {
        if (v instanceof DuckDBTimestampTZValue) {
            return Date.parse(v.toString())
        } else if (typeof v == 'number') {
            max_count = Math.max(v, max_count)
            return v
        } else {
            return null
        }
    }));
    const series_index = (await (await duckdb_connection.run(fs.readFileSync('assets/graph_query.sql', {
        encoding: 'utf-8',
        flag: 'r'
    }), [table_name, graph_limit])).getRows()).map(([v]) => v as string);
    // console.log(series_index);
    const raw_series: LineSeriesOption[] = await Promise.all(column_names.slice(1).map((async (column_name) => {
        const title = (((await (await duckdb_connection.run('SELECT cleaned_title FROM __title__ WHERE youtube_id = ? AND cleaned_title IS NOT NULL', [column_name])).getRows()).at(0) || [column_name]).at(0) || column_name).toString() || column_name;
        return ({
            name: title || '',
            type: 'line',
            smooth: true,
            encode: {
                x: 'index',
                y: column_name
            },
            symbol: 'circle',
            symbolSize: 2.5,
            lineStyle: {
                type: 'solid',
                width: .8,
                dashOffset: 2
            },
            connectNulls: true,

        } as LineSeriesOption)
    })))
    const series = series_index.map((youtube_id) => {
        return raw_series.find((elm) => elm?.encode?.y == youtube_id)
    }).filter((elm) => elm != undefined);

    const chart_option: EChartsOption = {
        textStyle: {
            fontFamily: defaultFont.fontFamily,
            fontSize: defaultFont.fontSize,
        },
        animation: false,
        title: {
            left: 'center',
            textStyle: {
                fontFamily: defaultFont.fontFamily,
                fontSize: defaultFont.fontSize * 1.5
            },
            text: title,
        },
        backgroundColor: bgColor,
        dataset: {
            source: data,
            dimensions: column_names
        },
        xAxis: {
            type: 'time',
            axisLabel: {
                formatter(value, _index, _extra) {
                    const date = dayjs(value)
                    return `${date.format('YYYY').padStart(4, ' ')}/${date.format('M').padStart(2, ' ')}/${date.format('D').padStart(2, ' ')}`
                },
                rotate: 30,
                fontSize: defaultFont.fontSize * .8,
                fontFamily: defaultFont.fontFamily,
                fontWeight: 'normal'
            },
            splitLine: {
                show: true
            }
        },
        grid: {
            right: 400,
            left: 100,
        },
        legend: {
            type: 'scroll',
            orient: 'vertical',
            align: 'left',
            right: 20,
            top: 20,
            textStyle: {
                fontSize: defaultFont.fontSize * .8,
                fontFamily: defaultFont.fontFamily,
            },
            formatter(name) {
                let postfix = '';
                const canvas = createCanvas(1, 1);
                const ctx = canvas.getContext('2d');
                ctx.font = `${defaultFont.fontSize * .8}pt ${defaultFont.fontFamily}`;
                while (ctx.measureText(name + postfix).width > 400) {
                    postfix = '...'
                    name = [...name].slice(0, name.length - 1).join('')
                    // console.log('name:', name)
                }
                return name + postfix
            },
            pageIconColor: bgColor,
            pageIconInactiveColor: bgColor,
            pageTextStyle: {
                color: bgColor
            }
        },
        yAxis: {
            min: 0,
            position: 'left',
            axisLabel: {
                formatter(value: number) {
                    if (value == 0) {
                        return '0回'
                    } else {
                        return `${Math.floor(value / 10000)}万回`
                    }
                },
                rotate: 30,
                fontSize: defaultFont.fontSize * .7
            }
        },
        series: series
    }
    chart_option && echarts_instance.setOption(chart_option);

    const chart_png = (new Resvg(echarts_instance.renderToSVGString(), {
        fitTo: {
            mode: 'zoom',
            value: 2
        },
        font: {
            fontFiles: ['./assets/BIZUDPGothic-Regular.ttf'],
            loadSystemFonts: false,
            defaultFontFamily: 'BIZ UDPGothic',
        },
        logLevel: 'info'
    })).render().asPng()
    fs.writeFileSync(path.join(...[process.cwd(), 'debug', `${table_name}.graph.png`]), chart_png);

    echarts_instance.clear();

    const typst_array = (await (await duckdb_connection.run(fs.readFileSync('assets/typst_table_query.sql', {
        encoding: 'utf-8',
        flag: 'r'
    }), [table_name, 25])).getRows()).map(
        ([title, total_views, daily_views, momentum]) => {
            return `[ #[\` ${(title as string).replace(/(`)/g, '\\$1')} \`].text ],[#[ ${total_views as number}回 ]],[#[ ${daily_views as number}回 ]], ${(momentum as string | null) ?? '[#[N/A]]'},`
        }).join('\n');
    // console.log(typst_array);
    const typst_src = fs.readFileSync('assets/template.typ', 'utf-8')
        .replace('#let data = ()', `#let data = (${typst_array})`)
        .replace('#let title = ""', `#let title = "${title}"`)
    // execute with typst stdin
    const res = spawnSync('typst', ['compile', '--format', 'png', '--ppi', '200', '--font-path', 'assets', '-', '-'], {
        input: typst_src
    })
    // console.log(typst_src)
    const table_png = res.stdout;

    console.warn((new TextDecoder()).decode(res.stderr))

    fs.writeFileSync(path.join(...[process.cwd(), 'debug', `${table_name}.typst.png`]), table_png);

    const tweet_rows = await (await duckdb_connection.run(fs.readFileSync('assets/tweet_query.sql', {
        encoding: 'utf-8',
        flag: 'r'
    }), [table_name])).getRows() as string[][];
    const tweet_text = tweet_rows
        .map((row: string[], index: number) => String.fromCodePoint(0x1F947 + index) + row.join(' '))
        .join('\n');
    console.log(truncateToByteLength(`#hpytvc 昨日からの再生回数: #${hashtag}\n${tweet_text}`))

    if (twitterClient && !is_debug) {
        try {
            const mediaIds = [] as string[];
            try {
                for (const image of [chart_png, table_png]) {
                    mediaIds.push(await uploadMedia(image));
                }
            } catch (e) {
                console.error(`Media upload failed for ${table_name}:`, e);
            }

            const text = truncateToByteLength(`#hpytvc 昨日からの再生回数: #${hashtag}\n${tweet_text}`);
            const media = toTweetMediaIds(mediaIds);
            try {
                await postTweet(twitterClient, text, media ?? []);
            } catch (e) {
                if (!media) {
                    throw e;
                }
                console.error(`Tweet with media failed for ${table_name}; retrying text-only:`, e);
                await postTweet(twitterClient, text);
            }
            console.log(`Tweet posted for ${table_name}`);
        } catch (e) {
            console.error(`Tweet failed for ${table_name}:`, e);
        }
    }
}

duckdb_connection.closeSync()
duckdb_instance.closeSync()
echarts_instance.clear()
echarts_instance.dispose()
process.exit(0)
