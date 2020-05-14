mod error;
mod logger;

use clap::*;
use warp::Filter;

#[tokio::main]
async fn main() {
    logger::initialize(cfg!(debug_assertions), false);

    let matches = App::new(crate_name!())
        .version(crate_version!())
        .about("Does awesome things")
        .arg(
            Arg::with_name("config")
                .short("c")
                .long("config")
                .value_name("FILE")
                .help("Sets a custom config file")
                .takes_value(true),
        )
        .get_matches();

    // GET /hello/warp => 200 OK with body "Hello, warp!"
    let hello = warp::path!("hello" / String).map(|name| format!("Hello, {}!", name));

    warp::serve(hello).run(([127, 0, 0, 1], 3030)).await;
}
