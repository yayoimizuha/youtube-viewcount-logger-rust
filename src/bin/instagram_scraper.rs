use anyhow::anyhow;
use duckdb::{params, Connection};
use reqwest::Client;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use youtube_viewcount_logger_rust::get_desired_date;

const USER_AGENT: &str = "curl/8.5.0";
fn detect_json_end(string: String) -> Option<usize> {
    let mut depth = 0;
    for (i, c) in string.chars().enumerate() {
        match c {
            '{' => depth += 1,
            '}' => {
                if depth == 0 {
                    return Some(i);
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    None
}
#[tokio::main]
async fn main() {
    let today = get_desired_date().await;

    let duckdb = Connection::open("misc.duckdb").unwrap();
    duckdb.execute("CREATE TABLE IF NOT EXISTS instagram_accounts (id TEXT PRIMARY KEY);", params![]).unwrap();
    // instagram_stats: profile_picをprofile_pic_hashに変更
    duckdb.execute("CREATE TABLE IF NOT EXISTS instagram_stats (date TIMESTAMPTZ, username TEXT, followers_count INT, full_name TEXT, profile_pic_hash TEXT, posts_count INT, PRIMARY KEY (date, username));", params![]).unwrap();
    // instagram_profile_pics: 新しいテーブル
    duckdb.execute("CREATE TABLE IF NOT EXISTS instagram_profile_pics (hash TEXT PRIMARY KEY, image_blob BLOB);", params![]).unwrap();
    duckdb.execute("CREATE INDEX IF NOT EXISTS idx_instagram_stats_date ON instagram_stats (date, username);", params![]).unwrap();

    let mut usernames: HashSet<String> = duckdb.prepare("SELECT * FROM instagram_accounts;").unwrap().query_map(params![], |row| {
        Ok(row.get::<_, String>(0).unwrap())
    }).unwrap().filter_map(|v| { v.ok() }).collect();

    if fs::exists("instagram_user.list").unwrap() {
        // gather usernames from the file without started with #
        usernames.extend(fs::read_to_string("instagram_user.list")
            .unwrap()
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    None
                } else {
                    duckdb.execute("INSERT OR IGNORE INTO instagram_accounts (id) VALUES (?);", params![line]).unwrap();
                    Some(line.to_string())
                }
            }));
    }
    let client = Client::builder().user_agent(USER_AGENT).build().unwrap();
    let futures = usernames.clone().into_iter().map(async |username: String| -> anyhow::Result<()>{
        let document = client.get(format!("https://www.instagram.com/{username}/embed/")).send().await?.text().await?;
        let json_start_pos = document.find("contextJSON").ok_or(anyhow!("contextJSON is not found"))? + 14;
        let json_end_pos = detect_json_end(document[json_start_pos..].to_string()).ok_or(anyhow!("The end of JSON not found"))? + json_start_pos - 1;
        let json: Value = serde_json::from_str(unescaper::unescape(&document[json_start_pos..json_end_pos])?.as_str())?;
        // fs::write("debug.json", serde_json::to_string_pretty(&json)?)?;
        let followers_count = json["context"]["followers_count"].as_u64().ok_or(anyhow!("context.followers_count is not available"))?;
        let full_name = json["context"]["full_name"].as_str().ok_or(anyhow!("context.full_name is not available"))?;
        let profile_pic_url = json["context"]["profile_pic_url"].as_str().ok_or(anyhow!("context.profile_pic_url is not available"))?;
        let posts_count = json["context"]["posts_count"].as_u64().ok_or(anyhow!("context.posts_count is not available"))?;

        println!("{}",
                 format!("username:        {username}\n").to_owned() +
                     format!("followers_count: {followers_count}\n").as_str() +
                     format!("full_name:       {full_name}\n").as_str() +
                     format!("profile_pic_url: {profile_pic_url}\n").as_str() +
                     format!("posts_count:     {posts_count}\n\n").as_str());
        let profile_pic = client.get(profile_pic_url).send().await?.bytes().await?;
        let profile_pic_hash = Sha256::digest(profile_pic.as_ref()).iter().map(|v| { format!("{:02x}", v) }).collect::<String>();

        duckdb.execute("INSERT OR IGNORE INTO instagram_profile_pics (hash, image_blob) VALUES (?, ?);", params![&profile_pic_hash, profile_pic.as_ref()]).unwrap();

        duckdb.execute(format!("INSERT OR IGNORE INTO instagram_stats (date, username, followers_count, full_name, profile_pic_hash, posts_count) VALUES (timezone('Asia/Tokyo',TIMESTAMP '{}'), ?, ?, ?, ?, ?);", today).as_str(),
                       params![username,  followers_count as i64, full_name, &profile_pic_hash, posts_count as i64]).unwrap();
        Ok(())
    }).collect::<Vec<_>>();
    let results = futures::future::join_all(futures).await;
    for (result, username) in results.into_iter().zip(usernames) {
        if let Err(e) = result {
            eprintln!("Error: {} @ {}", e, username);
        }
    }
}
