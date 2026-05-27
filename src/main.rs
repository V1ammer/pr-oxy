use anyhow::{Context, Result};
use base64::Engine as _;
use std::env;
use std::net::SocketAddr;
use std::sync::OnceLock;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{info, warn};

#[derive(Debug)]
struct Auth {
    user: String,
    pass: String,
}

#[derive(Debug)]
struct Config {
    addr: SocketAddr,
    auth: Option<Auth>,
}

static CONFIG: OnceLock<Config> = OnceLock::new();

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let port: u16 = env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let auth = match (env::var("USER"), env::var("PASS")) {
        (Ok(u), Ok(p)) => Some(Auth { user: u, pass: p }),
        _ => None,
    };

    let cfg = Config {
        addr: SocketAddr::from(([0, 0, 0, 0], port)),
        auth,
    };

    CONFIG.set(cfg).unwrap();
    let cfg = CONFIG.get().unwrap();

    let listener = TcpListener::bind(cfg.addr).await?;
    info!(addr = %cfg.addr, "proxy listening");

    loop {
        let (client, peer) = listener.accept().await?;
        tokio::spawn(async move {
            if let Err(e) = serve(client).await {
                warn!(%peer, "error: {e}");
            }
        });
    }
}

async fn serve(mut client: TcpStream) -> Result<()> {
    client.set_nodelay(true)?;

    let mut buf = [0u8; 8192];
    let n = read_headers(&mut client, &mut buf).await?;
    let head = &buf[..n];

    if let Some(ref auth) = CONFIG.get().unwrap().auth {
        if !check_auth(head, auth) {
            client
                .write_all(b"HTTP/1.1 407 Proxy Authentication Required\r\nProxy-Authenticate: Basic\r\nContent-Length: 0\r\n\r\n")
                .await?;
            return Ok(());
        }
    }

    if head.starts_with(b"CONNECT ") {
        tunnel_connect(&mut client, head).await
    } else {
        proxy_http(&mut client, head).await
    }
}

async fn read_headers(stream: &mut TcpStream, buf: &mut [u8]) -> Result<usize> {
    let mut pos = 0;
    loop {
        let n = stream.read(&mut buf[pos..]).await?;
        if n == 0 {
            anyhow::bail!("client closed before headers complete");
        }
        pos += n;
        if buf[..pos].windows(4).any(|w| w == b"\r\n\r\n") {
            return Ok(pos);
        }
        if pos >= buf.len() {
            anyhow::bail!("headers too large");
        }
    }
}

fn check_auth(head: &[u8], auth: &Auth) -> bool {
    let text = match std::str::from_utf8(head) {
        Ok(t) => t,
        Err(_) => return false,
    };
    for line in text.lines() {
        let Some((key, val)) = line.split_once(':') else { continue };
        if !key.trim().eq_ignore_ascii_case("proxy-authorization") {
            continue;
        }
        let val = val.trim();
        let b64 = val
            .strip_prefix("Basic ")
            .or_else(|| val.strip_prefix("basic "))
            .unwrap_or(val);
        let decoded = match base64::engine::general_purpose::STANDARD.decode(b64) {
            Ok(d) => d,
            Err(_) => return false,
        };
        let creds = match std::str::from_utf8(&decoded) {
            Ok(c) => c,
            Err(_) => return false,
        };
        let mut parts = creds.splitn(2, ':');
        if let (Some(u), Some(p)) = (parts.next(), parts.next()) {
            return u == auth.user && p == auth.pass;
        }
    }
    false
}

async fn tunnel_connect(client: &mut TcpStream, head: &[u8]) -> Result<()> {
    let end = head.iter().position(|&b| b == b'\n').context("no newline")?;
    let line = std::str::from_utf8(&head[..end])?.trim();

    let target = line.split_whitespace().nth(1).context("bad CONNECT")?;
    info!(target, "tunnel");

    let mut upstream = TcpStream::connect(target).await?;
    upstream.set_nodelay(true)?;

    client
        .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
        .await?;

    tokio::io::copy_bidirectional(client, &mut upstream).await?;
    Ok(())
}

async fn proxy_http(client: &mut TcpStream, head: &[u8]) -> Result<()> {
    let host = extract_host(head).context("missing Host header")?;
    let target = format!("{host}:80");
    info!(target, "http");

    let mut upstream = TcpStream::connect(&target).await?;
    upstream.set_nodelay(true)?;

    upstream.write_all(head).await?;
    tokio::io::copy_bidirectional(client, &mut upstream).await?;
    Ok(())
}

fn extract_host(data: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(data).ok()?;
    for line in text.lines() {
        let lower = line.to_ascii_lowercase();
        if let Some(val) = lower.strip_prefix("host:") {
            return Some(val.trim().split(':').next()?.to_string());
        }
    }
    None
}
