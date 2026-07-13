use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::{fmt, sync::Arc};

use axum::async_trait;

use pingora::protocols::l4::virt::{VirtualSockOpt, VirtualSocket, VirtualSocketStream};
use pingora::protocols::l4::{socket::SocketAddr, stream::Stream as L4Stream};
use pingora::{connectors::L4Connect, upstreams::peer::HttpPeer};
use quinn::{RecvStream, SendStream};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use crate::state::AppState;

pub struct TunnelPeer {
    state: Arc<AppState>,
    tunnel_id: String,
    target: String,
    upstream_tls: bool,
    sni: String,
}
impl TunnelPeer {
    pub fn new(
        state: Arc<AppState>,
        tunnel_id: impl Into<String>,
        target: impl Into<String>,
        upstream_tls: bool,
        sni: impl Into<String>,
    ) -> Self {
        Self {
            state,
            tunnel_id: tunnel_id.into(),
            target: target.into(),
            upstream_tls,
            sni: sni.into(),
        }
    }
    pub fn into_http_peer(self) -> HttpPeer {
        let group_key = 0;

        let connector = TunnelConnector {
            state: self.state,
            tunnel_id: self.tunnel_id,
            target: self.target,
        };

        let mut peer = HttpPeer::new("0.0.0.0:0", self.upstream_tls, self.sni);

        peer.group_key = group_key;
        peer.options.custom_l4 = Some(Arc::new(connector));

        peer
    }
}
impl fmt::Debug for TunnelConnector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TunnelConnector")
            .field("tunnel_id", &self.tunnel_id)
            .field("target", &self.target)
            .finish_non_exhaustive()
    }
}

struct TunnelConnector {
    state: Arc<AppState>,
    tunnel_id: String,
    target: String,
}

#[async_trait]
impl L4Connect for TunnelConnector {
    async fn connect(&self, _address: &SocketAddr) -> pingora::Result<L4Stream> {
        let connection = {
            let tunnels = self.state.active_tunnels.read().await;

            tunnels.get(&self.tunnel_id).cloned().ok_or_else(|| {
                pingora::Error::explain(pingora::ErrorType::ConnectError, "Tunnel is offline")
            })?
        };

        let (mut send, recv) = connection.open_bi().await.map_err(|error| {
            pingora::Error::explain(
                pingora::ErrorType::ConnectError,
                format!("Failed to open QUIC stream: {error}"),
            )
        })?;

        let upstream_header = format!("{}\n", self.target);
        send.write_all(upstream_header.as_bytes())
            .await
            .map_err(|error| {
                pingora::Error::explain(
                    pingora::ErrorType::ConnectError,
                    format!("Failed to write upstream url: {error}"),
                )
            })?;

        let socket = TunnelSocket {
            send,
            recv,
            state: self.state.clone(),
            tunnel_id: self.tunnel_id.clone(),
        };

        Ok(L4Stream::from(VirtualSocketStream::new(Box::new(socket))))
    }
}

struct TunnelSocket {
    send: SendStream,
    recv: RecvStream,
    state: Arc<AppState>,
    tunnel_id: String,
}

impl fmt::Debug for TunnelSocket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TunnelSocket")
            .field("tunnel_id", &self.tunnel_id)
            .finish_non_exhaustive()
    }
}
impl AsyncRead for TunnelSocket {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let before = buffer.filled().len();

        let result = Pin::new(&mut self.recv).poll_read(cx, buffer);

        if let Poll::Ready(Ok(())) = &result {
            let received = buffer.filled().len() - before;

            if received > 0 {
                self.state
                    .stats
                    .record_tunnel_traffic(&self.tunnel_id, 0, received as u64);
            }
        }

        result
    }
}

impl AsyncWrite for TunnelSocket {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<io::Result<usize>> {
        let result = AsyncWrite::poll_write(Pin::new(&mut self.send), cx, buffer);

        let written = match &result {
            Poll::Ready(Ok(n)) => *n,
            _ => 0,
        };

        if written > 0 {
            self.state
                .stats
                .record_tunnel_traffic(&self.tunnel_id, written as u64, 0);
        }

        result
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.send).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.send).poll_shutdown(cx)
    }
}

impl VirtualSocket for TunnelSocket {
    fn set_socket_option(&self, _option: VirtualSockOpt) -> io::Result<()> {
        Ok(())
    }
}
