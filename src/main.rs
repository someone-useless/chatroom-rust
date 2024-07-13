use std::net::{Ipv4Addr, SocketAddrV4};

use warp::Filter;

#[tokio::main]
async fn main() {
    let test = warp::path("hello")
        .map(|| "123");
    warp::serve(test).run(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8080)).await;
}
