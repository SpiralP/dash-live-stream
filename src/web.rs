use crate::error::*;
use log::*;
use std::{net::SocketAddr, path::PathBuf};
use warp::Filter;

const INDEX: &str = include_str!("index.html");

pub async fn start(addr: SocketAddr, temp_dir: PathBuf) -> Result<()> {
    let routes = warp::path::end()
        .map(|| warp::reply::html(INDEX))
        .or(warp::fs::dir(temp_dir.to_owned()))
        .with(warp::cors().allow_any_origin());

    info!("starting http server at http://{}/", addr);
    info!("hosting dash manifest at http://{}/stream.mpd", addr);

    warp::serve(routes).bind(addr).await;

    Ok(())
}
