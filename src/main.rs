use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use sqlx::ConnectOptions;
use sqlx::sqlite::SqliteConnectOptions;
use url::Url;

#[derive(Debug)]
#[derive(Eq, PartialEq)]
struct VideoData {
    video_id: String,
    title: Option<String>,
    views: Option<i64>,
}

impl Hash for VideoData {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.video_id.hash(state);
    }
}

async fn list_playlist_items(key: String) {
    let query_url = Url::parse_with_params("https://www.googleapis.com/youtube/v3/playlistItems", &[("", "")]).unwrap();
}
#[tokio::main]
async fn main() {
    let mut research_table: HashMap<String, HashSet<VideoData>> = HashMap::new();
    let mut db = SqliteConnectOptions::new().filename("data.sqlite").connect().await.unwrap();
    sqlx::query_as("SELECT tbl_name FROM sqlite_master WHERE type = 'table';")
        .fetch_all(&mut db).await.unwrap().into_iter().filter_map(|(row, ): (String,)| {
        if row.starts_with("__") && row.ends_with("__") { None } else { Some(row) }
    }).chain(sqlx::query_as("SELECT db_key FROM __source__;")
        .fetch_all(&mut db).await.unwrap().into_iter().map(|(row, ): (String,)| { row })).for_each(|key| {
        research_table.insert(key, HashSet::new());
    });
    for (table_name, table_data) in research_table.iter_mut() {
        for video_id in sqlx::query_as("SELECT name FROM pragma_table_info(?);").bind(table_name).fetch_all(&mut db).await.unwrap().into_iter().skip(1).map(|(video_id, ): (String,)| { video_id }) {
            let title = sqlx::query_as("SELECT title FROM __title__ WHERE youtube_id = ?").bind(&video_id).fetch_one(&mut db).await.map(|(t, )| { t }).ok();
            table_data.insert(VideoData { video_id, title, views: None });
        }
    }
    println!("{:?}", research_table);
}
