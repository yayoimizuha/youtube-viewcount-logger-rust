use youtube_viewcount_logger_rust::struct_title;

#[tokio::main]
async fn main() {
    let structured_title = struct_title("アンジュルム「SHAKA SHAKA TO LOVE」".to_owned()).await.unwrap();
    println!("{:?}", structured_title);
}