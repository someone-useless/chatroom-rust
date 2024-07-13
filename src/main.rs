use std::{
    net::{Ipv4Addr, SocketAddrV4},
    sync::{atomic::AtomicI32, Arc},
};

use warp::Filter;

#[tokio::main]
async fn main() {
    let x = Arc::new(AtomicI32::from(0));
    let x1 = x.clone();
    let add = warp::path("add").map(move || {
        x.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        ""
    });
    let main = warp::path("").map(move || x1.load(std::sync::atomic::Ordering::Relaxed).to_string());
    warp::serve(main.or(add))
        .run(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8080))
        .await;
}
