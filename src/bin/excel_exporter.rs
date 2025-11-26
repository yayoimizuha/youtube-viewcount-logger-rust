use duckdb::{params, Connection};
use rust_xlsxwriter::{ColNum, ExcelDateTime, Format, FormatAlign, RowNum, Table, Workbook};

#[tokio::main]
async fn main() {
    let duckdb = Connection::open("data.duckdb").unwrap();
    let mut workbook = Workbook::new();
    let source = (&mut workbook).add_worksheet().set_name("source").unwrap();
    let source_vec = duckdb.prepare("SELECT * FROM __source__;").unwrap()
        .query_map(params![], |row| {
            let playlist_key = row.get::<_, String>(0).unwrap();
            let db_key = row.get::<_, String>(1).unwrap();
            let hashtag = row.get::<_, String>(2).unwrap();
            let screen_name = row.get::<_, String>(3).unwrap();
            let is_tweet = row.get::<_, bool>(4).unwrap();
            Ok((playlist_key, db_key, hashtag, screen_name, is_tweet))
        }).unwrap().flat_map(|v| { v }).map(|(playlist_key, db_key, hashtag, screen_name, is_tweet)| {
        // println!("playlist_key: {playlist_key}, db_key: {db_key}, hashtag: {hashtag}, screen_name: {screen_name}, is_tweet: {is_tweet}");
        ([playlist_key, db_key, hashtag, screen_name], is_tweet)
    }).collect::<Vec<_>>();
    source.write_row_matrix(0, 0, [["YouTubeプレイリスト キー", "DuckDBデータベースのテーブル名", "ツイート用ハッシュタグ文字列", "グループ・活動名称", "ツイートするかどうか"]]).unwrap();
    source.write_row_matrix(1, 0, source_vec.iter().map(|(v, _)| { v })).unwrap();
    source.write_row_matrix(1, 4, source_vec.iter().map(|(_, v)| { [v.clone()] })).unwrap();
    source.set_column_width(0, 50.0).unwrap();
    source.set_column_range_width(1, 3, 30.0).unwrap();
    source.set_column_width(4, 20.0).unwrap();
    source.add_table(0, 0, source_vec.len() as RowNum, 4, &Table::new().set_header_row(true).set_name("データ取得元")).unwrap();

    let title = (&mut workbook).add_worksheet().set_name("title").unwrap();
    let title_vec = duckdb.prepare("SELECT * FROM __title__;").unwrap()
        .query_map(params![], |row| {
            let youtube_id = row.get::<_, String>(0).unwrap();
            let raw_title = row.get::<_, Option<String>>(1).unwrap();
            let cleaned_title = row.get::<_, Option<String>>(2).unwrap();
            let structured_title = row.get::<_, Option<String>>(3).unwrap();
            let published_at = row.get::<_, Option<i64>>(4).unwrap();
            Ok((youtube_id, raw_title, cleaned_title, structured_title, published_at))
        }).unwrap().flat_map(|v| { v }).map(|(playlist_key, db_key, hashtag, screen_name, is_tweet)| {
        (playlist_key, [db_key, hashtag, screen_name], is_tweet)
    }).collect::<Vec<_>>();
    title.write_row_matrix(0, 0, [["YouTube動画 ID", "取得済み動画タイトル", "整形済み動画タイトル", "構造化済み動画タイトル", "動画公開時刻"]]).unwrap();
    title.write_row_matrix(1, 0, title_vec.iter().map(|(v, _, _)| { [v] })).unwrap();
    title.write_row_matrix(1, 1, title_vec.iter().map(|(_, v, _)| { v.to_vec() })).unwrap();
    title.write_row_matrix(1, 4, title_vec.iter().map(|(_, _, v)| { [v.map(|t| ExcelDateTime::from_timestamp(t / (1000 * 1000) + 9 * 60 * 60).unwrap())] })).unwrap();
    title.set_column_format(4, &Format::new().set_num_format("yyyy\"年\"mm\"月\"dd\"日\" [$-ja-JP]AM/PMhh\"時\"mm\"分\";@")).unwrap();
    title.set_column_width(0, 20.0).unwrap();
    title.set_column_width(1, 80.0).unwrap();
    title.set_column_width(2, 50.0).unwrap();
    title.set_column_width(3, 80.0).unwrap();
    title.set_column_width(4, 30.0).unwrap();
    title.add_table(0, 0, title_vec.len() as RowNum, 4, &Table::new().set_header_row(true).set_name("動画メタデータ")).unwrap();


    duckdb.prepare("SELECT table_name FROM information_schema.tables WHERE NOT STARTS_WITH(table_name,'__') AND NOT ENDS_WITH(table_name,'__');").unwrap()
        .query_map(params![], |row| {
            Ok(row.get::<_, String>(0).unwrap())
        }).unwrap().flat_map(|v| { v }).for_each(|table_name| {
        println!("Table name: {table_name}");
        let worksheet = workbook.add_worksheet().set_name(&table_name).unwrap();
        let column_names = duckdb.prepare("SELECT name FROM pragma_table_info(?) WHERE name <> 'index';").unwrap()
            .query_map(params![table_name], |row| {
                Ok(row.get::<_, String>(0).unwrap())
            }).unwrap().flat_map(|v| { v }).collect::<Vec<_>>();
        worksheet.write(0, 0, "日付").unwrap();
        worksheet.write_row_matrix(0, 1, [&column_names]).unwrap();
        let index_dates = duckdb.prepare("SELECT index FROM query_table(?);").unwrap()
            .query_map(params![table_name], |row| {
                Ok(row.get::<_, i64>(0).unwrap())
            }).unwrap().flat_map(|v| { v })
            .map(|v| ExcelDateTime::from_timestamp(v / (1000 * 1000) + 9 * 60 * 60).unwrap()).collect::<Vec<_>>();
        worksheet.write_column_matrix(1, 0, [index_dates.clone()]).unwrap();
        worksheet.set_column_format(0, &Format::new().set_num_format("yyyy\"年\"mm\"月\"dd\"日\" [$-ja-JP]AM/PMhh\"時\"mm\"分\";@")).unwrap();
        worksheet.set_column_width(0, 30.0).unwrap();
        worksheet.set_column_range_width(1, column_names.len() as ColNum, 15.0).unwrap();
        worksheet.set_row_format(0, &Format::new().set_bold().set_align(FormatAlign::Center)).unwrap();
        let data = duckdb.prepare("SELECT * EXCLUDE('index') FROM query_table(?);").unwrap()
            .query_map(params![table_name], |row| {
                Ok((0..column_names.len()).map(|idx| {
                    row.get::<_, Option<i64>>(idx).unwrap()
                }).collect::<Vec<_>>())
            }).unwrap().flat_map(|v| { v }).collect::<Vec<_>>();
        worksheet.write_row_matrix(1, 1, data.clone()).unwrap();
        worksheet.add_table(0, 0, data.len() as RowNum, column_names.len() as ColNum, &Table::new().set_header_row(true)).unwrap();
    });
    workbook.save("workdir/export.xlsx").unwrap();
}