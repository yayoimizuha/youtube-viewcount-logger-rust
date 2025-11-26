// noinspection SqlNoDataSourceInspection,SqlDialectInspection

import { DuckDBConnection, DuckDBInstance, DuckDBTimestampTZValue, DuckDBValue } from 'npm:@duckdb/node-api@1.3.2-alpha.26';
import * as echarts from 'npm:echarts';
import { EChartsOption, LineSeriesOption } from 'npm:echarts';
import dayjs from 'npm:dayjs';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { createCanvas, GlobalFonts } from 'npm:@napi-rs/canvas';
import { spawnSync } from 'node:child_process';
import * as process from 'node:process';

const duckdbInstance: DuckDBInstance = await DuckDBInstance.create('data.duckdb');
const duckdbConnection: DuckDBConnection = await duckdbInstance.connect();

GlobalFonts.loadFontsFromDir('assets');

const bgColor = '#ffffff';
const defaultFont = {
    fontFamily: 'BIZ UDPGothic',
    fontSize: 20,
    fontWeight: 'Regular'
};
const graphLimit = 35;
const echartsInstance = echarts.init(null, null, {
    renderer: 'svg',
    ssr: true,
    width: 1920,
    height: 1080
});

const workdir = path.join(process.cwd(), 'workdir');
fs.mkdirSync(workdir, { recursive: true });
const reportTypPath = path.join(workdir, 'group_report.typ');
const reportPdfPath = path.join(workdir, 'group_report.pdf');
const releaseMarkdownPath = path.join(workdir, 'github_release.md');

const graphQuerySql = readSqlFile('assets/graph_query.sql');
const tableQuerySql = readSqlFile('assets/typst_table_query.sql');
const tweetQuerySql = readSqlFile('assets/tweet_query.sql');

type TableSummaryRow = [string, number, number, string | null];
type TweetRow = [string, string];

const escapeTypstText = (text: string) => text
    .replace(/\\/g, '\\\\')
    .replace(/`/g, '\\`')
    .replace(/\[/g, '\\[')
    .replace(/\]/g, '\\]')
    .replace(/\{/g, '\\{')
    .replace(/\}/g, '\\}')
    .replace(/#/g, '\\#');

const renderTablePrelude = `#let new = table.cell(
    fill: yellow,
)[#emoji.new]

#let up = table.cell(
    fill: rgb("#74ff84"),
)[#sym.arrow.tr]
#let down = table.cell(
    fill: rgb("#ff74ae"),
)[#sym.arrow.br]
#let equal = table.cell(
    fill: rgb("#c7c7c7"),
)[#sym.arrow.r]

#show table.cell.where(x: 3): set text(size: 12pt, font: "Noto Color Emoji")

#let render_table(data) = [
    #align(center)[#figure(
        table(
            columns: (200pt, auto, auto, auto),
            align: (x, y) => if y == 0 or x != 0  {
                center + horizon
            } else {
                (left, right, right, center).at(x)
            },

            stroke: 0.5pt + gray,
            fill: (x, y) => if y == 0 { rgb("#5191f7") } else if calc.even(y) { rgb("#e5e8f5c7") },
            inset: 8pt,

            table.header(
                [*楽曲名*],
                [ *#{ datetime.today().display("[year]年[month padding:none]月[day padding:none]日") }\ 時点での総再生回数*],
                [*1日当たりの\ 再生回数*],
                [*トレンド*],
            ),
            ..data,
        ),
    )]
];`;

await (async () => {
    try {
        await main();
    } finally {
        duckdbConnection.closeSync();
        duckdbInstance.closeSync();
        echartsInstance.clear();
        echartsInstance.dispose();
    }
})();

async function main(): Promise<void> {
    const { typstSections, releaseSections } = await collectRenderAssets();
    if (!typstSections.length) {
        console.warn('No renderable tables were found.');
        return;
    }

    writeGithubRelease(releaseSections);

    const typstDocument = buildTypstDocument(typstSections);
    fs.writeFileSync(reportTypPath, typstDocument, { encoding: 'utf-8' });

    const reportResult = spawnSync('typst', ['compile', '--font-path', 'assets', reportTypPath, reportPdfPath], { encoding: 'utf-8' });
    if (reportResult.status !== 0) {
        console.error(reportResult.stderr);
        throw new Error('Failed to compile group report PDF');
    }

    console.log(`PDF generated at ${reportPdfPath}`);
}

function buildTypstDocument(sections: string[]): string {
    return [
        renderTablePrelude,
        '#set text(font: "BIZ UDPGothic", size: 14pt, lang: "ja")',
        '#set page(width: 800pt, height: auto, margin: (top: 24pt, bottom: 24pt, left: 48pt, right: 48pt))',
        ...sections
    ].join('\n\n');
}

type ReleaseSection = {
    heading: string;
    rankings: TweetRow[];
};

async function collectRenderAssets(): Promise<{ typstSections: string[]; releaseSections: ReleaseSection[]; }> {
    const typstSections: string[] = [];
    const releaseSections: ReleaseSection[] = [];
    const tableNames = await fetchRenderableTables();

    for (const tableName of tableNames) {
        const title = await fetchTableTitle(tableName);
        const columnNames = await fetchColumnNames(tableName);
        const normalizedTitle = title.replace(/\s+/g, ' ');

        console.log(`Rendering assets for ${tableName}`);

        const svgLiteral = await renderChartSvg({ tableName, title, columnNames });
        const tableRows = await fetchSummaryRows(tableName);
        const tweetRows = await fetchTweetRows(tableName);
        const topThreeRows = selectTopThree(tweetRows);

        const rankingBlock = buildRankingBlock(tweetRows);
        const heading = escapeTypstText(normalizedTitle);

        typstSections.push(
            buildGraphSection({ heading, rankingBlock, svgLiteral, isFirstPage: typstSections.length === 0 }),
            buildTableSection(tableRows)
        );

        if (topThreeRows.length) {
            releaseSections.push({ heading: normalizedTitle, rankings: topThreeRows });
        }
    }

    return { typstSections, releaseSections };
}

type ChartRenderContext = {
    tableName: string;
    title: string;
    columnNames: string[];
};

type ChartOptionInput = {
    title: string;
    columnNames: string[];
    dataset: (number | null)[][];
    series: LineSeriesOption[];
};

async function renderChartSvg(context: ChartRenderContext): Promise<string> {
    const dataset = await fetchChartDataset(context.tableName);
    const seriesOrder = await fetchSeriesOrder(context.tableName);
    const rawSeries = await createSeriesDefinitions(context.columnNames.slice(1));
    const series = filterSeries(rawSeries, seriesOrder);

    const chartOption = buildChartOption({
        title: context.title,
        columnNames: context.columnNames,
        dataset,
        series
    });

    echartsInstance.setOption(chartOption);
    const svgLiteral = JSON.stringify(echartsInstance.renderToSVGString());
    echartsInstance.clear();
    return svgLiteral;
}

function buildChartOption({ title, columnNames, dataset, series }: ChartOptionInput): EChartsOption {
    return {
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
            source: dataset,
            dimensions: columnNames
        },
        xAxis: {
            type: 'time',
            axisLabel: {
                formatter(value) {
                    const date = dayjs(value);
                    return `${date.format('YYYY').padStart(4, ' ')}/${date.format('M').padStart(2, ' ')}/${date.format('D').padStart(2, ' ')}`;
                },
                rotate: 30,
                fontSize: defaultFont.fontSize * 0.8,
                fontFamily: defaultFont.fontFamily,
                fontWeight: 'normal'
            },
            splitLine: { show: true }
        },
        grid: { right: 400, left: 100 },
        legend: {
            type: 'scroll',
            orient: 'vertical',
            align: 'left',
            right: 20,
            top: 20,
            textStyle: {
                fontSize: defaultFont.fontSize * 0.8,
                fontFamily: defaultFont.fontFamily,
            },
            formatter: truncateLegendLabel,
            pageIconColor: bgColor,
            pageIconInactiveColor: bgColor,
            pageTextStyle: { color: bgColor }
        },
        yAxis: {
            min: 0,
            position: 'left',
            axisLabel: {
                formatter(value: number) {
                    if (value === 0) {
                        return '0回';
                    }
                    return `${Math.floor(value / 10000)}万回`;
                },
                rotate: 30,
                fontSize: defaultFont.fontSize * 0.7
            }
        },
        series
    };
}

function truncateLegendLabel(input: string): string {
    let postfix = '';
    let glyphs = [...input];
    const canvas = createCanvas(1, 1);
    const ctx = canvas.getContext('2d');
    ctx.font = `${defaultFont.fontSize * 0.8}pt ${defaultFont.fontFamily}`;

    while (ctx.measureText(glyphs.join('') + postfix).width > 400 && glyphs.length > 0) {
        postfix = '...';
        glyphs = glyphs.slice(0, glyphs.length - 1);
    }

    return glyphs.join('') + postfix;
}

async function fetchChartDataset(tableName: string): Promise<(number | null)[][]> {
    const rows = await runQuery<Array<number | DuckDBTimestampTZValue | null>>('SELECT * FROM query_table(?)', [tableName]);
    return rows.map((row) => row.map(normalizeValue));
}

function normalizeValue(value: number | DuckDBTimestampTZValue | null): number | null {
    if (value instanceof DuckDBTimestampTZValue) {
        return Date.parse(value.toString());
    }
    if (typeof value === 'number') {
        return value;
    }
    return null;
}

async function fetchSeriesOrder(tableName: string): Promise<string[]> {
    const rows = await runQuery<[string]>(graphQuerySql, [tableName, graphLimit]);
    return rows.map(([value]) => value as string);
}

function createSeriesDefinitions(columnNames: string[]): Promise<LineSeriesOption[]> {
    return Promise.all(columnNames.map(async (columnName) => {
        const cleanedTitle = await fetchCleanedTitle(columnName);
        return {
            name: cleanedTitle || '',
            type: 'line',
            smooth: true,
            encode: { x: 'index', y: columnName },
            symbol: 'circle',
            symbolSize: 2.5,
            lineStyle: { type: 'solid', width: 0.8, dashOffset: 2 },
            connectNulls: true,
        } as LineSeriesOption;
    }));
}

function filterSeries(rawSeries: LineSeriesOption[], order: string[]): LineSeriesOption[] {
    return order
        .map((youtubeId) => rawSeries.find((series) => series?.encode?.y === youtubeId))
        .filter((series): series is LineSeriesOption => Boolean(series));
}

async function fetchCleanedTitle(columnName: string): Promise<string> {
    const rows = await runQuery<[string]>('SELECT cleaned_title FROM __title__ WHERE youtube_id = ? AND cleaned_title IS NOT NULL', [columnName]);
    const fallback = columnName.toString();
    const cleanedTitle = rows.at(0)?.[0];
    return (cleanedTitle?.toString() || fallback);
}

function fetchSummaryRows(tableName: string): Promise<TableSummaryRow[]> {
    return runQuery<TableSummaryRow>(tableQuerySql, [tableName, 25]);
}

function fetchTweetRows(tableName: string): Promise<TweetRow[]> {
    return runQuery<TweetRow>(tweetQuerySql, [tableName]);
}

function buildRankingBlock(rows: TweetRow[]): string {
    if (!rows.length) {
        return 'ランキングデータがありません。';
    }

    return rows
        .map(([titleText, dailyText], idx) => `${String.fromCodePoint(0x1F947 + idx)} ${escapeTypstText(titleText)} ${escapeTypstText(dailyText)} \\`)
        .join('\n');
}

function buildGraphSection(args: { heading: string; rankingBlock: string; svgLiteral: string; isFirstPage: boolean; }): string {
    const prefix = args.isFirstPage ? '' : '#pagebreak()\n';
    return `${prefix}= ${args.heading}\n`
        + `上位3位 \\ `
        + `${args.rankingBlock}\n`
        + `#figure(image(bytes(${args.svgLiteral}), width: 95%))`;
}

function buildTableSection(tableRows: TableSummaryRow[]): string {
    return `#pagebreak()\n`
        + `#render_table(${formatTypstTableData(tableRows)})`;
}

function writeGithubRelease(sections: ReleaseSection[]): void {
    if (!sections.length) {
        console.warn('No release content was found; github_release.md will not be written.');
        return;
    }

    const markdown = buildReleaseMarkdown(sections);
    fs.writeFileSync(releaseMarkdownPath, markdown, { encoding: 'utf-8' });
    console.log(`GitHub release notes generated at ${releaseMarkdownPath}`);
}

function buildReleaseMarkdown(sections: ReleaseSection[]): string {
    const lines: string[] = ['# デイリーレポート\n'];

    for (const section of sections) {
        lines.push(`## ${escapeMarkdownText(section.heading)}\n`);

        section.rankings.forEach(([titleText, dailyText], idx) => {
            const medal = String.fromCodePoint(0x1F947 + idx);
            lines.push(`${idx + 1}. ${medal} ${escapeMarkdownText(titleText)} ${escapeMarkdownText(dailyText)}`);
        });

        lines.push('');
    }

    return lines.join('\n').trimEnd() + '\n';
}

function escapeMarkdownText(input: string): string {
    return input
        .replace(/\\/g, '\\\\')
        .replace(/([`*_{}\[\]()#+\-.!|>])/g, '\\$1')
        .replace(/\s+/g, ' ');
}

function selectTopThree(rows: TweetRow[]): TweetRow[] {
    return rows.slice(0, 3);
}

function formatTypstTableData(rows: TableSummaryRow[]): string {
    if (!rows.length) {
        return '()';
    }

    const body = rows
        .map(([rowTitle, totalViews, dailyViews, momentum]) =>
            `[ #[\` ${rowTitle.replace(/(`)/g, '\\$1')} \`].text ],[#[ ${totalViews}回 ]],[#[ ${dailyViews}回 ]], ${(momentum ?? '[#[N/A]]')},`
        )
        .join('\n');

    return `(${body})`;
}

async function fetchRenderableTables(): Promise<string[]> {
    const rows = await runQuery<[string]>('SELECT t1.table_name FROM information_schema.tables AS t1 LEFT JOIN (SELECT db_key,MIN(rowid) AS min_rowid FROM __source__ GROUP BY db_key) AS t2 ON t1.table_name = t2.db_key WHERE NOT STARTS_WITH(t1.table_name, \'__\') AND NOT ENDS_WITH(t1.table_name, \'__\') ORDER BY CASE WHEN t2.min_rowid IS NULL THEN 1 ELSE 0 END,t2.min_rowid;');
    return rows.map(([tableName]) => tableName as string);
}

async function fetchTableTitle(tableName: string): Promise<string> {
    const rows = await runQuery<[string]>('SELECT DISTINCT screen_name FROM __source__ WHERE db_key = ? ORDER BY playlist_key;', [tableName]);
    const fallback = tableName.toString();
    const resolved = rows.at(0)?.[0];
    return (resolved?.toString() || fallback);
}

async function fetchColumnNames(tableName: string): Promise<string[]> {
    const rows = await runQuery<[string]>('SELECT name FROM pragma_table_info(?);', [tableName]);
    return rows.map(([value]) => value as string);
}

async function runQuery<TRow extends unknown[]>(sql: string, params?: DuckDBValue[] | Record<string, DuckDBValue>): Promise<TRow[]> {
    const statement = await duckdbConnection.run(sql, params);
    return (await statement.getRows()) as TRow[];
}

function readSqlFile(relativePath: string): string {
    return fs.readFileSync(relativePath, { encoding: 'utf-8', flag: 'r' });
}
