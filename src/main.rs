#![warn(rust_2018_idioms)]

use std::error::Error;
use std::net::SocketAddr;
use std::{env, io};
use tokio;
use tokio::net::UdpSocket;
use tokio_util::codec::BytesCodec;
use tokio_util::udp::UdpFramed;

use bytes::Bytes;
use futures::{FutureExt, SinkExt, StreamExt};

use std::time::{SystemTime, Duration};

const TIMEOUT_THRESHOLD: u64 = 60;

struct Server {
    framed_socket: UdpFramed<BytesCodec>,
    to_send: Option<(Bytes, SocketAddr)>,
}

#[derive(Debug, Copy, Clone)]
struct Peer {
    joined_time: SystemTime,
    updated_time: SystemTime,
    ip_addr: SocketAddr
}

impl Peer {
    pub fn set_updated_time(self, time: SystemTime) -> Self {
        Peer { updated_time: time, ..self }
    }
}

impl Server {
    fn remove_timing_out(peers: &mut Vec<Peer>) {
        let mut index_to_remove: Vec<usize> = Vec::new();

        for (index, peer) in peers.iter().enumerate() {
            if SystemTime::now().duration_since(peer.updated_time).unwrap() > Duration::from_secs(TIMEOUT_THRESHOLD) {
                index_to_remove.push(index);
            }
        }

        for index in index_to_remove {
            peers.swap_remove(index);
        }
    }

    fn get_peer_by_ip_addr(peers: &Vec<Peer>, ip_addr: SocketAddr) -> Option<(usize, &Peer)> {
        for (index, peer) in peers.iter().enumerate() {
            if peer.ip_addr == ip_addr {
                return Some((index, peer));
            }
        }

        return None;
    }

    async fn run(self) -> Result<(), io::Error> {
        let Server {
            mut framed_socket,
            mut to_send,
        } = self;
        let mut peers: Vec<Peer> = Vec::new();

        loop {
            Server::remove_timing_out(&mut peers);
            
            if let Some((bytes, ip_addr)) = to_send {
                if let Some((index, found_peer)) = Server::get_peer_by_ip_addr(&peers, ip_addr) {
                    peers[index] = found_peer.set_updated_time(SystemTime::now());
                } else {
                    peers.push(Peer {
                        joined_time: SystemTime::now(),
                        updated_time: SystemTime::now(),
                        ip_addr: ip_addr
                    });
                }

                for peer in &peers {
                    if peer.ip_addr != ip_addr {
                        framed_socket.send((bytes.clone(), peer.ip_addr)).await?;
                    }
                }
            }

            let (bytes, peer) = framed_socket.next().map(|e| e.unwrap()).await?;
            to_send = Some((bytes.freeze(), peer));
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let addr = env::args().nth(1).unwrap_or("127.0.0.1:6142".to_string());

    let socket = UdpSocket::bind(&addr).await?;

    println!("Listening on: {}", socket.local_addr()?);

    let framed_socket = UdpFramed::new(socket, BytesCodec::new());

    let server = Server {
        framed_socket,
        to_send: None,
    };

    server.run().await?;

    Ok(())
}
