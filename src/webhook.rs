//! Webhook signature verification and parameter handling.

use std::time::SystemTime;

use crate::errors::WebhookError;

/// Return the current Unix timestamp in seconds.
fn unix_timestamp_now() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("system clock is before Unix epoch")
        .as_secs() as i64
}

/// Validate webhook timestamp to prevent replay attacks.
///
/// Rejects webhooks that are too old or too far in the future based on
/// `max_age_seconds`. Returns the parsed timestamp on success.
pub(crate) fn validate_timestamp(
    timestamp: usize,
    max_offset_seconds: i64,
) -> Result<usize, WebhookError> {
    let current_timestamp = unix_timestamp_now();
    let age_seconds = current_timestamp - timestamp as i64;

    if age_seconds > max_offset_seconds {
        return Err(WebhookError::TooOld {
            age_seconds,
            max_seconds: max_offset_seconds,
        });
    }
    if age_seconds < -max_offset_seconds {
        return Err(WebhookError::FromFuture {
            offset_seconds: age_seconds.abs(),
            max_seconds: max_offset_seconds,
        });
    }

    Ok(timestamp)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAX_AGE: i64 = 300;

    mod validate_timestamp {
        use super::*;

        #[test]
        fn current_timestamp_is_valid() {
            let now = unix_timestamp_now() as usize;
            assert!(validate_timestamp(now, MAX_AGE).is_ok());
        }

        #[test]
        fn very_old_timestamp_is_rejected() {
            assert!(validate_timestamp(1_000_000_000, MAX_AGE).is_err());
        }

        #[test]
        fn far_future_timestamp_is_rejected() {
            assert!(validate_timestamp(9_999_999_999_999, MAX_AGE).is_err());
        }

        #[test]
        fn exactly_at_boundary_is_valid() {
            let now = unix_timestamp_now();
            let at_boundary = now - MAX_AGE;
            assert!(validate_timestamp(at_boundary as usize, MAX_AGE).is_ok());
        }

        #[test]
        fn one_second_beyond_boundary_is_rejected() {
            let now = unix_timestamp_now();
            let beyond_boundary = now - MAX_AGE - 1;
            assert!(validate_timestamp(beyond_boundary as usize, MAX_AGE).is_err());
        }

        #[test]
        fn near_future_is_valid() {
            let now = unix_timestamp_now();
            let near_future = now + 10;
            assert!(validate_timestamp(near_future as usize, MAX_AGE).is_ok());
        }

        #[test]
        fn far_future_is_rejected() {
            let now = unix_timestamp_now();
            let far_future = now + MAX_AGE + 1;
            assert!(validate_timestamp(far_future as usize, MAX_AGE).is_err());
        }
    }
}
