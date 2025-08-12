use std::collections::HashSet;
use std::fs;
use duckdb::{params, Connection};
use reqwest::Client;

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:142.0) Gecko/20100101 Firefox/142.0";
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
    let duckdb = Connection::open("misc.duckdb").unwrap();
    duckdb.execute("CREATE TABLE IF NOT EXISTS instagram_accounts (id TEXT PRIMARY KEY);", params![]).unwrap();
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
    for username in usernames {
        println!("{}", username);
        let document = client.get(format!("https://www.instagram.com/{username}/embed")).send().await.unwrap().text().await.unwrap();
        fs::write("document.html", &document).unwrap();
        let json_start_pos = document.find("contextJSON").unwrap() + 15;
        let json_end_pos = detect_json_end(document[json_start_pos..].to_string()).unwrap() + json_start_pos;
        let json_str = &document[json_start_pos..json_end_pos];
        println!("{}", json_str);
        break;
    }
}

