use youtube_viewcount_logger_rust::struct_title;

#[tokio::main]
async fn main() {
    let structured_title = struct_title("アンジュルム『光のうた』Promotion Edit".to_owned()).await.unwrap();
    println!("{:?}", structured_title);
}