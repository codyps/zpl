use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts},
    http::request::Parts,
};
use serde::Deserialize;

use iptrie::{Ipv4Prefix, Ipv6Prefix};
use std::sync::Arc;
use std::{convert::Infallible, net::SocketAddr};

#[derive(Debug, Clone)]
pub struct RealIp(pub std::net::IpAddr);

#[async_trait]
impl<S> FromRequestParts<S> for RealIp
where
    RealIpState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let connect_info = parts
            .extensions
            .get::<axum::extract::ConnectInfo<SocketAddr>>()
            .unwrap();
        let cf_connecting_ip = parts.headers.get("CF-Connecting-IP");

        let ip = connect_info.ip();
        match cf_connecting_ip {
            Some(cf_connecting_ip) => {
                let real_ip_state = RealIpState::from_ref(state);
                if real_ip_state.is_cf_ip(ip) {
                    return Ok(RealIp(cf_connecting_ip.to_str().unwrap().parse().unwrap()));
                }

                Ok(RealIp(ip))
            }
            None => Ok(RealIp(ip)),
        }
    }
}

#[derive(Deserialize, Debug)]
struct CfIpsResponse {
    #[allow(dead_code)]
    errors: Vec<CfCode>,
    #[allow(dead_code)]
    messages: Vec<CfCode>,
    result: CfIps,
}

#[derive(Deserialize, Debug)]
struct CfIps {
    #[allow(dead_code)]
    etag: String,
    ipv4_cidrs: Vec<String>,
    ipv6_cidrs: Vec<String>,
    success: bool,
}

#[derive(Deserialize, Debug)]
struct CfCode {
    #[allow(dead_code)]
    code: u16,
    #[allow(dead_code)]
    message: String,
}

#[derive(Clone)]
pub struct RealIpState {
    inner: Arc<Inner>,
}

struct Inner {
    // TODO: consider if we should merge these by using mapped addresses
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

        let cf_ips_v4 =
            iptrie::Ipv4LCTrieSet::from_iter(cf_ips.result.ipv4_cidrs.iter().map(|cidr| {
                let cidr: iptrie::Ipv4Prefix = cidr.parse().unwrap();
                cidr
            }));

        let cf_ips_v6 =
            iptrie::Ipv6LCTrieSet::from_iter(cf_ips.result.ipv6_cidrs.iter().map(|cidr| {
                let cidr: iptrie::Ipv6Prefix = cidr.parse().unwrap();
                cidr
            }));

        let inner = Arc::new(Inner {
            cf_ips_v4,
            cf_ips_v6,
        });

        Ok(Self { inner })
    }

    pub fn is_cf_ip(&self, ip: std::net::IpAddr) -> bool {
        match ip {
            std::net::IpAddr::V4(ip) => self.inner.cf_ips_v4.contains(&Ipv4Prefix::from(ip)),
            std::net::IpAddr::V6(ip) => self.inner.cf_ips_v6.contains(&Ipv6Prefix::from(ip)),
        }
    }
}
