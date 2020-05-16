use crate::error::*;
use log::*;
use std::{convert::Infallible, net::SocketAddr, path::PathBuf};
use warp::{http::StatusCode, Filter, Rejection, Reply};

const INDEX: &str = include_str!("index.html");

pub async fn start(addr: SocketAddr, temp_dir: PathBuf) -> Result<()> {
    let routes = warp::path::end()
        .map(|| warp::reply::html(INDEX))
        .or(warp::fs::dir(temp_dir.to_owned()))
        .recover(handle_rejection)
        .with(warp::cors().allow_any_origin());

    info!("starting http server at http://{}/", addr);
    info!("hosting dash manifest at http://{}/stream.mpd", addr);

    warp::serve(routes).bind(addr).await;

    Ok(())
}

async fn handle_rejection(rejection: Rejection) -> std::result::Result<impl Reply, Infallible> {
    // hack so that cors works with 404 errors
    // this will create a successful reply which our with(cors) will then handle
    // https://github.com/seanmonstar/warp/issues/518
    if rejection.is_not_found() {
        Ok(warp::reply::with_status("", StatusCode::NOT_FOUND))
    } else {
        warn!("unhandled rejection: {:?}", rejection);
        Ok(warp::reply::with_status(
            "",
            StatusCode::INTERNAL_SERVER_ERROR,
        ))
    }
}
