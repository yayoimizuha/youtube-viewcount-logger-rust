use duckdb::{params, Connection};

#[tokio::main]
async fn main() {
    let duckdb = Connection::open("data.duckdb").unwrap();
    duckdb.prepare("SELECT table_name FROM information_schema.tables WHERE NOT STARTS_WITH(table_name,'__') AND NOT ENDS_WITH(table_name,'__');").unwrap().query_map(params![], |row| {
        let db_name: String = row.get(0).unwrap();
        println!("Database: {}", db_name);
        Ok(())
    }).unwrap().count();
}