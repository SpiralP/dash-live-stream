#[cfg(feature = "tls")]
mod cert;

use crate::error::*;
use futures::prelude::*;
use log::*;
use reqwest::header::CONTENT_LENGTH;
use std::{
    collections::HashMap,
    convert::Infallible,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use warp::{fs::File, http::StatusCode, Filter, Rejection, Reply};

const INDEX: &str = include_str!("index.html");

pub async fn start(addr: SocketAddr, temp_dir: PathBuf, tls: bool, log: bool) -> Result<()> {
    let clients: Arc<Mutex<HashMap<IpAddr, Instant>>> = Default::default();
    let sent_bytes: Arc<AtomicUsize> = Arc::new(AtomicUsize::new(0));

    let checker_handle = {
        let clients = clients.clone();
        let sent_bytes = sent_bytes.clone();

        let (f, handle) = async move {
            let mut max_bytes_per_second = 0;

            loop {
                tokio::time::delay_for(Duration::from_secs(1)).await;
                let now = Instant::now();
                {
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

                // show bitrate
                {
                    let byte_count = sent_bytes.swap(0, Ordering::SeqCst);
                    if log {
                        info!("{:.1} kbps", byte_count / 1000);
                    }

                    if byte_count > max_bytes_per_second {
                        max_bytes_per_second = byte_count;
                        info!("new max: {} kbps", max_bytes_per_second / 1000);
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
                move |addr: Option<SocketAddr>, proxy_ip: Option<IpAddr>, file: File| {
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

                    let response = file.into_response();
                    if let Some(s) = response.headers().get(CONTENT_LENGTH) {
                        if let Ok(s) = s.to_str() {
                            if let Ok(byte_count) = s.parse::<usize>() {
                                sent_bytes.fetch_add(byte_count, Ordering::SeqCst);
                            }
                        }
                    }

                    response
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

    {
        // lookup external/internet ip address
        let port = addr.port();
        tokio::spawn(async move {
            let result = async move {
                let response_text = reqwest::get("https://api.ipify.org/").await?.text().await?;
                let ip: Ipv4Addr = response_text.parse()?;
                Ok::<_, Error>(ip)
            };

            match result.await {
                Ok(ip) => {
                    info!("external link {}://{}:{}/stream.mpd", protocol, ip, port);
                }
                Err(e) => {
                    warn!("error looking up external ip: {}", e);
                }
            }
        });
    }

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
