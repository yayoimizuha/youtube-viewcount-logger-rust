use std::collections::HashMap;
use sqlx::ConnectOptions;
use sqlx::sqlite::SqliteConnectOptions;

#[derive(Debug)]
struct VideoData {
    video_id: String,
    title: Option<String>,
    views: Option<i64>,
}
#[tokio::main]
async fn main() {
    let mut research_table: HashMap<String, HashMap<String, VideoData>> = HashMap::new();
    let mut db = SqliteConnectOptions::new().filename("data.sqlite").connect().await.unwrap();
    let _ = sqlx::query_as("SELECT tbl_name FROM sqlite_master WHERE type = 'table';")
        .fetch_all(&mut db).await.unwrap().into_iter().for_each(|(row, ): (String,)| {
        if row.starts_with("__") && row.ends_with("__") {
            return;
        }
        research_table.insert(row, HashMap::new());
    });
    println!("{:?}", research_table);
}
