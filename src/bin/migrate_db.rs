use std::fs;
use std::fs::File;
use std::path::PathBuf;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{Acquire, ConnectOptions};

#[tokio::main]
async fn main() {
    let mut py_db = SqliteConnectOptions::new().filename("save.sqlite").connect().await.unwrap();
    if fs::exists(PathBuf::from("data.sqlite")).unwrap() {
        fs::remove_file("data.sqlite").unwrap();
    }
    let mut rust_db = SqliteConnectOptions::new().create_if_missing(true).filename("data.sqlite").connect().await.unwrap();
    sqlx::query("CREATE TABLE __source__ (playlist_key TEXT PRIMARY KEY NOT NULL,db_key TEXT NOT NULL,hashtag TEXT NOT NULL,screen_name TEXT NOT NULL,is_tweet INTEGER NOT NULL);").execute(&mut rust_db).await.unwrap();
    sqlx::query("CREATE TABLE __title__ (youtube_id TEXT PRIMARY KEY NOT NULL,title TEXT NOT NULL);").execute(&mut rust_db).await.unwrap();
    // return;
    let table_names = sqlx::query_as("SELECT tbl_name FROM sqlite_master WHERE type = 'table';")
        .fetch_all(&mut py_db).await.unwrap().iter().map(|(tbl_name, ): &(String,)| tbl_name.to_owned()).collect::<Vec<_>>();
    for table in table_names {
        sqlx::query(format!("CREATE TABLE '{}' ('index' DATE PRIMARY KEY NOT NULL);", table).as_str()).execute(&mut rust_db).await.unwrap();

        // let rows = sqlx::query_as("SELECT name FROM pragma_table_info(?);").bind(&table)
        //     .fetch_all(&mut py_db).await.unwrap().iter().map(|(index_name, ): &(String,)| { index_name.to_owned() }).collect::<Vec<_>>();
        // println!("{}", table);
        for (index, title) in sqlx::query_as(format!("SELECT \"index\",\"タイトル\" FROM '{}';", table).as_str())
            .fetch_all(&mut py_db).await.unwrap().iter().map(|(index, title, ): &(String, String)| { (index.to_owned(), title.to_owned()) }) {
            println!("{}:{}", index, title);
            sqlx::query("INSERT OR REPLACE INTO __title__(youtube_id,title) VALUES (?,?);")
                .bind(index.strip_prefix("https://youtu.be/").unwrap()).bind(title).execute(&mut rust_db).await.unwrap();
            sqlx::query(format!("ALTER TABLE '{}' ADD COLUMN '{}' INTEGER;", table, index.strip_prefix("https://youtu.be/").unwrap()).as_str()).execute(&mut rust_db).await.unwrap();
        }
        let mut transaction = rust_db.begin().await.unwrap();
        for date in sqlx::query_as("SELECT name FROM pragma_table_info(?);").bind(&table)
            .fetch_all(&mut py_db).await.unwrap().iter().map(|(index_name, ): &(String,)| { index_name.to_owned() }).skip(2) {
            sqlx::query(format!("INSERT INTO '{}'('index') VALUES(datetime(?))", table).as_str()).bind({
                if date.contains(":") {
                    date.replace("00:00:00", "08:00:00")
                } else {
                    format!("{} 08:00:00", date)
                }
            }).execute(&mut *transaction).await.unwrap();

            for (index, count) in sqlx::query_as(format!(r##"SELECT "index","{date}" FROM "{table}" ;"##).as_str())
                .fetch_all(&mut py_db).await.unwrap().iter().map(|(index, count): &(String, Option<i64>)| { (index.to_owned(), count.to_owned()) }) {
                sqlx::query(format!(r##"UPDATE "{table}" SET "{}" = ? WHERE "index"=?;"##, index.strip_prefix("https://youtu.be/").unwrap()).as_str()).bind(count).bind({
                    if date.contains(":") {
                        date.replace("00:00:00", "08:00:00")
                    } else {
                        format!("{} 08:00:00", date)
                    }
                }).execute(&mut *transaction).await.unwrap();
            }
        }
        transaction.commit().await.unwrap();

        // for row in rows.iter().skip(2) {
        //     // println!("{}", row);
        //     let data = sqlx::query_as(format!("SELECT \"{}\" FROM '{}';", row, table).as_str())
        //         .fetch_all(&mut py_db).await.unwrap().iter().map(|(dat, ): &(Option<i64>,)| { dat.to_owned() }).collect::<Vec<_>>();
        //     // println!("{:?}", data);
        // }
    }
}

