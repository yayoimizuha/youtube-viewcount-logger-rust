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
import moment from 'npm:moment';
import * as fs from 'node:fs';

const duckdb_instance = await DuckDBInstance.create('data.duckdb');

const duckdb_connection = await duckdb_instance.connect();

const bgColor: echarts.Color = '#f0f0f0';
const defaultFont = {
    fontFamily: 'Noto Sans JP',
    fontSize: 20,
    fontWeight: 'Regular'
}
const echarts_instance = echarts.init(null, null, {
    renderer: 'svg',
    ssr: true,
    width: 1920,
    height: 1080
});

for (const [table_name] of (await (await duckdb_connection.run('SELECT table_name FROM information_schema.tables WHERE NOT STARTS_WITH(table_name,\'__\') AND NOT ENDS_WITH(table_name,\'__\');')).getRows())) {
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
        const title = (await (await duckdb_connection.run('SELECT cleaned_title FROM __title__ WHERE youtube_id = ?', [column_name])).getRows()).at(0)!.at(0)!.toString();
        return {
            name: title || '',
            type: 'line',
            smooth: true,
            encode: {
                x: 'index',
                y: column_name
            }
        }
    })))
    const chart_option: EChartsOption = {
        title: {
            left:'center',
            text: (await (await duckdb_connection.run('SELECT DISTINCT screen_name FROM __source__ WHERE db_key = ? ORDER BY playlist_key;', [table_name])).getRows()).at(0)!.at(0)!.toString() || '',
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
                    return moment(value).format('YYYY/MM/DD');
                },
                rotate: 30
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
                    return `${Math.floor(value / 10000)}万回`
                }
            }
        },
        series: series
    }
    chart_option && echarts_instance.setOption(chart_option);

    const chart_svg = echarts_instance.renderToSVGString()
    fs.writeFileSync(`${table_name}.svg`, chart_svg);


    echarts_instance.clear();
}

duckdb_connection.closeSync()
duckdb_instance.closeSync()
echarts_instance.clear()
echarts_instance.dispose()
