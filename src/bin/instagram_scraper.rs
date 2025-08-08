use std::collections::HashSet;
use std::fs;
use duckdb::{params, Connection};

#[tokio::main]
async fn main() {
    let duckdb = Connection::open("misc.duckdb").unwrap();
    duckdb.execute("CREATE TABLE IF NOT EXISTS instagram_accounts (id TEXT PRIMARY KEY);", params![]).unwrap();
    let mut usernames: HashSet<String> = duckdb.prepare("SELECT * FROM instagram_accounts;").unwrap().query_map(params![], |row| {
        Ok(row.get::<_, String>(0).unwrap())
    }).unwrap().filter_map(|v| { v.ok() }).collect();

    if fs::exists("instagram_user.list").unwrap() {
        // gather usernames from the file without started with #
        usernames = fs::read_to_string("instagram_user.list")
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
            }).collect();
    }
    println!("accounts={:?}", usernames.into_iter().collect::<Vec<String>>());
}

