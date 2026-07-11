use std::collections::HashMap;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;

const MAX_REQUESTS: u32 = 5;
const WINDOW_DURATION: Duration = Duration::from_secs(300);

struct RateLimitEntry {
    count: u32,
    window_start: Instant,
}

static CONTACT_LIMITS: LazyLock<Mutex<HashMap<String, RateLimitEntry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Returns `true` if the request is within the rate limit, `false` if the
/// caller has exceeded the maximum allowed requests within the window.
///
/// # Lock Ordering
/// This function acquires `CONTACT_LIMITS` lock and holds it for the duration
/// of the read-modify-write. This is safe because the lock is uncontended
/// (per-key granularity is handled inside the entry logic, not via separate
/// locks). The critical section is ~O(1) hashmap operations — should never
/// block for more than a few microseconds.
pub fn check_contact_rate_limit(key: &str) -> bool {
    let mut limits = CONTACT_LIMITS.lock().expect("rate limit mutex poisoned");

    let now = Instant::now();
    let entry = limits.entry(key.to_string()).or_insert(RateLimitEntry {
        count: 0,
        window_start: now,
    });

    if now.duration_since(entry.window_start) > WINDOW_DURATION {
        entry.count = 0;
        entry.window_start = now;
    }

    if entry.count >= MAX_REQUESTS {
        return false;
    }

    entry.count += 1;
    true
}

#[cfg(test)]
mod tests {
    use std::thread;

    use super::*;

    #[test]
    fn allows_first_request_for_new_key() {
        assert!(check_contact_rate_limit("allows-first-key"));
    }

    #[test]
    fn allows_up_to_five_consecutive_requests() {
        let key = "allows-five-explicit-key";

        assert!(check_contact_rate_limit(key));
        assert!(check_contact_rate_limit(key));
        assert!(check_contact_rate_limit(key));
        assert!(check_contact_rate_limit(key));
        assert!(check_contact_rate_limit(key));
        assert!(!check_contact_rate_limit(key));
    }

    #[test]
    fn rejects_sixth_request_within_window() {
        let key = "rejects-sixth-key";

        assert!(check_contact_rate_limit(key));
        assert!(check_contact_rate_limit(key));
        assert!(check_contact_rate_limit(key));
        assert!(check_contact_rate_limit(key));
        assert!(check_contact_rate_limit(key));
        assert!(!check_contact_rate_limit(key));
    }

    #[test]
    fn reopens_window_after_expiry() {
        let key = "reopens-window-key";

        assert!(check_contact_rate_limit(key));
        assert!(check_contact_rate_limit(key));
        assert!(check_contact_rate_limit(key));
        assert!(check_contact_rate_limit(key));
        assert!(check_contact_rate_limit(key));
        assert!(!check_contact_rate_limit(key));

        {
            let mut limits = CONTACT_LIMITS.lock().unwrap();
            limits.insert(
                key.to_string(),
                RateLimitEntry {
                    count: 5,
                    window_start: Instant::now() - Duration::from_secs(301),
                },
            );
        }

        assert!(check_contact_rate_limit(key));
        assert!(check_contact_rate_limit(key));
        assert!(check_contact_rate_limit(key));
        assert!(check_contact_rate_limit(key));
        assert!(check_contact_rate_limit(key));
        assert!(!check_contact_rate_limit(key));
    }

    #[test]
    fn isolates_rate_limits_per_key() {
        let key_a = "isolates-key-a";
        let key_b = "isolates-key-b";

        assert!(check_contact_rate_limit(key_a));
        assert!(check_contact_rate_limit(key_a));
        assert!(check_contact_rate_limit(key_a));
        assert!(check_contact_rate_limit(key_a));
        assert!(check_contact_rate_limit(key_a));
        assert!(!check_contact_rate_limit(key_a));

        assert!(check_contact_rate_limit(key_b));
    }

    #[test]
    /// Exception: thread spawn loop is unavoidable — the behavior under test
    /// IS concurrent access to the Mutex. A Vec of handles and sequential
    /// joins is the idiomatic safe concurrency pattern in std Rust.
    fn handles_concurrent_access_safely() {
        let key = "handles-concurrent-key";

        let mut handles = Vec::new();
        for _ in 0..10 {
            handles.push(thread::spawn(move || {
                check_contact_rate_limit(key);
            }));
        }
        for handle in handles {
            handle.join().unwrap();
        }
    }
}
