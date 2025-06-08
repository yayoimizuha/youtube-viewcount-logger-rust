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
    // let mut rust_db = SqliteConnectOptions::new().create_if_missing(true).filename("data.sqlite").connect().await.unwrap();

    let mut rust_db = Connection::open("data.duckdb").unwrap();

    rust_db.execute_batch("CREATE TABLE __source__ (playlist_key TEXT PRIMARY KEY NOT NULL,db_key TEXT NOT NULL,hashtag TEXT NOT NULL,screen_name TEXT NOT NULL,is_tweet INTEGER NOT NULL);").unwrap();
    rust_db.execute_batch("CREATE TABLE __title__ (youtube_id TEXT PRIMARY KEY NOT NULL,raw_title TEXT,cleaned_title TEXT,structured_title TEXT);").unwrap();

    // sqlx::query("CREATE TABLE __source__ (playlist_key TEXT PRIMARY KEY NOT NULL,db_key TEXT NOT NULL,hashtag TEXT NOT NULL,screen_name TEXT NOT NULL,is_tweet INTEGER NOT NULL);").execute(&mut rust_db).await.unwrap();
    // sqlx::query("CREATE TABLE __title__ (youtube_id TEXT PRIMARY KEY NOT NULL,raw_title TEXT,cleaned_title TEXT,structured_title TEXT);").execute(&mut rust_db).await.unwrap();
    // return;

    let table_names = sqlx::query_as("SELECT tbl_name FROM sqlite_master WHERE type = 'table';")
        .fetch_all(&mut py_db).await.unwrap().iter().map(|(tbl_name, ): &(String,)| tbl_name.to_owned()).collect::<Vec<_>>();
    for table in table_names {
        rust_db.execute(format!(r##"CREATE TABLE "{}" (index DATE PRIMARY KEY NOT NULL);"##, table).as_str(), params![]).unwrap();
        // sqlx::query(format!("CREATE TABLE '{}' ('index' DATE PRIMARY KEY NOT NULL);", table).as_str()).execute(&mut rust_db).await.unwrap();

        // let rows = sqlx::query_as("SELECT name FROM pragma_table_info(?);").bind(&table)
        //     .fetch_all(&mut py_db).await.unwrap().iter().map(|(index_name, ): &(String,)| { index_name.to_owned() }).collect::<Vec<_>>();
        // println!("{}", table);
        for (index, title) in sqlx::query_as(format!("SELECT \"index\",\"タイトル\" FROM '{}';", table).as_str())
            .fetch_all(&mut py_db).await.unwrap().iter().map(|(index, title, ): &(String, String)| { (index.to_owned(), title.to_owned()) }) {
            println!("{}:{}", index, title);
            rust_db.execute("INSERT OR REPLACE INTO __title__(youtube_id,cleaned_title) VALUES (?,?);",
                            params![index.strip_prefix("https://youtu.be/").unwrap(),title]).unwrap();
            // sqlx::query("INSERT OR REPLACE INTO __title__(youtube_id,cleaned_title) VALUES (?,?);")
            //     .bind(index.strip_prefix("https://youtu.be/").unwrap()).bind(title).execute(&mut rust_db).await.unwrap();
            rust_db.execute(format!(r##"ALTER TABLE "{}" ADD COLUMN "{}" INTEGER;"##, table, index.strip_prefix("https://youtu.be/").unwrap()).as_str(),
                            params![]).unwrap();
            // sqlx::query(format!("ALTER TABLE '{}' ADD COLUMN '{}' INTEGER;", table, index.strip_prefix("https://youtu.be/").unwrap()).as_str()).execute(&mut rust_db).await.unwrap();
        }
        // let mut transaction = rust_db.begin().await.unwrap();
        // let transaction = rust_db.transaction().unwrap();
        for date in sqlx::query_as("SELECT name FROM pragma_table_info(?);").bind(&table)
            .fetch_all(&mut py_db).await.unwrap().iter().map(|(index_name, ): &(String,)| { index_name.to_owned() }).skip(2) {
            rust_db.execute(format!(r##"INSERT INTO "{}"("index") VALUES(timezone('Asia/Tokyo',TIMESTAMP '{}'))"##,
                                        table,
                                        if date.contains(":") {
                                            date.replace("00:00:00", "08:00:00")
                                        } else {
                                            format!("{} 08:00:00", date)
                                        }).as_str(), params![
                               
            ]).unwrap();
            // sqlx::query(format!("INSERT INTO '{}'('index') VALUES(datetime(?))", table).as_str()).bind({
            //     if date.contains(":") {
            //         date.replace("00:00:00", "08:00:00")
            //     } else {
            //         format!("{} 08:00:00", date)
            //     }
            // }).execute(&mut *transaction).await.unwrap();

            let mut queries = Vec::new();
            for (index, count) in sqlx::query_as(format!(r##"SELECT "index","{date}" FROM "{table}" ;"##).as_str())
                .fetch_all(&mut py_db).await.unwrap().iter().map(|(index, count): &(String, Option<i64>)| { (index.to_owned(), count.to_owned()) }) {
                queries.push(format!(r##"UPDATE "{table}" SET "{}" = {} WHERE "index"="{}";"##,
                                     index.strip_prefix("https://youtu.be/").unwrap(),
                                     match count {
                                         None => { "NULL".to_owned() }
                                         Some(v) => { v.to_string() }
                                     },
                                     if date.contains(":") {
                                         date.replace("00:00:00", "08:00:00")
                                     } else {
                                         format!("{} 08:00:00", date)
                                     }));
                // transaction.execute(format!(r##"UPDATE "{table}" SET "{}" = ? WHERE "index"=?;"##, index.strip_prefix("https://youtu.be/").unwrap()).as_str(),
                //                     params![count,
                //                         if date.contains(":") {
                //                             date.replace("00:00:00", "08:00:00")
                //                         } else {
                //                             format!("{} 08:00:00", date)
                //                         }]).unwrap();
                // sqlx::query(format!(r##"UPDATE "{table}" SET "{}" = ? WHERE "index"=?;"##, index.strip_prefix("https://youtu.be/").unwrap()).as_str()).bind(count).bind({
                //     if date.contains(":") {
                //         date.replace("00:00:00", "08:00:00")
                //     } else {
                //         format!("{} 08:00:00", date)
                //     }
                // }).execute(&mut *transaction).await.unwrap();
            }
            rust_db.execute_batch(queries.join("\n").as_str()).unwrap()
        }
        // rust_db.commit().unwrap();

        // for row in rows.iter().skip(2) {
        //     // println!("{}", row);
        //     let data = sqlx::query_as(format!("SELECT \"{}\" FROM '{}';", row, table).as_str())
        //         .fetch_all(&mut py_db).await.unwrap().iter().map(|(dat, ): &(Option<i64>,)| { dat.to_owned() }).collect::<Vec<_>>();
        //     // println!("{:?}", data);
        // }
    }
}

