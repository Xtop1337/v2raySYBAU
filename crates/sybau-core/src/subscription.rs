//! Subscription download and merge.

use std::time::Duration;

use anyhow::{bail, Context, Result};
use chrono::Utc;

use crate::import::import_text;
use crate::model::{Profile, Subscription};
use crate::util::b64_decode_str;

#[derive(Debug, Default)]
pub struct FetchedSubscription {
    pub profiles: Vec<Profile>,
    pub title: Option<String>,
    pub update_interval_hours: Option<u32>,
    pub upload: Option<u64>,
    pub download: Option<u64>,
    pub total: Option<u64>,
    pub expire: Option<i64>,
    pub errors: Vec<String>,
}

/// Downloads a subscription. `proxy` (e.g. `socks5h://127.0.0.1:10808`) is used when given,
/// which helps when the subscription host is blocked.
pub async fn fetch(url: &str, user_agent: &str, proxy: Option<&str>) -> Result<FetchedSubscription> {
    let mut builder = reqwest::Client::builder().timeout(Duration::from_secs(30)).user_agent(user_agent);
    builder = match proxy {
        Some(proxy) => builder.proxy(reqwest::Proxy::all(proxy)?),
        None => builder.no_proxy(),
    };
    let response = builder.build()?.get(url).send().await.context("download subscription")?;
    let status = response.status();
    if !status.is_success() {
        bail!("subscription server answered {status}");
    }
    let header = |name: &str| response.headers().get(name).and_then(|v| v.to_str().ok()).map(str::to_string);
    let userinfo = header("subscription-userinfo");
    let interval = header("profile-update-interval");
    let title = header("profile-title");
    let body = response.text().await?;
    Ok(parse_response(&body, userinfo.as_deref(), interval.as_deref(), title.as_deref()))
}

pub fn parse_response(body: &str, userinfo: Option<&str>, interval: Option<&str>, title: Option<&str>) -> FetchedSubscription {
    let imported = import_text(body);
    let mut fetched = FetchedSubscription { profiles: imported.profiles, errors: imported.errors, ..Default::default() };
    if let Some(info) = userinfo {
        for part in info.split(';') {
            let Some((key, value)) = part.trim().split_once('=') else { continue };
            let value = value.trim();
            match key.trim() {
                "upload" => fetched.upload = value.parse().ok(),
                "download" => fetched.download = value.parse().ok(),
                "total" => fetched.total = value.parse().ok(),
                "expire" => fetched.expire = value.parse().ok(),
                _ => {}
            }
        }
    }
    fetched.update_interval_hours = interval.and_then(|v| v.trim().parse().ok());
    fetched.title = title.map(|t| match t.strip_prefix("base64:") {
        Some(encoded) => b64_decode_str(encoded).unwrap_or_else(|| t.to_string()),
        None => t.to_string(),
    });
    fetched
}

/// Replaces the profiles of `subscription` inside `all`, keeping ids and measured delays of
/// servers that are still present so the selection and the delay column survive updates.
pub fn merge(all: &mut Vec<Profile>, subscription: &Subscription, fresh: Vec<Profile>) -> usize {
    let group = if subscription.name.trim().is_empty() { "Subscription".to_string() } else { subscription.name.clone() };
    let old: Vec<Profile> = all.iter().filter(|p| p.subscription_id == Some(subscription.id)).cloned().collect();
    let insert_at = all.iter().position(|p| p.subscription_id == Some(subscription.id)).unwrap_or(all.len());
    all.retain(|p| p.subscription_id != Some(subscription.id));

    let mut seen = std::collections::HashSet::new();
    let merged: Vec<Profile> = fresh
        .into_iter()
        .filter(|p| seen.insert(p.identity_key()))
        .map(|mut p| {
            if let Some(previous) = old.iter().find(|o| o.identity_key() == p.identity_key()) {
                p.id = previous.id;
                p.delay = previous.delay.clone();
                p.created_at = previous.created_at;
            }
            p.subscription_id = Some(subscription.id);
            p.group = group.clone();
            p
        })
        .collect();
    let count = merged.len();
    let insert_at = insert_at.min(all.len());
    all.splice(insert_at..insert_at, merged);
    count
}

/// Applies header metadata to the subscription record after a successful fetch.
pub fn mark_updated(subscription: &mut Subscription, fetched: &FetchedSubscription) {
    subscription.updated_at = Some(Utc::now());
    subscription.last_error = None;
    subscription.upload = fetched.upload;
    subscription.download = fetched.download;
    subscription.total = fetched.total;
    subscription.expire = fetched.expire;
    if subscription.name.trim().is_empty() {
        if let Some(title) = &fetched.title {
            subscription.name = title.clone();
        }
    }
}

pub fn is_due(subscription: &Subscription) -> bool {
    if !subscription.enabled || subscription.auto_update_hours == 0 {
        return false;
    }
    match subscription.updated_at {
        None => true,
        Some(at) => Utc::now() - at >= chrono::Duration::hours(subscription.auto_update_hours as i64),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::b64_encode;

    #[test]
    fn parses_headers_and_body() {
        let body = b64_encode(b"vless://id@a.com:443#A\nvless://id@b.com:443#B");
        let f = parse_response(
            &body,
            Some("upload=10; download=20; total=1000; expire=1767225600"),
            Some("24"),
            Some("base64:0JzQvtGPINC/0L7QtNC/0LjRgdC60LA="),
        );
        assert_eq!(f.profiles.len(), 2);
        assert_eq!((f.upload, f.download, f.total, f.expire), (Some(10), Some(20), Some(1000), Some(1767225600)));
        assert_eq!(f.update_interval_hours, Some(24));
        assert_eq!(f.title.as_deref(), Some("Моя подписка"));
    }

    #[test]
    fn merge_keeps_ids_and_delays() {
        let sub = Subscription { name: "Sub".into(), ..Default::default() };
        let manual = Profile { name: "manual".into(), address: "m.com".into(), ..Default::default() };
        let mut all = vec![manual.clone()];
        let first = parse_response("vless://id@a.com:443#A\nvless://id@b.com:443#B", None, None, None).profiles;
        assert_eq!(merge(&mut all, &sub, first), 2);
        all[1].delay.ms = Some(123);
        let kept_id = all[1].id;

        let second = parse_response("vless://id@a.com:443#A renamed\nvless://id@c.com:443#C", None, None, None).profiles;
        merge(&mut all, &sub, second);
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].name, "manual");
        assert_eq!(all[1].id, kept_id);
        assert_eq!(all[1].delay.ms, Some(123));
        assert_eq!(all[1].name, "A renamed");
        assert_eq!(all[2].address, "c.com");
        assert!(all[1..].iter().all(|p| p.group == "Sub" && p.subscription_id == Some(sub.id)));
    }
}
