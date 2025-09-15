// noinspection SqlNoDataSourceInspection,SqlDialectInspection

import {DuckDBInstance, DuckDBTimestampTZValue,} from 'npm:@duckdb/node-api';
import * as echarts from 'npm:echarts';
import {EChartsOption, LineSeriesOption} from 'npm:echarts';
import dayjs from 'npm:dayjs';
import * as fs from 'node:fs';
import * as path from 'node:path';
import {createCanvas, GlobalFonts} from 'npm:@napi-rs/canvas';
import {Resvg} from 'npm:@resvg/resvg-js'
import {spawnSync} from 'node:child_process';
import * as process from 'node:process';
import {TwitterApi} from 'npm:twitter-api-v2';
import {Buffer} from 'node:buffer';


const duckdb_instance = await DuckDBInstance.create('data.duckdb');
const duckdb_connection = await duckdb_instance.connect();

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

// Twitter クライアント初期化 (環境変数が無い場合は null)
const twitterClient = await (async () => {
    const required = ['TWITTER_APP_KEY', 'TWITTER_APP_SECRET', 'TWITTER_ACCESS_TOKEN', 'TWITTER_ACCESS_SECRET'] as const;
    if (!required.every(k => process.env[k]) && is_debug) {
        console.warn('Twitter credentials not fully set in env; tweeting will be skipped.');
        return null;
    }
    const twitter_api: TwitterApi = new TwitterApi({
        appKey: process.env.TWITTER_APP_KEY as string,
        appSecret: process.env.TWITTER_APP_SECRET as string,
        accessToken: process.env.TWITTER_ACCESS_TOKEN as string,
        accessSecret: process.env.TWITTER_ACCESS_SECRET as string,
    });

    try {
        await twitter_api.v2.tweet(
            "毎日の最新データはこちらから👉https://github.com/yayoimizuha/youtube-viewcount-logger-python/releases/latest\n" +
            "以下のサイトでグループごとの再生回数のグラフを見られます！\n" +
            "拡大縮小したり、表示したい曲を選択して表示できたりして、毎日の画像ツイートより見やすくなっています！\n" +
            "https://viewcount-logger-20043.web.app/"
        )
    } catch (e) {
        console.error('Tweet failed for:', e);

    }

    return twitter_api;
})();

const truncateToByteLength = (text: string, maxBytes: number) => {
    const encoder = new TextEncoder();
    const encodedText = encoder.encode(text);

    if (encodedText.length <= maxBytes) {
        return text;
    }
    let truncatedText = text;
    while (encoder.encode(truncatedText).length > maxBytes) {
        truncatedText = truncatedText.slice(0, -1);
    }
    return truncatedText;
}

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
    if ((((await (await duckdb_connection.run(fs.readFileSync('assets/max_daily_viewcount.sql', {
        encoding: 'utf-8',
        flag: 'r'
    }), [table_name])).getRows()).at(0) || [0]).at(0) || 0) < 1000) {
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

    console.error((new TextDecoder()).decode(res.stderr))

    fs.writeFileSync(path.join(...[process.cwd(), 'debug', `${table_name}.typst.png`]), table_png);

    const tweet_text = (await (await duckdb_connection.run(fs.readFileSync('assets/tweet_query.sql', {
        encoding: 'utf-8',
        flag: 'r'
    }), [table_name])).getRows()).entries().map(
        ([index, row]) => String.fromCodePoint(0x1F947 + index) + (row as string[]).join(' ')
    ).toArray().join('\n');
    console.log(tweet_text);

    const upload_media = async (image: Buffer<ArrayBufferLike>, twitter: TwitterApi) => {
        return await twitter.v1.uploadMedia(image, {mimeType: 'image/png'});
    }

    if (twitterClient && !is_debug) {
        try {
            const mediaIds = [] as string[];
            try {
                for (const image of [chart_png, table_png]) {
                    mediaIds.push(await upload_media(Buffer.from(image), twitterClient));
                }
            } catch (e) {
                console.error(`Media upload failed for ${table_name}:`, e);
            }

            await twitterClient.v2.tweet({
                text: truncateToByteLength(`#hpytvc 昨日からの再生回数: #${hashtag}\n${tweet_text}`, 280),
                media: {media_ids: mediaIds as [string, string] | [string]}
            });
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
