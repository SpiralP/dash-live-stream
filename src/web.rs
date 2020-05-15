use crate::error::*;
use log::*;
use std::{net::SocketAddr, path::PathBuf};
use warp::Filter;

const INDEX: &str = include_str!("index.html");

pub async fn start(addr: SocketAddr, temp_dir: PathBuf) -> Result<()> {
    let cors = warp::cors().allow_any_origin();

    let routes = warp::path::end()
        .map(|| warp::reply::html(INDEX))
        .or(warp::fs::dir(temp_dir.to_owned()))
        .with(cors);

    debug!("binding to https://{}/", addr);
    info!("dash file hosted at https://{}/stream.mpd", addr);
    warp::serve(routes)
        // .tls()
        .bind(addr).await;

    Ok(())
}
