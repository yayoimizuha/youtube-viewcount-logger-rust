<script lang="ts">
	/* eslint-disable @typescript-eslint/no-unused-vars */
	import * as duckdb from '@duckdb/duckdb-wasm';
	import { DuckDBAccessMode, LogLevel } from '@duckdb/duckdb-wasm';
	import duckdb_wasm from '@duckdb/duckdb-wasm/dist/duckdb-mvp.wasm?url';
	import mvp_worker from '@duckdb/duckdb-wasm/dist/duckdb-browser-mvp.worker.js?url';
	import duckdb_wasm_eh from '@duckdb/duckdb-wasm/dist/duckdb-eh.wasm?url';
	import eh_worker from '@duckdb/duckdb-wasm/dist/duckdb-browser-eh.worker.js?url';

	const initiateDuckDB = async () => {
		const MANUAL_BUNDLES: duckdb.DuckDBBundles = {
			mvp: {
				mainModule: duckdb_wasm,
				mainWorker: mvp_worker
			},
			eh: {
				mainModule: duckdb_wasm_eh,
				mainWorker: eh_worker
			}
		};
		// Select a bundle based on browser checks
		const bundle = await duckdb.selectBundle(MANUAL_BUNDLES);
		// Instantiate the asynchronous version of DuckDB-wasm
		const worker = new Worker(bundle.mainWorker!);
		const logger = new duckdb.ConsoleLogger(LogLevel.DEBUG);
		const db = new duckdb.AsyncDuckDB(logger, worker);
		await db.instantiate(bundle.mainModule, bundle.pthreadWorker);

		const db_url = 'https://cdn.jsdelivr.net/gh/yayoimizuha/youtube-viewcount-logger-rust@master/data.sqlite';
		const response = await fetch(db_url);
		if (!response.ok) {
			throw new Error(`Failed to fetch ${db_url}: ${response.statusText}`);
		}
		const buffer = await response.arrayBuffer();
		console.log('File fetched successfully.');

		const root = await navigator.storage.getDirectory();
		const fileHandle = await root.getFileHandle('data.sqlite', { create: true });
		const writable = await fileHandle.createWritable();
		await writable.write(buffer);
		await writable.close();

		return db;
	};

	const list_tables = async (db: duckdb.AsyncDuckDB) => {
		const conn = await db.connect();
		try {
			await conn.query(`INSTALL sqlite;`);
			await conn.query(`LOAD sqlite;`);
		} catch (e) {
			console.warn('SQLite extension install/load warning (may be already loaded):', e);
		}
		console.log('SQLite installed.');

		console.log(await conn.query('ATTACH \'data.sqlite\' AS data (TYPE sqlite);'));

		console.log('DATABASES:', (await conn.query('SHOW DATABASES;')).toArray().map(row => row.database_name));
		console.log('USE:', (await conn.query('USE data;')));
		console.log('USE:', (await conn.query('FROM duckdb_extensions() WHERE loaded;')));
		console.log('TABLES:', (await conn.query('SHOW TABLES;')).toArray().map(row => row.toJSON()));

		const resp = await conn.query('SELECT * FROM sqlite_master;');
		console.log(resp.toArray().map(v => v.toJSON()));
		return resp.toArray().map(v => v.toJSON());

	};
</script>
{#await initiateDuckDB()}
	<p>Initializing DB...</p>
{:then db}
	{#await list_tables(db)}
		<p>getting tables...</p>
	{:then table_lists}
		{#each table_lists as table_list}
			<p>{table_list}</p>
		{/each}

	{/await}
{/await}
<h1>Welcome to SvelteKit</h1>
<p>Visit <a href="https://svelte.dev/docs/kit">svelte.dev/docs/kit</a> to read the documentation</p>
