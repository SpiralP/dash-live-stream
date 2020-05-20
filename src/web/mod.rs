#[cfg(feature = "tls")]
mod cert;

use crate::error::*;
use futures::prelude::*;
use log::*;
use std::{
    collections::HashMap,
    convert::Infallible,
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use warp::{fs::File, http::StatusCode, Filter, Rejection, Reply};

const INDEX: &str = include_str!("index.html");

pub async fn start(addr: SocketAddr, temp_dir: PathBuf, tls: bool, log: bool) -> Result<()> {
    let clients: Arc<Mutex<HashMap<IpAddr, Instant>>> = Default::default();

    let checker_handle = {
        let clients = clients.clone();

        let (f, handle) = async move {
            loop {
                tokio::time::delay_for(Duration::from_millis(1000)).await;
                {
                    let now = Instant::now();

                    let mut to_remove = Vec::new();
                    let mut clients = clients.lock().unwrap();
                    for (ip, time) in clients.iter() {
                        if now - *time > Duration::from_secs(3 * 2) {
                            to_remove.push(*ip);
                        }
                    }

                    for ip in to_remove {
                        clients.remove(&ip);
                        info!("client {} left ({} clients)", ip, clients.len());
                    }
                }
            }
        }
        .remote_handle();
        tokio::spawn(f);
        handle
    };

    let routes = warp::path::end()
        .map(|| warp::reply::html(INDEX))
        .or(warp::addr::remote()
            .and(warp::header::optional::<IpAddr>("x-forwarded-for"))
            .and(warp::fs::dir(temp_dir.to_owned()))
            .map(
                move |addr: Option<SocketAddr>, proxy_ip: Option<IpAddr>, f: File| {
                    if let Some(ip) = proxy_ip.or_else(|| addr.map(|addr| addr.ip())) {
                        let mut clients = clients.lock().unwrap();
                        let len = clients.len();
                        clients
                            .entry(ip)
                            .and_modify(|time| {
                                *time = Instant::now();
                            })
                            .or_insert_with(|| {
                                info!("client {} connected ({} clients)", ip, len + 1);
                                Instant::now()
                            });
                    }
                    f
                },
            ))
        .recover(handle_rejection)
        .with(warp::cors().allow_any_origin())
        .with(warp::log::custom(move |info| {
            if log {
                debug!(
                    "{:?} \"{} {}\" {} \"{}\" {:?}",
                    info.remote_addr(),
                    info.method(),
                    info.path(),
                    info.status().as_u16(),
                    info.referer().unwrap_or("-"),
                    info.elapsed(),
                );
            }
        }));

    let protocol = if tls { "https" } else { "http" };

    info!("starting {} server at {}://{}/", protocol, protocol, addr);
    info!(
        "hosting dash manifest at {}://{}/stream.mpd",
        protocol, addr
    );

    let server = warp::serve(routes);
    if tls {
        #[cfg(feature = "tls")]
        {
            let (cert, key) = cert::generate_cert_and_key()?;

            let cert_bytes = cert.to_pem()?;
            let key_bytes = key.private_key_to_pem_pkcs8()?;

            server
                .tls()
                .cert(&cert_bytes)
                .key(&key_bytes)
                .bind(addr)
                .await;
        }
    } else {
        server.bind(addr).await;
    };

    drop(checker_handle);

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
