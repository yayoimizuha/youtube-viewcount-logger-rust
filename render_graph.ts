import { DuckDBInstance } from "npm:@duckdb/node-api";


const duckdb_instance = await DuckDBInstance.create("data.duckdb");

const duckdb_connection = await duckdb_instance.connect();
const resp = await duckdb_connection.run("SELECT * FROM __title__;");
console.log(await resp.getRowsJson());
