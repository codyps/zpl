use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts},
    http::request::Parts,
};
use serde::Deserialize;

use std::sync::Arc;
use std::{convert::Infallible, net::SocketAddr};

#[derive(Debug, Clone)]
pub struct RealIp(pub std::net::IpAddr);

#[async_trait]
impl<S> FromRequestParts<S> for RealIp
where
    Arc<RealIpState>: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let connect_info = parts
            .extensions
            .get::<axum::extract::ConnectInfo<SocketAddr>>()
            .unwrap();
        let cf_connecting_ip = parts.headers.get("CF-Connecting-IP");

        match cf_connecting_ip {
            Some(cf_connecting_ip) => {
                // check if remote_addr is a cloudflare ip
                // parse cf_connecting_ip to an ip address
                // return cf_connecting_ip
                todo!()
            }
            None => Ok(RealIp(connect_info.ip())),
        }
    }
}

#[derive(Deserialize, Debug)]
struct CfIpsResponse {
    errors: Vec<CfCode>,
    messages: Vec<CfCode>,
    result: CfIps,
}

#[derive(Deserialize, Debug)]
struct CfIps {
    etag: String,
    ipv4_cidrs: Vec<String>,
    ipv6_cidrs: Vec<String>,
    success: bool,
}

#[derive(Deserialize, Debug)]
struct CfCode {
    code: u16,
    message: String,
}

#[derive(Clone)]
pub struct RealIpState {
    inner: Arc<Inner>,
}

struct Inner {
    cf_ips_v4: iptrie::Ipv4LCTrieSet,
    cf_ips_v6: iptrie::Ipv6LCTrieSet,
}

impl RealIpState {
    pub async fn new() -> eyre::Result<Self> {
        // TODO: do this periodically
        let cf_ips = reqwest::get(
            "https://api.cloudflare.com/client/v4
/ips",
        )
        .await
        .unwrap()
        .json::<CfIpsResponse>()
        .await
        .unwrap();

        if !cf_ips.result.success {
            return Err(eyre::eyre!("failed to fetch cloudflare ips: {:?}", cf_ips));
        }

        let cf_ips_v4 = cf_ips
            .result
            .ipv4_cidrs
            .iter()
            .map(|cidr| {
                let cidr: iptrie::Ipv4Prefix = cidr.parse().unwrap();
                cidr
            })
            .collect();

        let cf_ips_v6 = cf_ips
            .result
            .ipv6_cidrs
            .iter()
            .map(|cidr| {
                let cidr: iptrie::Ipv6Prefix = cidr.parse().unwrap();
                cidr
            })
            .collect();

        let inner = Arc::new(Inner {
            cf_ips_v4,
            cf_ips_v6,
        });

        Ok(Self { inner })
    }
}
