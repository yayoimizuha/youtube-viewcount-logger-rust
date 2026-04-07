use std::collections::HashMap;
use url::Url;

#[tokio::main]
async fn main() {
    let api_key = match std::env::var("GOOGLE_API_KEY") {
        Ok(v) if !v.trim().is_empty() => v,
        _ => {
            eprintln!(
            "GOOGLE_API_KEY is not set.\n\nSet it in your current shell (do NOT paste the key into chat):\n  $env:GOOGLE_API_KEY = '...';\n\nThen re-run:\n  cargo run --bin youtube_v3_api_test -- <VIDEO_ID> [<VIDEO_ID> ...]"
        );
            std::process::exit(2);
        }
    };

    let ids: Vec<String> = std::env::args().skip(1).collect();
    if ids.is_empty() {
        eprintln!(
            "Usage: cargo run --bin youtube_v3_api_test -- <VIDEO_ID> [<VIDEO_ID> ...]\n\nExample:\n  cargo run --bin youtube_v3_api_test -- nmXHicn726k aaaaaaaaaaa"
        );
        std::process::exit(2);
    }

    let malformed_ids: Vec<&str> = ids
        .iter()
        .map(|s| s.as_str())
        .filter(|id| id.len() != 11)
        .collect();
    if !malformed_ids.is_empty() {
        eprintln!(
            "WARNING: malformed video id(s) (len != 11): {}",
            malformed_ids.join(",")
        );
    }

    eprintln!("Requested IDs ({}): {}", ids.len(), ids.join(","));

    let id_param = ids.join(",");
    let client = reqwest::Client::new();
    let query_url = Url::parse_with_params(
        "https://www.googleapis.com/youtube/v3/videos",
        HashMap::from(
            [
                ("part", "snippet,statistics"),
                (
                    "fields",
                    "items(id,snippet(title,publishedAt),statistics(viewCount))",
                ),
                ("id", id_param.as_str()),
                ("key", api_key.as_str()),
            ]
            .map(|(t1 /**/, t2)| (t1.to_owned(), t2.to_owned())),
        )
        .into_iter()
        .collect::<Vec<_>>(),
    )
    .unwrap();

    let res = client.get(query_url).send().await.unwrap();
    let status = res.status();
    let text = res.text().await.unwrap();

    let resp: serde_json::Value = serde_json::from_str(&text).unwrap_or_else(|_| {
        serde_json::json!({
            "_parse_error": true,
            "_raw": text,
        })
    });

    eprintln!("HTTP status: {}", status.as_u16());

    if let Some(error) = resp.get("error") {
        let code = error.get("code").and_then(|v| v.as_i64());
        let message = error.get("message").and_then(|v| v.as_str());
        let reason = error
            .get("errors")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|first| first.get("reason"))
            .and_then(|v| v.as_str());
        eprintln!("API returned error payload: code={code:?} message={message:?}");
        if let Some(reason) = reason {
            eprintln!("Error reason: {reason}");
        }
    } else {
        let item_ids: Vec<&str> = resp
            .get("items")
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
            .filter_map(|item| item.get("id").and_then(|v| v.as_str()))
            .collect();
        eprintln!("OK: items={}", item_ids.len());
        for id in &item_ids {
            eprintln!("  - {id}");
        }

        let missing: Vec<&str> = ids
            .iter()
            .map(|s| s.as_str())
            .filter(|id| !item_ids.iter().any(|got| got == id))
            .collect();
        if !missing.is_empty() {
            eprintln!("Missing IDs ({}): {}", missing.len(), missing.join(","));
        }
    }

    println!("{}", serde_json::to_string_pretty(&resp).unwrap());
}
