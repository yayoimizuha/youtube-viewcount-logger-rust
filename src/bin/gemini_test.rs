use futures::future::join_all;
use youtube_viewcount_logger_rust::struct_title;

#[tokio::main]
async fn main() {
    let titles = vec!["ロックンロール県庁所在地’９５",
                      "宮本佳林『SUPER IDOL -Especial-(Single Ver.)』Music Video",
                      "モーニング娘。'25『気になるその気の歌』Promotion Edit",
                      "女子会 The Night",
                      "森高千里 『私の大事な人』 【セルフカヴァー】",
                      "松原健之「雪明かりの駅」Music Video（full ver.）",
                      "希望の夜",
                      "真野恵里菜 『My Days for You』 (Riverside Ver.)",
                      "Juice=Juice『初恋の亡霊』Promotion Edit",
                      "アンジュルム『アンドロイドは夢を見るか？』Promotion Edit",
                      "OCHA NORMA『女の愛想は武器じゃない』Promotion Edit",
                      "Super Red",
                      "まひるの星 (2012 Remaster)",
                      "Watarasebashi (Live at Aichi Prefectural Art Theater, 2023.11.12)",
                      "稲場愛香『終わらないインソムニア』Promotion Edit",
                      "本気ボンバー！！ (Instrumental)",
                      "I LOVE YOU",
                      "The Stress (Live at Aichi Prefectural Art Theater, 2023.11.12)",
                      "さよなら私の恋",
                      "アンジュルム『光のうた』Promotion Edit",
                      "VERY BEAUTY (Instrumental)",
                      "雨 [1999]",
                      "つばきファクトリー『My Days for You』Promotion Edit",
                      "二人は恋人 （Remix）",
                      "Junanasai (Live Rock Alive Complete at Nakano Sunplaza, 1992.9.30) (Including MC)",
                      "森高千里 『照れ屋』 【セルフカヴァー】",
                      "27yo",
                      "Tokyo Rush (Kondo Wa More Better Yo! Version) (Live at Showa Women's University Hitomi Memorial...",
                      "ME:I (ミーアイ) : LEAP HIGH! 〜明日へ、めいっぱい〜 Dance Practice 11 ver.",
                      "【鈴木愛理】原神スカーク イメージソング「Star Odyssey」MV",
                      "松原健之「愛になるふたり」Music Video",
                      "Kaze Ni Fukarete (Live at Aichi Prefectural Art Theater, 2023.11.12)",
                      "鈴木愛理『Oops!』(Music Video)",
                      "サクラハラクサ",
                      "ME:I (ミーアイ) ⊹ 'MUSE' Official MV",
                      "転生chu☆",
                      "森高千里 『地味な女』 【セルフカヴァー】",
                      "ロージークロニクル『夏のイナズマ』Promotion Edit",
                      "地味な女",
                      "Hello！のテーマ (Berryz工房 Version)",
                      "松原健之「冬のひまわり」Music Video（full ver.）",
                      "ロージークロニクル『ウブとズル』Promotion Edit",
                      "アイセヨイマヲ",
                      "Beyond (End Credit Version) (From \"Moana 2\"/Japanese Soundtrack Version)",
                      "譜久村聖『ロングラブレター』Promotion Edit",
                      "ロージークロニクル『へいらっしゃい！～ニッポンで会いましょう～』Promotion Edit",
                      "松原健之「マリモの湖」Music Video（full ver.）",
                      "モーニング娘。'25『明るく良い子』Promotion Edit",
                      "ゆらり",
                      "なんちゅう恋をやってるぅ YOU KNOW？ (Instrumental)",
                      "ロージークロニクル『ガオガオガオ』Promotion Edit"];
    let proceed_titles = titles.into_iter().take(5).map(|title| {
        struct_title(title.to_owned())
    }).collect::<Vec<_>>();

    join_all(proceed_titles).await.into_iter().for_each(|result| {
        match result {
            Ok(structured_title) => {
                println!("Structured Title: {:?}", structured_title);
            }
            Err(e) => {
                eprintln!("Error processing title: {}", e);
            }
        }
    });
    ()
}

