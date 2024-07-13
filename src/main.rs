use warp::Filter;

#[tokio::main]
async fn main() {
    let test = warp::path("hello")
        .map(|| "123");
    warp::serve(test).run(8080).await;
}
