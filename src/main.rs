use std::env;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::str::FromStr;
use std::time::Duration;
use anyhow::{anyhow, Context};
use chrono::FixedOffset;
use cron::Schedule;
use futures::future::join_all;
use google_generative_ai_rs::v1::{api, gemini};
use google_generative_ai_rs::v1::gemini::{Content, Model, Part, Role};
use google_generative_ai_rs::v1::gemini::request::{GenerationConfig, SafetySettings};
use google_generative_ai_rs::v1::gemini::safety::{HarmBlockThreshold, HarmCategory};
use once_cell::sync::Lazy;
use url::Url;
use reqwest::Client;
use serde_json::Value;
use sqlx::types::chrono::Utc;
use tokio::sync::OnceCell;
use chrono::format::SecondsFormat;
use duckdb::{params, Connection};

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
    ) -> Result<VideoData, anyhow::Error>
    {
        executor.execute("INSERT OR IGNORE INTO __title__(youtube_id) VALUES (?)", params![self.video_id.clone()])?;
        match youtube_data_api_v3::<Value>("videos".to_owned(), HashMap::from([
            ("part", "statistics,snippet"),
            ("fields", "items(snippet/title,statistics/viewCount)"),
            ("id", format!("{}", self.video_id).as_str())
        ].map(|(t1, t2)| { (t1.to_owned(), t2.to_owned()) })), client).await {
            None => { Err(anyhow!("not valid JSON.")) }
            Some(dat) => {
                println!("{}", dat);
                if dat["items"].as_array().unwrap().is_empty() {
                    return Err(anyhow!("movie info is not available"));
                }
                let title = dat["items"][0]["snippet"]["title"].as_str().context("title string not available.")?.to_owned();
                if match executor.prepare("SELECT raw_title FROM __title__ WHERE youtube_id = ?")?
                    .query_map(params![self.video_id.clone()], |row| { Ok(row.get::<_, Option<String>>(0).unwrap()) })?
                    .filter_map(|v| v.ok()).next() {
                    Some(Some(db_title)) => { db_title != title }
                    _ => { true }
                } {
                    executor.execute("UPDATE __title__ SET raw_title = ?,cleaned_title = NULL, structured_title = NULL WHERE youtube_id = ?",
                                     params![&title,self.video_id.clone()])?;
                }
                let structured_title = match executor.prepare("SELECT structured_title FROM __title__ WHERE youtube_id = ?")?
                    .query_map(params![self.video_id.clone()], |row| { Ok(row.get::<_, Option<String>>(0).unwrap()) })?
                    .filter_map(|v| v.ok()).next() {
                    Some(Some(structured_title)) => {
                        structured_title
                    }
                    _ => {
                        let mut gemini_cache: String = "".to_owned();
                        File::open("gemini-cache.json")?.read_to_string(&mut gemini_cache)?;
                        let gemini_cache_value: Value = serde_json::from_str(gemini_cache.as_str())?;
                        match gemini_cache_value.get(&title) {
                            None => {
                                let llm = api::Client::new_from_model(Model::Custom("gemini-2.0-flash-001".to_owned()), GOOGLE_API_KEY.clone());
                                let query = gemini::request::Request {
                                    contents: vec![
                                        Content {
                                            role: Role::User,
                                            parts: vec![Part {
                                                text: Some(format!(r##"以下にYouTubeタイトルが与えられるので、YouTubeタイトルから楽曲名と歌手、バージョン、エディションをJSON形式で{{"song_name":"XXXXX","singer":["AAAAA","BBBB"],"edition":"CCCCC","version":"DDDDD"}}というフォーマットで出力しなさい。Markdownのコードブロックは使わないこと。
楽曲名は、以下のルールに従って加工しなさい。
・それぞれの項目に関する文字列がなかった場合、空白にすること。
・楽曲名の読み仮名は、楽曲名から除きなさい。
・英訳があった場合は楽曲名に含めてはいけない。
・バージョン(例:Ver.やversionやver等)に関する文字列があった場合、それをバージョンに含めなさい。
・バージョンに関する文字列がなかった場合、バージョンは空文字とすること。
・エディションや動画に関する文字列があった場合それをエディションに含めなさい。
・Promotion EditやMVやMusic Videoなどの単語があった場合、エディションは空文字にしなさい。
・エディションや動画に関する文字列がなかった場合、エディションは空文字とすること。


{title}"##)),
                                                inline_data: None,
                                                file_data: None,
                                                video_metadata: None,
                                            }],
                                        }
                                    ],
                                    tools: vec![],
                                    safety_settings: vec![
                                        SafetySettings { category: HarmCategory::HarmCategoryHarassment, threshold: HarmBlockThreshold::BlockNone },
                                        SafetySettings { category: HarmCategory::HarmCategoryHateSpeech, threshold: HarmBlockThreshold::BlockNone },
                                        SafetySettings { category: HarmCategory::HarmCategorySexuallyExplicit, threshold: HarmBlockThreshold::BlockNone },
                                        SafetySettings { category: HarmCategory::HarmCategoryDangerousContent, threshold: HarmBlockThreshold::BlockNone },
                                    ],
                                    generation_config: Some(GenerationConfig {
                                        temperature: Some(0.0),
                                        top_p: Some(1.0),
                                        top_k: Some(1),
                                        candidate_count: None,
                                        max_output_tokens: Some(1024),
                                        stop_sequences: None,
                                        response_mime_type: Some("application/json".to_owned()),
                                        response_schema: None,
                                    }),
                                    system_instruction: None,
                                };
                                let llm_resp = llm.post(30, &query).await?.rest().unwrap();
                                println!("{:?}", llm_resp.candidates[0]);
                                // println!("{:#?}", query);
                                llm_resp.candidates.get(0).ok_or(anyhow!(format!("llm_resp format is wrong.\n{:#?}",llm_resp.clone())))?
                                    .clone().content.parts.get(0).unwrap().clone().text.unwrap()
                            }
                            Some(v) => { serde_json::to_string_pretty(v)? }
                        }
                    }
                };
                executor.execute("UPDATE __title__ SET structured_title = ? WHERE youtube_id = ?", params![&structured_title,self.video_id.clone()])?;
                executor.execute("UPDATE __title__ SET cleaned_title = ? WHERE youtube_id = ?", params![{
                        let v: Value = serde_json::from_str(structured_title.as_str())?;
                        let song_name = match v["song_name"].as_str().unwrap().to_owned().as_str() {
                            "" => { None }
                            x => { Some(x.to_owned()) }
                        };
                        let singer = v["singer"].as_array().unwrap().into_iter().map(|v| { v.as_str().unwrap().to_owned() }).collect::<Vec<_>>().join(",");
                        let edition = match v["edition"].as_str().unwrap().to_owned().as_str() {
                            "" => { None }
                            x => { Some(x.to_owned()) }
                        };
                        let version = match v["version"].as_str().unwrap().to_owned().as_str() {
                            "" => { None }
                            x => { Some(x.to_owned()) }
                        };
                        [song_name, match [Some(singer), edition, version]
                            .into_iter().filter_map(|v| { v }).collect::<Vec<_>>().join(" - ").as_str() {
                            "" => { None }
                            x => { Some(x.to_owned()) }
                        }]
                            .into_iter().filter_map(|v| { v }).collect::<Vec<_>>().join(" : ")
                    },self.video_id.clone()])?;

                let view_count = dat["items"][0]["statistics"]["viewCount"].as_str().unwrap().parse::<i64>().context("viewCount not available.")?.to_owned();
                Ok(VideoData { video_id: self.video_id.clone(), title: Some(title.clone()), views: Some(view_count.clone()) })
            }
        }
    }
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
        .into_iter().filter_map(|v| { v.ok() }).collect::<Vec<_>>();

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
