//! Minimal SOCKS5 CONNECT client for TCP-based Modbus transports.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::net::IpAddr;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[derive(Debug, thiserror::Error)]
pub enum TcpConnectError {
    #[error("{0}")]
    Timeout(String),
    #[error("{0}")]
    Connection(String),
}

/// SOCKS5 proxy settings shared by all TCP-based master transports.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Socks5Config {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
}

impl Default for Socks5Config {
    fn default() -> Self {
        Self {
            enabled: false,
            host: "127.0.0.1".to_string(),
            port: 1080,
            username: String::new(),
            password: String::new(),
        }
    }
}

impl fmt::Debug for Socks5Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Socks5Config")
            .field("enabled", &self.enabled)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .finish()
    }
}

impl Socks5Config {
    pub fn is_disabled(&self) -> bool {
        !self.enabled
    }

    pub fn validate(&self) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }
        if self.host.trim().is_empty() {
            return Err("SOCKS5 proxy host is required".to_string());
        }
        if self.port == 0 {
            return Err("SOCKS5 proxy port must be between 1 and 65535".to_string());
        }

        let username_len = self.username.len();
        let password_len = self.password.len();
        if (username_len == 0) != (password_len == 0) {
            return Err(
                "SOCKS5 username and password must either both be set or both be empty".to_string(),
            );
        }
        if username_len > 255 || password_len > 255 {
            return Err("SOCKS5 username and password must not exceed 255 bytes".to_string());
        }
        Ok(())
    }

    fn authentication_method(&self) -> u8 {
        if self.username.is_empty() {
            0x00
        } else {
            0x02
        }
    }
}

/// Connect to a TCP target directly or through the configured SOCKS5 proxy.
/// The timeout covers DNS lookup, proxy connection, authentication, and CONNECT.
pub async fn connect_tcp(
    target_host: &str,
    target_port: u16,
    proxy: &Socks5Config,
    timeout: Duration,
) -> Result<TcpStream, TcpConnectError> {
    if target_host.trim().is_empty() {
        return Err(TcpConnectError::Connection(
            "target host is required".to_string(),
        ));
    }
    if target_port == 0 {
        return Err(TcpConnectError::Connection(
            "target port must be between 1 and 65535".to_string(),
        ));
    }
    proxy.validate().map_err(TcpConnectError::Connection)?;

    let connect = async {
        if proxy.enabled {
            connect_via_proxy(target_host, target_port, proxy).await
        } else {
            TcpStream::connect((target_host, target_port))
                .await
                .map_err(|e| format!("TCP connection failed: {e}"))
        }
    };

    tokio::time::timeout(timeout, connect)
        .await
        .map_err(|_| {
            TcpConnectError::Timeout(if proxy.enabled {
                "SOCKS5 connection timed out".to_string()
            } else {
                "TCP connection timed out".to_string()
            })
        })?
        .map_err(TcpConnectError::Connection)
}

async fn connect_via_proxy(
    target_host: &str,
    target_port: u16,
    proxy: &Socks5Config,
) -> Result<TcpStream, String> {
    let mut stream = TcpStream::connect((proxy.host.as_str(), proxy.port))
        .await
        .map_err(|e| format!("SOCKS5 proxy connection failed: {e}"))?;

    let method = proxy.authentication_method();
    stream
        .write_all(&[0x05, 0x01, method])
        .await
        .map_err(|e| format!("SOCKS5 method negotiation write failed: {e}"))?;

    let mut method_reply = [0u8; 2];
    stream
        .read_exact(&mut method_reply)
        .await
        .map_err(|e| format!("SOCKS5 method negotiation read failed: {e}"))?;
    if method_reply[0] != 0x05 {
        return Err(format!(
            "SOCKS5 proxy returned unsupported version {}",
            method_reply[0]
        ));
    }
    if method_reply[1] == 0xff {
        return Err("SOCKS5 proxy rejected all authentication methods".to_string());
    }
    if method_reply[1] != method {
        return Err(format!(
            "SOCKS5 proxy selected unexpected authentication method 0x{:02x}",
            method_reply[1]
        ));
    }

    if method == 0x02 {
        authenticate_username_password(&mut stream, proxy).await?;
    }

    let request = connect_request(target_host, target_port)?;
    stream
        .write_all(&request)
        .await
        .map_err(|e| format!("SOCKS5 CONNECT write failed: {e}"))?;

    let mut reply = [0u8; 4];
    stream
        .read_exact(&mut reply)
        .await
        .map_err(|e| format!("SOCKS5 CONNECT reply failed: {e}"))?;
    if reply[0] != 0x05 {
        return Err(format!(
            "SOCKS5 proxy returned unsupported version {}",
            reply[0]
        ));
    }
    if reply[2] != 0x00 {
        return Err("SOCKS5 proxy returned an invalid reserved field".to_string());
    }
    if reply[1] != 0x00 {
        return Err(format!(
            "SOCKS5 CONNECT failed: {}",
            reply_message(reply[1])
        ));
    }

    consume_bound_address(&mut stream, reply[3]).await?;
    Ok(stream)
}

async fn authenticate_username_password(
    stream: &mut TcpStream,
    proxy: &Socks5Config,
) -> Result<(), String> {
    let username = proxy.username.as_bytes();
    let password = proxy.password.as_bytes();
    let mut request = Vec::with_capacity(3 + username.len() + password.len());
    request.extend_from_slice(&[0x01, username.len() as u8]);
    request.extend_from_slice(username);
    request.push(password.len() as u8);
    request.extend_from_slice(password);
    stream
        .write_all(&request)
        .await
        .map_err(|e| format!("SOCKS5 authentication write failed: {e}"))?;

    let mut reply = [0u8; 2];
    stream
        .read_exact(&mut reply)
        .await
        .map_err(|e| format!("SOCKS5 authentication reply failed: {e}"))?;
    if reply[0] != 0x01 {
        return Err(format!(
            "SOCKS5 proxy returned unsupported authentication version {}",
            reply[0]
        ));
    }
    if reply[1] != 0x00 {
        return Err("SOCKS5 username/password authentication failed".to_string());
    }
    Ok(())
}

fn connect_request(target_host: &str, target_port: u16) -> Result<Vec<u8>, String> {
    let mut request = vec![0x05, 0x01, 0x00];
    match target_host.parse::<IpAddr>() {
        Ok(IpAddr::V4(address)) => {
            request.push(0x01);
            request.extend_from_slice(&address.octets());
        }
        Ok(IpAddr::V6(address)) => {
            request.push(0x04);
            request.extend_from_slice(&address.octets());
        }
        Err(_) => {
            let host = target_host.as_bytes();
            if host.is_empty() || host.len() > 255 {
                return Err("SOCKS5 target hostname must be between 1 and 255 bytes".to_string());
            }
            request.extend_from_slice(&[0x03, host.len() as u8]);
            request.extend_from_slice(host);
        }
    }
    request.extend_from_slice(&target_port.to_be_bytes());
    Ok(request)
}

async fn consume_bound_address(stream: &mut TcpStream, address_type: u8) -> Result<(), String> {
    let address_len = match address_type {
        0x01 => 4,
        0x03 => {
            let mut length = [0u8; 1];
            stream
                .read_exact(&mut length)
                .await
                .map_err(|e| format!("SOCKS5 bound address read failed: {e}"))?;
            length[0] as usize
        }
        0x04 => 16,
        other => {
            return Err(format!(
                "SOCKS5 proxy returned unsupported address type 0x{other:02x}"
            ))
        }
    };

    let mut remaining = vec![0u8; address_len + 2];
    stream
        .read_exact(&mut remaining)
        .await
        .map_err(|e| format!("SOCKS5 bound address read failed: {e}"))?;
    Ok(())
}

fn reply_message(code: u8) -> &'static str {
    match code {
        0x01 => "general proxy failure",
        0x02 => "connection not allowed by proxy ruleset",
        0x03 => "network unreachable",
        0x04 => "host unreachable",
        0x05 => "connection refused",
        0x06 => "TTL expired",
        0x07 => "command not supported",
        0x08 => "address type not supported",
        _ => "unassigned proxy error",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn connects_with_no_auth_and_preserves_domain_name() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_address = listener.local_addr().unwrap();
        let proxy_task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut greeting = [0u8; 3];
            socket.read_exact(&mut greeting).await.unwrap();
            assert_eq!(greeting, [0x05, 0x01, 0x00]);
            socket.write_all(&[0x05, 0x00]).await.unwrap();

            let mut header = [0u8; 5];
            socket.read_exact(&mut header).await.unwrap();
            assert_eq!(header, [0x05, 0x01, 0x00, 0x03, 11]);
            let mut target = [0u8; 13];
            socket.read_exact(&mut target).await.unwrap();
            assert_eq!(&target[..11], b"example.com");
            assert_eq!(u16::from_be_bytes([target[11], target[12]]), 502);

            socket
                .write_all(&[0x05, 0x00, 0x00, 0x01, 127, 0, 0, 1, 0x12, 0x34])
                .await
                .unwrap();
            socket.write_all(b"ok").await.unwrap();
        });

        let proxy = Socks5Config {
            enabled: true,
            host: proxy_address.ip().to_string(),
            port: proxy_address.port(),
            ..Socks5Config::default()
        };
        let mut stream = connect_tcp("example.com", 502, &proxy, Duration::from_secs(1))
            .await
            .unwrap();
        let mut payload = [0u8; 2];
        stream.read_exact(&mut payload).await.unwrap();
        assert_eq!(&payload, b"ok");
        proxy_task.await.unwrap();
    }

    #[tokio::test]
    async fn connects_with_username_password_authentication() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_address = listener.local_addr().unwrap();
        let proxy_task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut greeting = [0u8; 3];
            socket.read_exact(&mut greeting).await.unwrap();
            assert_eq!(greeting, [0x05, 0x01, 0x02]);
            socket.write_all(&[0x05, 0x02]).await.unwrap();

            let mut auth = [0u8; 12];
            socket.read_exact(&mut auth).await.unwrap();
            assert_eq!(
                &auth,
                &[0x01, 4, b'u', b's', b'e', b'r', 5, b's', b'e', b'c', b'r', b't']
            );
            socket.write_all(&[0x01, 0x00]).await.unwrap();

            let mut request = [0u8; 10];
            socket.read_exact(&mut request).await.unwrap();
            assert_eq!(request, [0x05, 0x01, 0x00, 0x01, 192, 0, 2, 10, 0x01, 0xf6]);
            socket
                .write_all(&[0x05, 0x00, 0x00, 0x03, 4, b't', b'e', b's', b't', 0, 80])
                .await
                .unwrap();
        });

        let proxy = Socks5Config {
            enabled: true,
            host: proxy_address.ip().to_string(),
            port: proxy_address.port(),
            username: "user".to_string(),
            password: "secrt".to_string(),
        };
        connect_tcp("192.0.2.10", 502, &proxy, Duration::from_secs(1))
            .await
            .unwrap();
        proxy_task.await.unwrap();
    }

    #[tokio::test]
    async fn reports_proxy_connect_rejection() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_address = listener.local_addr().unwrap();
        let proxy_task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut greeting = [0u8; 3];
            socket.read_exact(&mut greeting).await.unwrap();
            socket.write_all(&[0x05, 0x00]).await.unwrap();
            let mut request = [0u8; 10];
            socket.read_exact(&mut request).await.unwrap();
            socket
                .write_all(&[0x05, 0x05, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .await
                .unwrap();
        });

        let proxy = Socks5Config {
            enabled: true,
            host: proxy_address.ip().to_string(),
            port: proxy_address.port(),
            ..Socks5Config::default()
        };
        let error = connect_tcp("127.0.0.1", 502, &proxy, Duration::from_secs(1))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("connection refused"), "{error}");
        proxy_task.await.unwrap();
    }

    #[tokio::test]
    async fn classifies_proxy_handshake_timeout() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_address = listener.local_addr().unwrap();
        let proxy_task = tokio::spawn(async move {
            let (_socket, _) = listener.accept().await.unwrap();
            std::future::pending::<()>().await;
        });
        let proxy = Socks5Config {
            enabled: true,
            host: proxy_address.ip().to_string(),
            port: proxy_address.port(),
            ..Socks5Config::default()
        };

        let error = connect_tcp("127.0.0.1", 502, &proxy, Duration::from_millis(50))
            .await
            .unwrap_err();
        assert!(matches!(error, TcpConnectError::Timeout(_)));
        proxy_task.abort();
    }

    #[test]
    fn encodes_ipv6_target_address() {
        let request = connect_request("2001:db8::1", 502).unwrap();
        assert_eq!(&request[..4], &[0x05, 0x01, 0x00, 0x04]);
        assert_eq!(
            &request[4..20],
            &"2001:db8::1"
                .parse::<std::net::Ipv6Addr>()
                .unwrap()
                .octets()
        );
        assert_eq!(&request[20..], &502u16.to_be_bytes());
    }

    #[test]
    fn validates_authentication_pair_and_redacts_password() {
        let proxy = Socks5Config {
            enabled: true,
            username: "user".to_string(),
            password: String::new(),
            ..Socks5Config::default()
        };
        assert!(proxy.validate().is_err());

        let proxy = Socks5Config {
            password: "top-secret".to_string(),
            ..Socks5Config::default()
        };
        let debug = format!("{proxy:?}");
        assert!(!debug.contains("top-secret"));
    }
}
