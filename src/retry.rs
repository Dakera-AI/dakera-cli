//! Exponential-backoff retry helper for transient network errors.
//!
//! Retries up to 3 times with delays of 100ms → 500ms → 2s.
//! Errors classified as 4xx (client errors) are never retried.

use std::time::Duration;

use tokio::time::sleep;

/// Retry delays: 100ms, 500ms, 2s.
const DELAYS: &[Duration] = &[
    Duration::from_millis(100),
    Duration::from_millis(500),
    Duration::from_millis(2_000),
];

/// Run `f` up to 4 times (1 initial attempt + 3 retries).
///
/// Client errors (4xx) are returned immediately without retry.
/// All other errors trigger a backoff sleep before the next attempt.
pub async fn with_backoff<F, Fut, T>(mut f: F) -> anyhow::Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<T>>,
{
    let mut attempt = 0;
    loop {
        match f().await {
            Ok(v) => return Ok(v),
            Err(e) => {
                if is_non_retryable(&e) {
                    return Err(e);
                }
                if attempt >= DELAYS.len() {
                    return Err(e);
                }
                let delay = DELAYS[attempt];
                tracing::warn!(
                    "Request failed (attempt {}), retrying in {:?}: {}",
                    attempt + 1,
                    delay,
                    e
                );
                sleep(delay).await;
                attempt += 1;
            }
        }
    }
}

/// Returns true for errors that should NOT be retried (client errors, invalid input).
fn is_non_retryable(err: &anyhow::Error) -> bool {
    let msg = err.to_string().to_lowercase();
    // 4xx HTTP status codes — retrying won't help
    msg.contains(" 400 ") || msg.contains(" 401 ") || msg.contains(" 403 ")
        || msg.contains(" 404 ") || msg.contains(" 409 ") || msg.contains(" 422 ")
        || msg.contains("bad request") || msg.contains("unauthorized")
        || msg.contains("forbidden") || msg.contains("not found")
        || msg.contains("unprocessable") || msg.contains("conflict")
        // Local input errors — also not retryable
        || msg.contains("failed to parse") || msg.contains("invalid input")
        || msg.contains("failed to read file")
}

#[cfg(test)]
mod tests {
    use std::sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    };

    use super::*;

    #[tokio::test]
    async fn succeeds_on_first_try() {
        let count = Arc::new(AtomicU32::new(0));
        let c = count.clone();
        let result: anyhow::Result<u32> = with_backoff(|| {
            let c = c.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                Ok(42)
            }
        })
        .await;
        assert_eq!(result.unwrap(), 42);
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn retries_on_connection_error() {
        let count = Arc::new(AtomicU32::new(0));
        let c = count.clone();
        let result: anyhow::Result<u32> = with_backoff(|| {
            let c = c.clone();
            async move {
                let n = c.fetch_add(1, Ordering::SeqCst) + 1;
                if n < 3 {
                    Err(anyhow::anyhow!("connection refused"))
                } else {
                    Ok(n)
                }
            }
        })
        .await;
        assert!(result.is_ok());
        assert_eq!(count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn does_not_retry_404() {
        let count = Arc::new(AtomicU32::new(0));
        let c = count.clone();
        let result: anyhow::Result<u32> = with_backoff(|| {
            let c = c.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                Err(anyhow::anyhow!("namespace not found 404"))
            }
        })
        .await;
        assert!(result.is_err());
        assert_eq!(count.load(Ordering::SeqCst), 1, "404 should not be retried");
    }

    #[tokio::test]
    async fn does_not_retry_401() {
        let count = Arc::new(AtomicU32::new(0));
        let c = count.clone();
        let _result: anyhow::Result<u32> = with_backoff(|| {
            let c = c.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                Err(anyhow::anyhow!("401 Unauthorized"))
            }
        })
        .await;
        assert_eq!(count.load(Ordering::SeqCst), 1, "401 should not be retried");
    }

    #[tokio::test]
    async fn exhausts_retries_and_returns_error() {
        let count = Arc::new(AtomicU32::new(0));
        let c = count.clone();
        let result: anyhow::Result<u32> = with_backoff(|| {
            let c = c.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                Err(anyhow::anyhow!("tcp connect error"))
            }
        })
        .await;
        assert!(result.is_err());
        // 1 initial + 3 retries = 4 total attempts
        assert_eq!(count.load(Ordering::SeqCst), 4);
    }

    #[test]
    fn non_retryable_detects_4xx() {
        assert!(is_non_retryable(&anyhow::anyhow!("error 404 not found")));
        assert!(is_non_retryable(&anyhow::anyhow!("HTTP 401 Unauthorized")));
        assert!(is_non_retryable(&anyhow::anyhow!("bad request")));
        assert!(!is_non_retryable(&anyhow::anyhow!("connection refused")));
        assert!(!is_non_retryable(&anyhow::anyhow!("tcp connect error")));
    }
}
