use duckdb::{params, Connection};
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{Acquire, ConnectOptions};
use std::fs;
use std::path::PathBuf;


#[tokio::main]
async fn main() {
    let mut py_db = SqliteConnectOptions::new().filename("save.sqlite").connect().await.unwrap();
    if fs::exists(PathBuf::from("data.duckdb")).unwrap() {
        fs::remove_file("data.duckdb").unwrap();
    }
    let rust_db = Connection::open("data.duckdb").unwrap();

    rust_db.execute_batch("CREATE TABLE __source__ (playlist_key TEXT PRIMARY KEY NOT NULL,db_key TEXT NOT NULL,hashtag TEXT NOT NULL,screen_name TEXT NOT NULL,is_tweet INTEGER NOT NULL);").unwrap();
    rust_db.execute_batch("CREATE TABLE __title__ (youtube_id TEXT PRIMARY KEY NOT NULL,raw_title TEXT,cleaned_title TEXT,structured_title TEXT);").unwrap();

    let table_names = sqlx::query_as("SELECT tbl_name FROM sqlite_master WHERE type = 'table';")
        .fetch_all(&mut py_db).await.unwrap().iter().map(|(tbl_name, ): &(String,)| tbl_name.to_owned()).collect::<Vec<_>>();
    for table in table_names {
        rust_db.execute(format!(r##"CREATE TABLE "{}" (index TIMESTAMPTZ PRIMARY KEY NOT NULL);"##, table).as_str(), params![]).unwrap();
        for (index, title) in sqlx::query_as(format!("SELECT \"index\",\"タイトル\" FROM '{}';", table).as_str())
            .fetch_all(&mut py_db).await.unwrap().iter().map(|(index, title, ): &(String, String)| { (index.to_owned(), title.to_owned()) }) {
            println!("{} :{}", index, title);
            rust_db.execute("INSERT OR REPLACE INTO __title__(youtube_id,cleaned_title) VALUES (?,?);",
                            params![index.strip_prefix("https://youtu.be/").unwrap(),title]).unwrap();
            rust_db.execute(format!(r##"ALTER TABLE "{}" ADD COLUMN "{}" INT32;"##, table, index.strip_prefix("https://youtu.be/").unwrap()).as_str(),
                            params![]).unwrap();
        }
        let mut queries = Vec::new();
        for date in sqlx::query_as("SELECT name FROM pragma_table_info(?);").bind(&table)
            .fetch_all(&mut py_db).await.unwrap().iter().map(|(index_name, ): &(String,)| { index_name.to_owned() }).skip(2) {
            let mut video_keys = Vec::new();
            let mut viewcounts = Vec::new();
            for (index, count) in sqlx::query_as(format!(r##"SELECT "index","{date}" FROM "{table}" ;"##).as_str())
                .fetch_all(&mut py_db).await.unwrap().iter().map(|(index, count): &(String, Option<i64>)| { (index.to_owned(), count.to_owned()) }) {
                video_keys.push(format!(r##""{}""##, index.strip_prefix("https://youtu.be/").unwrap()));
                viewcounts.push(match count {
                    None => { "NULL".to_owned() }
                    Some(v) => { v.to_string() }
                });
            }
            queries.push(format!(r##"INSERT INTO "{}"("index",{}) VALUES(timezone('Asia/Tokyo',TIMESTAMP '{}'),{});"##,
                                 table,
                                 video_keys.join(","),
                                 if date.contains(":") {
                                     date.replace("00:00:00", "08:00:00")
                                 } else {
                                     format!("{} 08:00:00", date)
                                 }, viewcounts.join(",")));
        }
        rust_db.execute_batch(queries.join("\n").as_str()).unwrap();
        // rust_db.execute(format!(r##"COPY "{}" TO 'data.parquet' (FORMAT parquet,COMPRESSION zstd)"##, table).as_str(), params![]).unwrap();
    }
    rust_db.execute("VACUUM;", []).unwrap();
    rust_db.execute("CHECKPOINT;", []).unwrap();

    let playlists_build_sql = fs::read_to_string("playlists_build.sql").unwrap();
    rust_db.execute_batch(&playlists_build_sql).unwrap();
    let __title__sql = fs::read_to_string("__title__.sql").unwrap();
    rust_db.execute_batch(&__title__sql).unwrap();
}

