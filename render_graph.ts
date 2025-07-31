// noinspection SqlNoDataSourceInspection

import {
    // configurationOptionDescriptions,
    DuckDBInstance,
    // version as duckdb_version,
    DuckDBTimestampTZValue,
    // DuckDBValue
} from 'npm:@duckdb/node-api';
import * as echarts from 'npm:echarts';
import {EChartsOption, SeriesOption} from 'npm:echarts';
import dayjs from 'npm:dayjs';
import * as fs from 'node:fs';
import {createCanvas} from 'npm:@napi-rs/canvas';
import {Resvg} from 'npm:@resvg/resvg-js'


const duckdb_instance = await DuckDBInstance.create('data.duckdb');

const duckdb_connection = await duckdb_instance.connect();

const bgColor: echarts.Color = '#FFFFFF';
const defaultFont = {
    fontFamily: 'Noto Sans JP,Noto Sans',
    fontSize: 20,
}
const echarts_instance = echarts.init(null, null, {
    renderer: 'svg',
    ssr: true,
    width: 1920,
    height: 1080
});

for (const [table_name] of (await (await duckdb_connection.run('SELECT table_name FROM information_schema.tables WHERE NOT STARTS_WITH(table_name,\'__\') AND NOT ENDS_WITH(table_name,\'__\');')).getRows())) {
    // if (table_name != '小片リサ') continue
    if ((table_name != 'BEYOOOOONDS') && (table_name != 'モーニング娘。')) continue

    const column_names = (await (await duckdb_connection.run('SELECT name FROM pragma_table_info(?);', [table_name])).getRows()).map(([v]) => v as string)
    console.log(`Table: ${table_name}`);
    console.log(column_names);
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
    const series: SeriesOption[] = await Promise.all(column_names.slice(1).map((async (column_name) => {
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
            connectNulls: true
        } as SeriesOption)
    })))
    const chart_option: EChartsOption = {
        textStyle: {
            fontFamily: defaultFont.fontFamily
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
                fontFamily: defaultFont.fontFamily
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
                ctx.font = `Regular ${defaultFont.fontSize * .8}px ${defaultFont.fontFamily}`;
                while (ctx.measureText(name + postfix).width > 300) {
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
                rotate: 30
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
        }
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
