use crate::error::*;
use log::*;
use std::{net::SocketAddr, path::PathBuf};
use warp::Filter;

const INDEX: &str = include_str!("index.html");

pub async fn start(addr: SocketAddr, temp_dir: PathBuf, tls: bool) -> Result<()> {
    let cors = warp::cors().allow_any_origin();

    let routes = warp::path::end()
        .map(|| warp::reply::html(INDEX))
        .or(warp::fs::dir(temp_dir.to_owned()))
        .with(cors);

    let protocol = if tls { "https" } else { "http" };

    debug!("binding to {}://{}/", protocol, addr);
    info!("dash file hosted at {}://{}/stream.mpd", protocol, addr);
    let server = warp::serve(routes);

    if tls {
        server.tls().bind(addr).await;
    } else {
        server.bind(addr).await;
    };

    Ok(())
}
