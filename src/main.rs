use anyhow::{anyhow, Error};
use duckdb::{params, Connection};
use futures::future::join_all;
use reqwest::Client;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::env;
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::{OnceCell, Semaphore};
use youtube_viewcount_logger_rust::{get_desired_date, struct_title, youtube_data_api_v3};

#[derive(Debug, Default, Clone)]
struct VideoData {
    video_id: String,
    title: Option<String>,
    views: Option<i64>,
    published_at: Option<String>,
}

impl Hash for VideoData {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.video_id.hash(state);
    }
}

impl PartialEq for VideoData {
    fn eq(&self, other: &Self) -> bool {
        self.video_id == other.video_id
    }
}
impl Eq for VideoData {}



async fn register_title(executor: &Connection, video_data: VideoData) -> Result<(), Error> {
    match executor.prepare("SELECT raw_title FROM __title__ WHERE youtube_id = ?")?
        .query_map(params![video_data.video_id], |row| {
            Ok(row.get::<_, Option<String>>(0).unwrap())
        })?.filter_map(|v| v.ok()).next() {
        // raw_title is NULL
        // or not exists
        None => {
            executor.execute("INSERT INTO __title__(youtube_id,raw_title,cleaned_title,structured_title) VALUES (?,?,NULL,NULL)",
                             params![video_data.video_id, video_data.title.clone()])?;
        }
        // raw_title is existing and not NULL
        Some(value) => {
            if match value {
                None => { true }
                Some(title) => { title != video_data.title.clone().unwrap_or("".to_owned()) }
            } {
                executor.execute("UPDATE __title__ SET raw_title = ?,cleaned_title = NULL, structured_title = NULL WHERE youtube_id = ?",
                                 params![video_data.title.clone(),video_data.video_id])?;
            }
        }
    }

    executor.execute("UPDATE __title__ SET published_at = ? WHERE youtube_id = ?", params![video_data.published_at,video_data.video_id])?;

    let structured_title = match executor.prepare("SELECT structured_title FROM __title__ WHERE youtube_id = ?")?
        .query_map(params![video_data.video_id], |row| {
            Ok(row.get::<_, Option<String>>(0).unwrap())
        })?.filter_map(|v| v.ok()).next().unwrap() {
        None => {
            let _ = GEMINI_SEMAPHORE.get().unwrap().acquire().await?;
            match video_data.title {
                None => {
                    None
                }
                Some(ref title) => {
                    let title = struct_title(title.clone()).await.ok();
                    // tokio sleep 10 sec.
                    tokio::time::sleep(Duration::from_secs(10)).await;
                    title
                }
            }
        }
        Some(val) => { serde_json::from_str::<_>(val.as_str()).ok() }
    };

    let structured_title = match structured_title {
        None => {
            return Err(anyhow!("structured title is not available for {:?}", video_data))
        }
        Some(title) => title,
    };
    if ENABLE_DEBUG.load(Ordering::Relaxed) { println!("structured title @ {}:{}", video_data.video_id.clone(), serde_json::to_string(&structured_title)?); }
    executor.execute("UPDATE __title__ SET structured_title = ? WHERE youtube_id = ?", params![serde_json::to_string(&structured_title)?,video_data.video_id.clone()])?;
    executor.execute("UPDATE __title__ SET cleaned_title = ? WHERE youtube_id = ?", params![{
                        // let v: Value = serde_json::from_str(structured_title.as_str())?;
                        let song_name = match structured_title.song_name.as_str(){
                            "" => { None }
                            x => { Some(x.to_owned()) }
                        };
                        let singer = structured_title.singer.join(",");
                        let edition = match structured_title.edition.as_str() {
                            "" => { None }
                            x => { Some(x.to_owned()) }
                        };
                        let version = match structured_title.version.as_str() {
                            "" => { None }
                            x => { Some(x.to_owned()) }
                        };
                        [song_name, match [Some(singer), edition, version]
                            .into_iter().filter_map(|v| { v }).collect::<Vec<_>>().join(" - ").as_str() {
                            "" => { None }
                            x => { Some(x.to_owned()) }
                        }].into_iter().filter_map(|v| { v }).collect::<Vec<_>>().join(" : ")
                    },video_data.video_id.clone()])?;
    Ok(())
}

static LIST_MAX_RESULTS: usize = 50;

static GEMINI_SEMAPHORE: OnceCell<Semaphore> = OnceCell::const_new();
static ENABLE_DEBUG: AtomicBool = AtomicBool::new(false);

#[tokio::main]
async fn main() {
    let today = get_desired_date().await;
    ENABLE_DEBUG.store(env::var("DEBUG").map(|v| v.trim().parse().ok().unwrap_or(false)).unwrap_or(false), Ordering::Release);
    GEMINI_SEMAPHORE.get_or_init(|| async {
        Semaphore::new(5)
    }).await;
    let mut duckdb = Connection::open("data.duckdb").unwrap();
    println!("{}", today);

    let mut lookup_table: HashMap<String, HashSet<VideoData>> = HashMap::new();

    duckdb.prepare("SHOW TABLES;").unwrap().query_map([], |row| { Ok(row.get::<_, String>(0).unwrap()) })
        .unwrap().filter_map(|v| v.ok()).filter_map(|row: String| {
        if row.starts_with("__") && row.ends_with("__") { None } else { Some(row) }
    }).for_each(|key| {
        lookup_table.insert(key, HashSet::new());
    });
    for (table_name, table_data) in lookup_table.iter_mut() {
        for video_id in duckdb.prepare("SELECT name FROM pragma_table_info(?) WHERE name <> 'index';").unwrap()
            .query_map([table_name], |row| { Ok(row.get::<_, String>(0).unwrap()) })
            .unwrap().filter_map(|v| v.ok()) {
            table_data.insert(VideoData { video_id, ..Default::default() });
        }
    }
    // println!("{:?}", lookup_table["鈴木愛理"]);
    // return;
    duckdb.prepare("SELECT db_key FROM __source__;").unwrap()
        .query_map([], |row| { Ok(row.get::<_, String>(0).unwrap()) })
        .unwrap().filter_map(|v| v.ok()).map(|row: String| {
        row
    }).for_each(|key| {
        if !lookup_table.contains_key(&key) {
            lookup_table.insert(key, HashSet::new());
        }
    });
    let playlist_items_arg = HashMap::from([
        ("part", "snippet"),
        ("fields", "items/snippet/resourceId/videoId,nextPageToken"),
        ("maxResults", format!("{LIST_MAX_RESULTS}").as_str())
    ].map(|(t1, t2)| { (t1.to_owned(), t2.to_owned()) }));

    let client = Client::new();

    for (db_key, playlist_key) in duckdb.prepare("SELECT db_key,playlist_key FROM __source__;").unwrap()
        .query_map([], |row| { Ok((row.get::<_, String>(0).unwrap(), row.get::<_, String>(1).unwrap())) })
        .unwrap().filter_map(|v| v.ok()) {
        // break;
        // if db_key != "鈴木愛理" {
        //     continue;
        // }
        let mut next_page_token: Option<String> = Some("".to_owned());
        while next_page_token.is_some() {
            let mut arg = playlist_items_arg.clone();
            arg.insert("playlistId".to_owned(), playlist_key.to_owned());
            arg.insert("pageToken".to_owned(), next_page_token.clone().unwrap());
            if ENABLE_DEBUG.load(Ordering::Relaxed) { println!("{:?}", arg); }
            match youtube_data_api_v3::<Value>("playlistItems".to_owned(), arg, client.clone()).await {
                None => {}
                Some(resp) => {
                    next_page_token = resp.get("nextPageToken").map(|v| v.as_str().unwrap().to_owned());
                    resp.get("items").unwrap_or(&Value::Array(vec![])).as_array().unwrap().into_iter().for_each(|item| {
                        lookup_table.get_mut(&db_key).unwrap().insert(VideoData { video_id: item["snippet"]["resourceId"]["videoId"].as_str().unwrap().to_owned(), ..Default::default() });
                    });
                }
            };
        }
    }
    // lookup_table = HashMap::from([("鈴木愛理".to_owned(), lookup_table["鈴木愛理"].clone())]);
    // println!("{:?}", lookup_table);
    let all_videos = lookup_table.iter().map(|(_, v)| { v.into_iter() }).flatten().collect::<HashSet<_>>();

    let mut all_videos_vec = all_videos.into_iter().cloned().collect::<Vec<VideoData>>();
    all_videos_vec.sort_by(|a, b| a.video_id.cmp(&b.video_id));

    // 全チャンクのレスポンスを video_id → VideoData のハッシュマップに集約する。
    // APIから返ってこなかった動画はマップに存在しない（= views: None 扱い）。
    let mut all_videos_map: HashMap<String, VideoData> = HashMap::new();
    for results in join_all(all_videos_vec.chunks(50).map(|chunk| {
        let chunk = chunk.to_vec();
        let client = client.clone();
        async move {
            let ids = chunk.iter().map(|v| v.video_id.as_str()).collect::<Vec<_>>().join(",");
            let Some(resp) = youtube_data_api_v3::<Value>(
                "videos".to_owned(),
                HashMap::from([
                    ("part", "snippet,statistics"),
                    ("fields", "items(id,snippet(title,publishedAt),statistics(viewCount))"),
                    ("id", ids.as_str()),
                ].map(|(k, v)| (k.to_owned(), v.to_owned()))),
                client,
            ).await else {
                return vec![];
            };
            if resp.get("error").is_some() {
                eprintln!("videos.list error: {}", resp["error"]);
                return vec![];
            }
            let item_map: HashMap<String, &Value> = resp
                .get("items").and_then(|v| v.as_array()).into_iter().flatten()
                .filter_map(|item| item.get("id")?.as_str().map(|id| (id.to_owned(), item)))
                .collect();
            chunk.iter().filter_map(|video| {
                let item = item_map.get(&video.video_id)?;
                Some(VideoData {
                    video_id: video.video_id.clone(),
                    title:        item["snippet"]["title"].as_str().map(|v| v.to_owned()),
                    published_at: item["snippet"]["publishedAt"].as_str().map(|v| v.to_owned()),
                    views:        item["statistics"]["viewCount"].as_str().and_then(|v| i64::from_str(v).ok()),
                })
            }).collect::<Vec<_>>()
        }
    })).await {
        for video_data in results {
            all_videos_map.insert(video_data.video_id.clone(), video_data);
        }
    }

    for res in join_all(all_videos_map.values().cloned().map(|v| register_title(&duckdb, v))).await {
        if let Err(err) = res {
            eprintln!("Error registering title: {}", err);
        }
    }

    // lookup_table の各グループに取得済みデータを分配する。
    // O(1) ルックアップで分配するため、二重ループを排除。
    for (_, group) in &mut lookup_table {
        let updated: HashSet<VideoData> = group
            .iter()
            .map(|v| match all_videos_map.get(&v.video_id) {
                Some(data) => data.clone(),
                None => v.clone(), // APIエラー or missing → views: None のまま
            })
            .collect();
        *group = updated;
    }
    let transaction = duckdb.transaction().unwrap();
    for (key, set) in lookup_table {
        match transaction.prepare("SELECT COUNT() FROM information_schema.tables WHERE table_name = ?;").unwrap()
            .query_map([&key], |row| { Ok(row.get::<_, i64>(0).unwrap()) })
            .unwrap().filter_map(|v| v.ok()).next().unwrap() {
            0 => {
                eprintln!("create table: {key}");
                transaction.execute(format!(r##"CREATE TABLE "{}" (index TIMESTAMPTZ PRIMARY KEY NOT NULL);"##, &key).as_str(), []).unwrap();
            }
            _ => {}
        }

        println!("{}", format!(r##"INSERT OR IGNORE INTO "{}"(index) VALUES(timezone('Asia/Tokyo',TIMESTAMP '{}'));"##, &key, today).as_str());
        transaction.execute(format!(r##"INSERT OR IGNORE INTO "{}"(index) VALUES(timezone('Asia/Tokyo',TIMESTAMP '{}'));"##, &key, today).as_str(), []).unwrap();

        let exist_columns = transaction.prepare("SELECT name FROM pragma_table_info(?);").unwrap()
            .query_map(params![&key], |row| { Ok(row.get::<_, String>(0).unwrap()) })
            .unwrap().filter_map(|v| v.ok()).collect::<HashSet<_>>();
        for datum in set {
            match exist_columns.contains(&datum.video_id) {
                true => {}
                false => {
                    transaction.execute(format!(r##"ALTER TABLE "{}" ADD COLUMN "{}" INT32;"##, &key, &datum.video_id).as_str(), []).unwrap();
                }
            }
            // println!("{:?}", datum);
            match datum.views {
                None => {}
                Some(views) => {
                    let query = format!(r##"UPDATE "{key}" SET "{}" = ? WHERE "index"=timezone('Asia/Tokyo',TIMESTAMP '{}');"##, &datum.video_id, today);
                    println!("{}", query.replace("?", &views.to_string()));
                    transaction.execute(query.as_str(), params![views]).unwrap();
                }
            }
        }
    }
    transaction.commit().unwrap();
}
