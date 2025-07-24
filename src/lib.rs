use anyhow::{anyhow, Error};
use reqwest::header::CONTENT_TYPE;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::env;

static SYSTEM_PROMPT: &str = r##"以下にYouTubeタイトルが与えられるので、YouTubeタイトルからsong_nameとsinger,version,editionをJSON形式で出力しなさい。
以下のルールを守って出力すること。
・NFKCにより、正規化して出力しなさい。
・各項目に関する文字列がなかった場合、空白にしなさい。
・楽曲名の読み仮名、ふりがなは、song_nameに含めない。
・editionや動画の情報に関する文字列があった場合それをeditionに含めなさい。
・"Promotion Edit","MV","Music Video"などは、version・editionに含めない。
・楽曲名・歌手の英訳は含めない。
・version(例:Ver.やversionやver等)に関する文字列があった場合、それをversionに含めなさい。"##;

#[derive(Debug, Deserialize, Serialize)]
pub struct StructedSongTitle {
    pub song_name: String,
    pub singer: Vec<String>,
    pub edition: String,
    pub version: String,
}


pub async fn struct_title(title: String) -> Result<StructedSongTitle, Error> {
    println!("Use gemini to extract song title from: {}", title);
    if title == "" { return Err(anyhow!("Input string is empty!")); }
    let gemini_api_key =
        env::var("GOOGLE_API_KEY").expect("Please set environment variable GOOGLE_API_KEY");
    let model_id = "gemini-2.5-flash";
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
        model_id, gemini_api_key
    );

    // リクエストボディの構築
    let request_body = json!({
        "system_instruction": {
            "parts":{"text":SYSTEM_PROMPT}

        },
        "contents": [
            {
                "role": "user",
                "parts": [
                    {
                        "text":&title
                    },
                ]
            }
        ],
        "safetySettings":[
            {"category": "HARM_CATEGORY_HARASSMENT", "threshold": "BLOCK_NONE"},
            {"category": "HARM_CATEGORY_HATE_SPEECH", "threshold": "BLOCK_NONE"},
            {"category": "HARM_CATEGORY_SEXUALLY_EXPLICIT", "threshold": "BLOCK_NONE"},
            {"category": "HARM_CATEGORY_DANGEROUS_CONTENT", "threshold": "BLOCK_NONE"},
            {"category": "HARM_CATEGORY_CIVIC_INTEGRITY", "threshold": "BLOCK_NONE"}
        ],
        "generationConfig": {
            "temperature": 0,
            "thinkingConfig": {
                "thinkingBudget": 0,
            },
            "responseMimeType": "application/json",
            "responseSchema": {
                "type": "object",
                "properties": {
                    "song_name": {
                        "type": "string"
                    },
                    "singer": {
                        "type": "array",
                        "items": {
                            "type": "string"
                        }
                    },
                    "edition": {
                        "type": "string"
                    },
                    "version": {
                        "type": "string"
                    }
                },
                "required": [
                    "song_name",
                    "singer",
                    "edition",
                    "version"
                ],
                "propertyOrdering": [
                    "song_name",
                    "singer",
                    "edition",
                    "version"
                ]
            },
        },
    });

    // HTTPクライアントを初期化
    let client = reqwest::Client::new();

    // リクエストを送信
    let res = client.post(&url).header(CONTENT_TYPE, "application/json")
        .json(&request_body).send().await?;

    // レスポンスをJSONとしてパース
    let response_text = res.json::<Value>().await?;
    // println!("Response: {}", response_text);

    // 取得したJSON文字列を構造体に変換
    let text = response_text["candidates"][0]["content"]["parts"][0]["text"]
        .as_str().ok_or(anyhow!(""))?;
    let song_info: StructedSongTitle = serde_json::from_str(text)?;
    println!("extract song title by {} :{:?}", model_id, song_info);
    Ok(song_info)
}
