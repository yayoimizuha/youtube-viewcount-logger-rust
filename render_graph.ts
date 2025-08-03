// noinspection SqlNoDataSourceInspection,SqlDialectInspection

import {
    // configurationOptionDescriptions,
    DuckDBInstance,
    // version as duckdb_version,
    DuckDBTimestampTZValue,
    // DuckDBValue
} from 'npm:@duckdb/node-api';
import * as echarts from 'npm:echarts';
import {EChartsOption, LineSeriesOption} from 'npm:echarts';
import dayjs from 'npm:dayjs';
import * as fs from 'node:fs';
import {createCanvas} from 'npm:@napi-rs/canvas';
import {Resvg} from 'npm:@resvg/resvg-js'


const duckdb_instance = await DuckDBInstance.create('data.duckdb');

const duckdb_connection = await duckdb_instance.connect();

const bgColor: echarts.Color = '#ffffff';
const defaultFont = {
    fontFamily: 'BIZ UDPゴシック',
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

for (const [table_name] of (await (await duckdb_connection.run('SELECT table_name FROM information_schema.tables WHERE NOT STARTS_WITH(table_name,\'__\') AND NOT ENDS_WITH(table_name,\'__\');')).getRows())) {
    // if (table_name != '小片リサ') continue
    if ((table_name != 'BEYOOOOONDS') && (table_name != 'モーニング娘。') && (table_name != 'ochanorma')) continue

    const column_names = (await (await duckdb_connection.run('SELECT name FROM pragma_table_info(?);', [table_name])).getRows()).map(([v]) => v as string)
    console.log(`Table: ${table_name}`);
    console.log(JSON.stringify(column_names));
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
    const series_index = (await (await duckdb_connection.run(`
        WITH ranked_views AS (SELECT video_id, views, "index" AS timestamp, ROW_NUMBER() OVER(PARTITION BY video_id ORDER BY "index" DESC) AS rn
        FROM (UNPIVOT query_table(?) ON * EXCLUDE ("index") INTO NAME video_id VALUE views)
        WHERE views IS NOT NULL
            )
            , ranked_daily_views AS (
        SELECT t.cleaned_title, t.published_at, rv1.video_id, rv1.views AS total_views, CASE WHEN rv2.views IS NULL THEN (rv1.views:: DOUBLE / NULLIF (EPOCH(rv1.timestamp - t.published_at), 0))
            ELSE ((rv1.views - rv2.views):: DOUBLE / NULLIF (EPOCH(rv1.timestamp - rv2.timestamp), 0))
            END * 86400 AS daily_views
        FROM ranked_views AS rv1
            LEFT JOIN ranked_views AS rv2
        ON rv1.video_id = rv2.video_id AND rv2.rn = 2
            JOIN __title__ AS t ON rv1.video_id = t.youtube_id
        WHERE rv1.rn = 1
            )
            , final_scores AS (
        SELECT *, ((daily_views - AVG (daily_views) OVER()) / NULLIF (STDDEV_POP(daily_views) OVER(), 0) * 10) AS daily_views_deviation_score, ((EPOCH(published_at) - EPOCH(MIN (published_at) OVER())) / NULLIF (EPOCH(NOW()) - EPOCH(MIN (published_at) OVER()), 0) * 20) AS recency_deviation_score
        FROM ranked_daily_views
            ), top_n_by_score AS (
        SELECT *
        FROM final_scores
        ORDER BY (daily_views_deviation_score + recency_deviation_score) DESC NULLS LAST
            LIMIT ?
            )
        SELECT video_id
        FROM top_n_by_score
        ORDER BY total_views DESC;`, [table_name, graph_limit])).getRows()).map(([v]) => v as string);
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
            text: (((await (await duckdb_connection.run('SELECT DISTINCT screen_name FROM __source__ WHERE db_key = ? ORDER BY playlist_key;', [table_name])).getRows()).at(0) || [table_name as string]).at(0) || table_name as string).toString() || table_name as string,
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
                ctx.font = `${defaultFont.fontSize * .8}pt ${defaultFont.fontFamily} ${defaultFont.fontWeight}`;
                // console.log(ctx.font)
                while (ctx.measureText(name + postfix).width > 350) {
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
    // const chart_svg = echarts_instance.renderToSVGString()
    const chart_png = (new Resvg(echarts_instance.renderToSVGString(), {
        fitTo: {
            mode: 'zoom',
            value: 2
        },
        font: {
            fontFiles: ['./assets/BIZUDPGothic-Regular.ttf'],
            loadSystemFonts: false,
            defaultFontFamily: 'BIZ UDPゴシック',
        },
        logLevel: 'info'
    })).render().asPng()
    fs.writeFileSync(`${table_name}.png`, chart_png);
    // fs.writeFileSync(`${table_name}.svg`, echarts_instance.renderToSVGString());
    // fs.writeFileSync(`${table_name}.svg`, chart_svg);


    echarts_instance.clear();
}

duckdb_connection.closeSync()
duckdb_instance.closeSync()
echarts_instance.clear()
echarts_instance.dispose()

// SELECT T.youtube_id,T.cleaned_title,CAST(CAST(TT.value AS REAL) / EXTRACT(DAY FROM (NOW() - T.published_at)) AS INT) AS average_daily_views FROM __title__ AS T JOIN (SELECT name,value FROM (UNPIVOT 'アンジュルム' ON * EXCLUDE('index')) WHERE index = (SELECT MAX(index) FROM 'アンジュルム')) AS TT ON T.youtube_id = TT.name ORDER BY average_daily_views DESC LIMIT 20;