use std::env;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use anyhow::{anyhow, Context};
use google_generative_ai_rs::v1::{api, gemini};
use google_generative_ai_rs::v1::gemini::{Content, Model, Part, Role};
use google_generative_ai_rs::v1::gemini::request::{GenerationConfig, SafetySettings};
use google_generative_ai_rs::v1::gemini::safety::{HarmBlockThreshold, HarmCategory};
use sqlx::{ConnectOptions, Database, Executor, SqliteConnection};
use sqlx::sqlite::SqliteConnectOptions;
use url::Url;
use once_cell::sync::Lazy;
use reqwest::Client;
use serde_json::Value;

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
        &self,
        mut executor: SqliteConnection,
        client: Client,
    ) -> Result<VideoData, anyhow::Error>
    {
        match youtube_data_api_v3::<Value>("videos".to_owned(), HashMap::from([
            ("part", "statistics,snippet"),
            ("fields", "items(snippet/title,statistics/viewCount)"),
            ("id", format!("{}", self.video_id).as_str())
        ].map(|(t1, t2)| { (t1.to_owned(), t2.to_owned()) })), client).await {
            None => { Err(anyhow!("not valid JSON.")) }
            Some(dat) => {
                println!("{}", dat);
                let title = dat["items"][0]["snippet"]["title"].as_str().context("title string not available.")?.to_owned();
                if match sqlx::query_as("SELECT raw_title FROM __title__ WHERE youtube_id = ?").bind(self.video_id.clone())
                    .fetch_one(&mut executor).await? {
                    None => { true }
                    Some((db_title, )) => { db_title != title }
                } {
                    sqlx::query("UPDATE __title__ SET raw_title = ?,cleaned_title = NULL, structured_title = NULL WHERE youtube_id = ?")
                        .bind(&title).bind(self.video_id.clone()).execute(&mut executor).await?;
                }
                let structured_title = match sqlx::query_as("SELECT structured_title FROM __title__ WHERE youtube_id = ?").bind(self.video_id.clone())
                    .fetch_one(&mut executor).await.map(|(t, ): (String,)| { t }).ok() {
                    None => {
                        let llm = api::Client::new_from_model(Model::Custom("gemini-2.0-flash-001".to_owned()), YTV3_API_KEY.clone());
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
                        let llm_resp = llm.post(30, &query).await?;
                        println!("{:#?}", llm_resp);
                        println!("{:#?}", query);
                        llm_resp.rest().unwrap().candidates.get(1).unwrap().clone().content.parts.get(0).unwrap().clone().text.unwrap()
                    }
                    Some(structured_title) => { structured_title }
                };
                println!("{}", structured_title);
                println!("aaaaaaaaaaa");


                let view_count = dat["items"][0]["statistics"]["viewCount"].as_str().unwrap().parse::<i64>().context("viewCount not available.")?.to_owned();
                Ok(VideoData { video_id: self.video_id.clone(), title: Some(title.clone()), views: Some(view_count.clone()) })
            }
        }
    }
}

static YTV3_API_KEY: Lazy<String> = Lazy::new(|| env::var("YTV3_API_KEY").unwrap());
static LIST_MAX_RESULTS: usize = 50;
async fn youtube_data_api_v3<T: for<'de> serde::de::Deserialize<'de>>(api_path: String, param: HashMap<String, String>, client: Client) -> Option<T> {
    let mut param = param;
    param.insert("key".to_owned(), YTV3_API_KEY.clone());
    let query_url = Url::parse_with_params(format!("https://www.googleapis.com/youtube/v3/{api_path}").as_str(), param.into_iter().collect::<Vec<_>>()).unwrap();
    client.get(query_url).send().await.unwrap().json::<T>().await.ok()
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

    // for (db_key, playlist_key) in sqlx::query_as("SELECT db_key,playlist_key FROM __source__;")
    //     .fetch_all(&mut db).await.unwrap().into_iter().map(|(db_key, playlist_key): (String, String)| { (db_key, playlist_key) }) {
    //     let mut next_page_token: Option<String> = Some("".to_owned());
    //     while next_page_token.is_some() {
    //         let mut arg = playlist_items_arg.clone();
    //         arg.insert("playlistId".to_owned(), playlist_key.to_owned());
    //         arg.insert("pageToken".to_owned(), next_page_token.clone().unwrap());
    //         println!("{:?}", arg);
    //         let resp = youtube_data_api_v3("playlistItems".to_owned(), arg, client.clone()).await;
    //         next_page_token = resp.get("nextPageToken").map(|v| v.as_str().unwrap().to_owned());
    //         resp.get("items").unwrap_or(&Value::Array(vec![])).as_array().unwrap().into_iter().for_each(|item| {
    //             research_table.get_mut(&db_key).unwrap().insert(VideoData { video_id: item["snippet"]["resourceId"]["videoId"].as_str().unwrap().to_owned(), ..Default::default() });
    //         });
    //     }
    //     break;
    // }
    // println!("{:?}", research_table);
    let vec = research_table.into_iter().map(|(_, v)| { v.into_iter() }).flatten().collect::<HashSet<_>>();
    vec.iter().next().unwrap().get_data(db, client).await.unwrap();
}
