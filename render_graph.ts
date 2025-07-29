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


const echarts_instance = echarts.init(null, null, {
    renderer: 'svg',
    ssr: true,
    width: 4000,
    height: 2500
});

for (const [table_name] of (await (await duckdb_connection.run('SELECT table_name FROM information_schema.tables WHERE NOT STARTS_WITH(table_name,"__") AND NOT ENDS_WITH(table_name,"__");')).getRows())) {
    // if (table_name != 'BEYOOOOONDS') continue

    const column_names = (await (await duckdb_connection.run('SELECT name FROM pragma_table_info(?);', [table_name])).getRows()).map(([v]) => v as string)
    console.log(`Table: ${table_name}`);
    console.log(column_names);
    const data = (await (await duckdb_connection.run('SELECT * FROM query_table(?)', [table_name])).getRows()).map(row => row.map(v => {
        if (v instanceof DuckDBTimestampTZValue) {
            return Date.parse(v.toString())
        } else if (typeof v == 'number') {
            return v
        } else {
            return null
        }
    }));
    const series: SeriesOption[] = column_names.slice(1).map((column_name) => {
        return {
            name: column_name,
            type: 'line',
            smooth: true,
            encode: {
                x: 'index',
                y: column_name
            }
        }
    })

    const chart_option: EChartsOption = {
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
