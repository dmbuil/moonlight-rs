//! Sign-in throttling (roadmap 50): failed sign-ins counted per username and
//! per client address, in this process's memory. After `max` failures, `login`
//! answers without asking Keystone until `window` has passed since the last
//! failure. Many clouds leave Keystone's own lockout
//! (`[security_compliance] lockout_failure_attempts`) off.
//!
//! Per process: with several replicas behind a balancer each counts on its own.
//! Keys are hashes, so usernames and addresses are never stored as such.
//!
//! Counting is atomic per key, but `blocked` and `failed` are two steps: a
//! burst of parallel attempts can all pass `blocked` before the first one
//! fails, so at most the limit plus one burst reaches Keystone (roadmap 63).

use std::time::Duration;

use moka::future::Cache;

/// Failure counters with their limit; `max == 0` turns throttling off.
pub struct Throttle {
    failures: Cache<String, u32>,
    max: u32,
}

impl Throttle {
    /// `window`: how long a counter lives after its last failure.
    pub fn new(max: u32, window: Duration) -> Self {
        // `time_to_live` restarts on every insert, so the window counts from
        // the last failure, not the first.
        Throttle { failures: Cache::builder().max_capacity(100_000).time_to_live(window).build(), max }
    }

    /// True when any key has reached the limit: refuse without calling Keystone.
    pub async fn blocked(&self, keys: &[String]) -> bool {
        if self.max == 0 {
            return false;
        }
        for k in keys {
            if self.failures.get(k).await.unwrap_or(0) >= self.max {
                return true;
            }
        }
        false
    }

    /// Counts one failed sign-in against every key.
    pub async fn failed(&self, keys: &[String]) {
        if self.max == 0 {
            return;
        }
        for k in keys {
            // One atomic read-modify-write: a `get` then `insert` lost the
            // failures of parallel attempts (roadmap 63).
            self.failures
                .entry(k.clone())
                .and_upsert_with(|e| async move { e.map_or(1, |e| e.into_value().saturating_add(1)) })
                .await;
        }
    }

    /// A successful sign-in clears the **username's** counter only: an address
    /// that guessed many accounts stays slowed down even if one guess was right.
    pub async fn succeeded(&self, username_key: &str) {
        self.failures.invalidate(username_key).await;
    }
}

/// The counter keys of one attempt: the username (per domain, case-insensitive,
/// like Keystone's lookup) and, when `[log] client_ip_header` gives one, the
/// client address. The username key comes first.
pub fn keys(domain: &str, username: &str, client: Option<&str>) -> Vec<String> {
    let mut out = vec![format!("user:{}", super::os::hash_key(&[&domain.to_lowercase(), &username.to_lowercase()]))];
    if let Some(ip) = client.filter(|ip| !ip.is_empty() && *ip != "-") {
        out.push(format!("ip:{}", super::os::hash_key(&[ip])));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn blocks_after_max_failures_and_success_clears_only_the_user() {
        let t = Throttle::new(3, Duration::from_secs(60));
        let alice = keys("Default", "Alice", Some("203.0.113.9"));
        for _ in 0..2 {
            t.failed(&alice).await;
        }
        assert!(!t.blocked(&alice).await, "2 of 3");
        t.failed(&alice).await;
        assert!(t.blocked(&alice).await, "3 of 3");
        assert!(t.blocked(&keys("default", "ALICE", None)).await, "the username key is case-insensitive");
        let bob_same_ip = keys("Default", "bob", Some("203.0.113.9"));
        assert!(t.blocked(&bob_same_ip).await, "the address is blocked for every user");
        t.succeeded(&alice[0]).await;
        assert!(!t.blocked(&keys("Default", "alice", None)).await, "success clears the user");
        assert!(t.blocked(&bob_same_ip).await, "but not the address");
    }

    #[tokio::test]
    async fn the_window_counts_from_the_last_failure_and_zero_turns_it_off() {
        let t = Throttle::new(1, Duration::from_millis(50));
        let k = keys("Default", "carol", None);
        t.failed(&k).await;
        assert!(t.blocked(&k).await);
        tokio::time::sleep(Duration::from_millis(120)).await;
        t.failures.run_pending_tasks().await;
        assert!(!t.blocked(&k).await, "expired");
        let off = Throttle::new(0, Duration::from_secs(60));
        off.failed(&k).await;
        assert!(!off.blocked(&k).await, "max = 0: never blocks");
    }

    #[tokio::test]
    async fn parallel_failures_are_all_counted() {
        let t = Throttle::new(100, Duration::from_secs(60));
        let k = keys("Default", "erin", Some("203.0.113.9"));
        let f = || t.failed(&k);
        tokio::join!(f(), f(), f(), f(), f(), f(), f(), f(), f(), f());
        for key in &k {
            assert_eq!(t.failures.get(key).await, Some(10), "a get-then-insert undercounted");
        }
    }

    #[test]
    fn keys_hide_usernames_and_skip_unknown_addresses() {
        let k = keys("Default", "dave", Some("-"));
        assert_eq!(k.len(), 1, "`-` = no client address configured");
        assert!(!k[0].contains("dave"));
    }
}
