use std::fs;
use std::fs::File;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::ConnectOptions;

#[tokio::main]
async fn main() {
    let mut py_db = SqliteConnectOptions::new().filename("save.sqlite").connect().await.unwrap();
    fs::remove_file("data.sqlite").unwrap();
    let mut rust_db = SqliteConnectOptions::new().create_if_missing(true).filename("data.sqlite").connect().await.unwrap();
    sqlx::query("CREATE TABLE __source__ (playlist_key TEXT PRIMARY KEY NOT NULL,db_key TEXT NOT NULL,hashtag TEXT NOT NULL,screen_name TEXT NOT NULL,is_tweet INTEGER NOT NULL);").execute(&mut rust_db).await.unwrap();
    // return;
    let table_names = sqlx::query_as("SELECT tbl_name FROM sqlite_master WHERE type = 'table';")
        .fetch_all(&mut py_db).await.unwrap().iter().map(|(tbl_name, ): &(String,)| tbl_name.to_owned()).collect::<Vec<_>>();
    for table in table_names {
        let rows = sqlx::query_as("SELECT name FROM pragma_table_info(?);").bind(&table)
            .fetch_all(&mut py_db).await.unwrap().iter().map(|(index_name, ): &(String,)| { index_name.to_owned() }).collect::<Vec<_>>();
        println!("{}", table);
        let data = sqlx::query_as(format!("SELECT \"index\" FROM '{}';", table).as_str())
            .fetch_all(&mut py_db).await.unwrap().iter().map(|(dat, ): &(String,)| { dat.to_owned() }).collect::<Vec<_>>();
        println!("{:?}", data);
        let data = sqlx::query_as(format!("SELECT \"タイトル\" FROM '{}';", table).as_str())
            .fetch_all(&mut py_db).await.unwrap().iter().map(|(dat, ): &(String,)| { dat.to_owned() }).collect::<Vec<_>>();
        println!("{:?}", data);
        for row in rows.iter().skip(2) {
            // println!("{}", row);
            let data = sqlx::query_as(format!("SELECT \"{}\" FROM '{}';", row, table).as_str())
                .fetch_all(&mut py_db).await.unwrap().iter().map(|(dat, ): &(Option<i64>,)| { dat.to_owned() }).collect::<Vec<_>>();
            // println!("{:?}", data);
        }
    }
}
