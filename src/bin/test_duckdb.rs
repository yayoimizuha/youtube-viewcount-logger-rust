use duckdb::{params, Connection};
#[tokio::main]
async fn main() {
    let duckdb = Connection::open("data.duckdb").unwrap();
    println!("aaaa");
    let _ = duckdb.prepare("SELECT raw_title FROM __title__ WHERE youtube_id = (?)").unwrap().query_map(params!["FJfife3J8uM".to_owned()], |row| {
        let raw_title = row.get::<_,Option<String>>(0).unwrap();
        println!("raw_title: {:?}", raw_title);
        Ok(())
    }).unwrap().count();
}