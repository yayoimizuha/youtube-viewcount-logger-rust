import {configurationOptionDescriptions, DuckDBInstance, version as duckdb_version,} from "npm:@duckdb/node-api";

import * as echarts from 'npm:echarts';
import {DuckDBValue} from "npm:@duckdb/node-api@1.3.2-alpha.25";

// console.log("DuckDB version: " + duckdb_version());
// console.log(configurationOptionDescriptions());
const duckdb_instance = await DuckDBInstance.create("data.duckdb");

const duckdb_connection = await duckdb_instance.connect();

const echarts_instance = echarts.init(null, null, {
    renderer: "svg",
    ssr: true,
    width: 4000,
    height: 2500
});

(await (await duckdb_connection.run("SHOW TABLES;")).getRows()).map(async ([table_name]) => {
    const column_names = await (await duckdb_connection.run("SELECT name FROM pragma_table_info(?) WHERE name <> 'index';", [table_name])).getRows();
    console.log("Table: " + table_name);
    if (!(table_name).toString().startsWith('__') || !(table_name).toString().endsWith('__')) {
        const date_index = (await (await duckdb_connection.run(`SELECT index
                                                                FROM \"${table_name}\"`)).getRows()).map(([v]) => v);
        column_names.map(([column_name]) => {
                console.log(column_name)
            }
        )
    }


});
