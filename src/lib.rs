use anyhow::{anyhow, Error};
use reqwest::header::CONTENT_TYPE;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::env;
use std::str::FromStr;

static SYSTEM_PROMPT: &str = r##"以下にYouTubeタイトルが与えられるので、YouTubeタイトルからsong_nameとsinger,version,editionをJSON形式で出力しなさい。
以下のルールを守って出力すること。
・NFKCにより、正規化して出力しなさい。
・各項目に関する文字列がなかった場合、空白にしなさい。
・楽曲名の読み仮名、ふりがなは、song_nameに含めない。
・editionや動画の情報に関する文字列があった場合それをeditionに含めなさい。
・"Promotion Edit","MV","Music Video"などは、version・editionに含めない。
・楽曲名・歌手の英訳は含めない。
・version(例:Ver.やversionやver等)に関する文字列があった場合、それをversionに含めなさい。

例: 入力 -> 出力
ロックンロール県庁所在地’９５	-> { song_name: "ロックンロール県庁所在地'95", singer: [], edition: "", version: "" }
宮本佳林『SUPER IDOL -Especial-(Single Ver.)』Music Video	-> { song_name: "SUPER IDOL -Especial-", singer: ["宮本佳林"], edition: "", version: "Single Ver." }
モーニング娘。'25『気になるその気の歌』Promotion Edit	-> { song_name: "気になるその気の歌", singer: ["モーニング娘。'25"], edition: "", version: "" }
女子会 The Night	-> { song_name: "女子会 The Night", singer: [], edition: "", version: "" }
森高千里 『私の大事な人』 【セルフカヴァー】	-> { song_name: "私の大事な人", singer: ["森高千里"], edition: "セルフカヴァー", version: "" }
松原健之「雪明かりの駅」Music Video（full ver.）	-> { song_name: "雪明かりの駅", singer: ["松原健之"], edition: "", version: "full ver." }
希望の夜	-> { song_name: "希望の夜", singer: [], edition: "", version: "" }
真野恵里菜 『My Days for You』 (Riverside Ver.)	-> { song_name: "My Days for You", singer: ["真野恵里菜"], edition: "", version: "Riverside Ver." }
Juice=Juice『初恋の亡霊』Promotion Edit	-> { song_name: "初恋の亡霊", singer: ["Juice=Juice"], edition: "", version: "" }
アンジュルム『アンドロイドは夢を見るか？』Promotion Edit	-> { song_name: "アンドロイドは夢を見るか?", singer: ["アンジュルム"], edition: "", version: "" }
OCHA NORMA『女の愛想は武器じゃない』Promotion Edit	-> { song_name: "女の愛想は武器じゃない", singer: ["OCHA NORMA"], edition: "", version: "" }
Super Red	-> { song_name: "Super Red", singer: [], edition: "", version: "" }
まひるの星 (2012 Remaster)	-> { song_name: "まひるの星", singer: [], edition: "2012 Remaster", version: "" }
本気ボンバー！！ (Instrumental)	-> { song_name: "本気ボンバー!!", singer: [], edition: "Instrumental", version: "" }
ME:I (ミーアイ) : LEAP HIGH! 〜明日へ、めいっぱい〜 Dance Practice 11 ver.	-> { song_name: "LEAP HIGH! 〜明日へ、めいっぱい〜", singer: ["ME:I"], edition: "", version: "Dance Practice 11 ver." }
【鈴木愛理】原神スカーク イメージソング「Star Odyssey」MV	-> { song_name: "Star Odyssey", singer: ["鈴木愛理"], edition: "", version: "" }
ME:I (ミーアイ) ⊹ 'MUSE' Official MV	-> { song_name: "MUSE", singer: ["ME:I"], edition: "", version: "" }
Hello！のテーマ (Berryz工房 Version)	-> { song_name: "Hello!のテーマ", singer: [], edition: "", version: "Berryz工房 Version" }
COVERS -One on One- サマーナイトタウン / 田中れいな x 佐藤優樹 -> { song_name: "サマーナイトタウン", singer: ["田中れいな","佐藤優樹"], edition: "COVERS -One on One-", version: "" }
モーニング娘。'14 『見返り美人』(Morning Musume。'14[A looking back beauty]) (Promotion Ver.) -> { song_name: "見返り美人", singer: ["モーニング娘。'14"], edition: "", version: "" }
LoVendoЯ「だけどもう一度 それでももう一度」（LIVE @ TSUTAYA O-WEST 2014.4.15） -> { song_name: "だけどもう一度 それでももう一度", singer: ["LoVendoЯ"], edition: "", version: "" }
金澤朋子『黄色い線の内側で並んでお待ちください』(Tomoko Kanazawa [Please wait in line behind the yellow line.])(MV) -> { song_name: "黄色い線の内側で並んでお待ちください", singer: ["金澤朋子"], edition: "", version: "" }
モーニング娘。'17『弩級のゴーサイン』(Morning Musume。'17[Green Light of the Dreadnaught])(Promotion Edit) -> { song_name: "弩級のゴーサイン", singer: ["モーニング娘。'17"], edition: "", version: "" }
アンジュルム『君だけじゃないさ...friends(2018アコースティックVer.)』(ANGERME[It's not just you…friends])(Promotion Edit) -> { song_name: "君だけじゃないさ...friends", singer: ["アンジュルム"], edition: "", version: "2018アコースティックVer." }
"##;

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
            "temperature": 0.5,
            "thinkingConfig": {
                "thinkingBudget": -1,
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

    let response_text = res.text().await?;
    // eprintln!("{}", &response_text);
    // レスポンスをJSONとしてパース
    let response_text = Value::from_str(&response_text)?;
    // println!("Response: {}", response_text);

    // 取得したJSON文字列を構造体に変換
    let text = response_text["candidates"][0]["content"]["parts"][0]["text"]
        .as_str().ok_or(anyhow!(""))?;
    let song_info: StructedSongTitle = match serde_json::from_str(text) {
        Ok(song_info) => song_info,
        _ => {
            return Err(anyhow!("Failed to parse response as StructedSongTitle: {}", text));
        }
    };
    // println!("extract song title by {} :{:?}", model_id, song_info);
    Ok(song_info)
}
