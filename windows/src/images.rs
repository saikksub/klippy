//! Downloads a copy's images for the Rust core, which cleans them. Only
//! https; no cookies, cache or saved logins; each capped in size and time.

use std::time::Duration;

use ureq::{
    Agent,
    tls::{TlsConfig, TlsProvider},
};

/// Matches `clean::MAX_IMAGE_BYTES` in the core, which checks it again.
const MAX_BYTES: u64 = 5 * 1024 * 1024;

/// One result per URL, in order; `None` where the download failed.
pub fn fetch(urls: &[String]) -> Vec<Option<Vec<u8>>> {
    let agent: Agent = Agent::config_builder()
        .https_only(true)
        .timeout_global(Some(Duration::from_secs(10)))
        // Schannel, the system's TLS.
        .tls_config(TlsConfig::builder().provider(TlsProvider::NativeTls).build())
        .build()
        .into();
    std::thread::scope(|scope| {
        let downloads: Vec<_> = urls
            .iter()
            .map(|url| scope.spawn(|| download(&agent, url)))
            .collect();
        downloads
            .into_iter()
            .map(|download| download.join().ok().flatten())
            .collect()
    })
}

fn download(agent: &Agent, url: &str) -> Option<Vec<u8>> {
    if !url.get(..8)?.eq_ignore_ascii_case("https://") {
        return None;
    }
    let mut response = agent.get(url).call().ok()?;
    response.body_mut().with_config().limit(MAX_BYTES).read_to_vec().ok()
}
