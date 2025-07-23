static SYSTEM_PROMPT: &str = r##"以下にYouTubeタイトルが与えられるので、YouTubeタイトルからsong_nameとsinger,version,editionをJSON形式で出力しなさい。
以下のルールを守って出力すること。
・各項目に関する文字列がなかった場合、空白にしなさい。
・楽曲名の読み仮名、ふりがなは、song_nameに含めない。
・editionや動画の情報に関する文字列があった場合それをeditionに含めなさい。
・"Promotion Edit","MV","Music Video"などは、version・editionに含めない。
・楽曲名・歌手の英訳は含めない。
・version(例:Ver.やversionやver等)に関する文字列があった場合、それをversionに含めなさい。"##;
use reqwest::Error;
use serde_json::{json, Value};
use std::env;
use reqwest::header::CONTENT_TYPE;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let gemini_api_key =
        env::var("GOOGLE_API_KEY").expect("GOOGLE_API_KEY環境変数が設定されていません");
    let model_id = "gemini-2.5-flash";
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
        model_id, gemini_api_key
    );

    // リクエストボディの構築
    let request_body = json!({
        "contents": [
            {
                "role": "system",
                "parts": [
                    {
                        "text": SYSTEM_PROMPT
                    },
                ]
            },
            {
                "role": "user",
                "parts": [
                    {
                        "text": "こぶしファクトリー『きっと私は』(Magnolia Factory[I must be…])(Promotion Edit)"
                    },
                ]
            }
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
    let res = client
        .post(&url)
        .header(CONTENT_TYPE, "application/json")
        .json(&request_body)
        .send()
        .await?;

    // レスポンスをJSONとしてパース
    let response_text = res.text().await?;
    println!("Response: {}", response_text);

    Ok(())
}
