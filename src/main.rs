use std::env;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use sqlx::ConnectOptions;
use sqlx::sqlite::SqliteConnectOptions;
use url::Url;
use once_cell::sync::Lazy;
use reqwest::Client;
use serde_json::Value;

#[derive(Debug, Eq, PartialEq, Default)]
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

static YTV3_API_KEY: Lazy<String> = Lazy::new(|| env::var("YTV3_API_KEY").unwrap());
static LIST_MAX_RESULTS: usize = 50;
async fn youtube_data_api_v3(api_path: String, param: HashMap<String, String>, client: Client) -> Value {
    let mut param = param;
    param.insert("key".to_owned(), YTV3_API_KEY.clone());
    let query_url = Url::parse_with_params(format!("https://www.googleapis.com/youtube/v3/{api_path}").as_str(), param.into_iter().collect::<Vec<_>>()).unwrap();
    client.get(query_url).send().await.unwrap().json::<Value>().await.unwrap()
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
        for video_id in sqlx::query_as("SELECT name FROM pragma_table_info(?);").bind(table_name)
            .fetch_all(&mut db).await.unwrap().into_iter().skip(1).map(|(video_id, ): (String,)| { video_id }) {
            // let title = sqlx::query_as("SELECT title FROM __title__ WHERE youtube_id = ?").bind(&video_id)
            //     .fetch_one(&mut db).await.map(|(t, ): (String,)| { t }).ok();
            table_data.insert(VideoData { video_id, ..Default::default() });
        }
    }
    let playlist_items_arg = HashMap::from([
        ("part", "snippet"),
        ("fields", "items/snippet/resourceId/videoId,nextPageToken"),
        ("maxResults", format!("{LIST_MAX_RESULTS}").as_str())
    ].map(|(t1, t2)| { (t1.to_owned(), t2.to_owned()) }));

    let client = Client::new();

    for (db_key, playlist_key) in sqlx::query_as("SELECT db_key,playlist_key FROM __source__;")
        .fetch_all(&mut db).await.unwrap().into_iter().map(|(db_key, playlist_key): (String, String)| { (db_key, playlist_key) }) {
        let mut next_page_token: Option<String> = Some("".to_owned());
        while next_page_token.is_some() {
            let mut arg = playlist_items_arg.clone();
            arg.insert("playlistId".to_owned(), playlist_key.to_owned());
            arg.insert("pageToken".to_owned(), next_page_token.clone().unwrap());
            println!("{:?}", arg);
            let resp = youtube_data_api_v3("playlistItems".to_owned(), arg, client.clone()).await;
            next_page_token = resp.get("nextPageToken").map(|v| v.as_str().unwrap().to_owned());
            resp.get("items").unwrap_or(&Value::Array(vec![])).as_array().unwrap().into_iter().for_each(|item| {
                research_table.get_mut(&db_key).unwrap().insert(VideoData { video_id: item["snippet"]["resourceId"]["videoId"].as_str().unwrap().to_owned(), ..Default::default() });
            });
        }
        break;
    }
    // println!("{:?}", research_table);
}
