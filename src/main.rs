use anyhow::{anyhow, Error};
use chrono::format::SecondsFormat;
use chrono::FixedOffset;
use cron::Schedule;
use duckdb::{params, Connection};
use futures::future::join_all;
use once_cell::sync::Lazy;
use reqwest::Client;
use serde_json::Value;
use sqlx::types::chrono::Utc;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::str::FromStr;
use std::time::Duration;
use tokio::sync::OnceCell;
use url::Url;
use youtube_viewcount_logger_rust::struct_title;

#[derive(Debug, Default, Clone)]
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

impl PartialEq for VideoData {
    fn eq(&self, other: &Self) -> bool {
        self.video_id == other.video_id
    }
}
impl Eq for VideoData {}


impl VideoData {
    async fn get_data(
        self,
        executor: &Connection,
        client: Client,
    ) -> Result<VideoData, Error>
    {
        executor.execute("INSERT OR IGNORE INTO __title__(youtube_id) VALUES (?)", params![self.video_id.clone()])?;
        match youtube_data_api_v3::<Value>("videos".to_owned(), HashMap::from([
            ("part", "statistics,snippet"),
            ("fields", "items(snippet/title,statistics/viewCount)"),
            ("id", format!("{}", self.video_id).as_str())
        ].map(|(t1/**/, t2)| { (t1.to_owned(), t2.to_owned()) })), client).await {
            None => { Err(anyhow!("not valid JSON.")) }
            Some(dat) => {
                println!("{}", dat);
                if dat["items"].as_array().ok_or(anyhow!("Parse error."))?.is_empty() {
                    return Err(anyhow!("movie info is not available"));
                }

                let video_data = VideoData {
                    video_id: self.video_id,
                    title: dat["items"][0]["snippet"]["title"].as_str().map(|v| v.to_owned()),
                    views: dat["items"][0]["statistics"]["viewCount"].as_str().map(|v| i64::from_str(v).unwrap()),
                };

                register_title(&executor, video_data.clone()).await?;
                Ok(video_data)
            }
        }
    }
}

async fn register_title(executor: &Connection, video_data: VideoData) -> Result<(), Error> {
    match executor.prepare("SELECT raw_title FROM __title__ WHERE youtube_id = ?")?
        .query_map(params![video_data.video_id], |row| {
            Ok(row.get::<_, Option<String>>(0).unwrap())
        })?.filter_map(|v| v.ok()).next() {
        None => {
            executor.execute("INSERT INTO __title__(youtube_id,raw_title,cleaned_title,structured_title) VALUES (?,?,NULL,NULL)",
                             params![video_data.video_id, video_data.title.clone().unwrap_or("".to_owned())])?;
        }
        Some(value) => {
            if match value {
                None => { true }
                Some(title) => { title != video_data.title.clone().unwrap_or("".to_owned()) }
            } {
                executor.execute("UPDATE __title__ SET raw_title = ?,cleaned_title = NULL, structured_title = NULL WHERE youtube_id = ?",
                                 params![video_data.title.clone().unwrap_or("".to_owned()),video_data.video_id])?;
            }
        }
    }


    let structured_title = match executor.prepare("SELECT structured_title FROM __title__ WHERE youtube_id = ?")?
        .query_map(params![video_data.video_id], |row| {
            Ok(row.get::<_, Option<String>>(0).unwrap())
        })?.filter_map(|v| v.ok()).next().unwrap() {
        None => { struct_title(video_data.title.clone().unwrap_or("".to_owned())).await.ok() }
        Some(val) => { serde_json::from_str::<_>(val.as_str()).ok() }
    };

    let structured_title = match structured_title {
        None => {
            return Err(anyhow!("structured title is not available for {:?}", video_data))
        }
        Some(title) => title,
    };
    println!("structured title @ {}:{}", video_data.video_id.clone(), serde_json::to_string(&structured_title)?);
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

static GOOGLE_API_KEY: Lazy<String> = Lazy::new(|| env::var("GOOGLE_API_KEY").unwrap());
static TODAY: OnceCell<String> = OnceCell::const_new();
static LIST_MAX_RESULTS: usize = 50;
async fn youtube_data_api_v3<T: for<'de> serde::de::Deserialize<'de>>(api_path: String, param: HashMap<String, String>, client: Client) -> Option<T> {
    let mut param = param;
    param.insert("key".to_owned(), GOOGLE_API_KEY.clone());
    let query_url = Url::parse_with_params(format!("https://www.googleapis.com/youtube/v3/{api_path}").as_str(), param.into_iter().collect::<Vec<_>>()).unwrap();
    client.get(query_url).send().await.unwrap().json::<T>().await.ok()
}
#[tokio::main]
async fn main() {
    TODAY.get_or_init(|| async {
        let mut github_event_path = String::new();
        File::open(env::var("GITHUB_EVENT_PATH").unwrap()).unwrap().read_to_string(&mut github_event_path).unwrap();
        let a = match serde_json::from_str::<Value>(github_event_path.as_str()).unwrap().get("schedule") {
            None => { None }
            Some(schedule) => {
                let cron_str = format!("0 {}", schedule.as_str().unwrap().trim());
                let sched = Schedule::from_str(cron_str.as_str()).unwrap();
                let mut duration = 100f32;
                while sched.after(&(Utc::now() - Duration::from_secs_f32(duration))).take_while(|&date| date < Utc::now()).count() == 0 {
                    duration *= 1.2;
                }
                let date = sched.after(&(Utc::now() - Duration::from_secs_f32(duration))).next().unwrap();
                Some(date)
            }
        };
        let a = a.unwrap_or(Utc::now());
        a.with_timezone(&FixedOffset::east_opt(3600 * 9).unwrap()).to_rfc3339_opts(SecondsFormat::AutoSi, true).replace("T", " ").chars().take(19).collect::<String>()
    }).await;
    let mut duckdb = Connection::open("data.duckdb").unwrap();
    println!("{}", TODAY.get().unwrap());

    let mut lookup_table: HashMap<String, HashSet<VideoData>> = HashMap::new();

    duckdb.prepare("SHOW TABLES;").unwrap().query_map([], |row| { Ok(row.get::<_, String>(0).unwrap()) })
        .unwrap().filter_map(|v| v.ok()).filter_map(|row: String| {
        if row.starts_with("__") && row.ends_with("__") { None } else { Some(row) }
    }).for_each(|key| {
        lookup_table.insert(key, HashSet::new());
    });
    for (table_name, table_data) in lookup_table.iter_mut() {
        for video_id in duckdb.prepare("SELECT name FROM pragma_table_info(?);").unwrap()
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
            println!("{:?}", arg);
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

    let all_videos_data = join_all(all_videos.into_iter().map(|video| { video.clone().get_data(&duckdb, client.clone()) })).await
        .into_iter().filter_map(|v1| {
        match &v1 {
            Ok(_) => {}
            Err(err) => {
                eprintln!("Error fetching video data: {}", err);
            }
        }
        v1.ok()
    }).collect::<Vec<_>>();

    for video_data in all_videos_data {
        for (_, group) in &mut lookup_table {
            if group.contains(&video_data) {
                group.remove(&video_data);
                group.insert(video_data.clone());
            }
        }
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

        println!("{}", format!(r##"INSERT OR IGNORE INTO "{}"(index) VALUES(timezone('Asia/Tokyo',TIMESTAMP '{}'));"##, &key, TODAY.get().unwrap()).as_str());
        transaction.execute(format!(r##"INSERT OR IGNORE INTO "{}"(index) VALUES(timezone('Asia/Tokyo',TIMESTAMP '{}'));"##, &key, TODAY.get().unwrap()).as_str(), []).unwrap();

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
                    let query = format!(r##"UPDATE "{key}" SET "{}" = ? WHERE "index"=timezone('Asia/Tokyo',TIMESTAMP '{}');"##, &datum.video_id, TODAY.get().unwrap());
                    println!("{}", query.replace("?", &views.to_string()));
                    transaction.execute(query.as_str(), params![views]).unwrap();
                }
            }
        }
    }
    transaction.commit().unwrap();
}
