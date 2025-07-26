use std::collections::HashMap;
use youtube_viewcount_logger_rust::youtube_data_api_v3;

#[tokio::main]
async fn main() {
    let client = reqwest::Client::new();
    let resp = youtube_data_api_v3::<serde_json::Value>("videos".to_owned(), HashMap::from([
        ("part", "statistics,snippet"),
        ("fields", "items(snippet(title,publishedAt),statistics/viewCount)"),
        ("id", format!("{}", "nmXHicn726k").as_str())
    ].map(|(t1/**/, t2)| { (t1.to_owned(), t2.to_owned()) })), client).await.unwrap();

    println!("{}", resp);
}