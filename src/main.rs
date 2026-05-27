use anyhow::{Context, Result};
use serde::Deserialize;
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{info, warn};

#[derive(Debug, Deserialize)]
struct Config {
    bind: String,
    port: u16,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let raw = tokio::fs::read_to_string("proxy.toml")
        .await
        .context("read proxy.toml")?;
    let cfg: Config = toml::from_str(&raw).context("parse proxy.toml")?;
    let addr: SocketAddr = format!("{}:{}", cfg.bind, cfg.port).parse()?;

    let listener = TcpListener::bind(addr).await?;
    info!(%addr, "proxy listening");

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

async fn tunnel_connect(client: &mut TcpStream, head: &[u8]) -> Result<()> {
    let end = head.iter().position(|&b| b == b'\n').context("no newline")?;
    let line = std::str::from_utf8(&head[..end])?.trim();

    // CONNECT host:port HTTP/1.1
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
