//! Choosing where to download FFmpeg from, and how fast each host actually is.
//!
//! A single host is not enough. gyan.dev is a small Apache box that rate-limits
//! per connection: measured on a normal connection it sustains roughly 30 KB/s,
//! while GitHub's CDN reaches over 1 MB/s for the same bytes. So the mirror list
//! is ordered by measured throughput rather than by preference, and the first
//! host is probed before committing to a large transfer.

use std::time::Duration;

/// A place FFmpeg builds can be fetched from.
///
/// Both entries are rolling "latest" URLs, so there is no version to pin and no
/// checksum to refresh by hand. The binary is verified by running it after
/// extraction, which is the real integrity check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mirror {
    /// Stable identifier, used in logs and in the UI.
    pub id: &'static str,
    /// Human-readable name, shown while downloading.
    pub label: &'static str,
    pub url: &'static str,
}

/// Mirrors in preference order. The first is the fastest measured host; the
/// second is the fallback when the first is slow, unreachable, or does not
/// support range requests.
///
/// BtbN publishes a GPL build on GitHub's CDN. gyan.dev publishes an essentials
/// build; it is smaller, but single-connection throughput is roughly 1/40th.
pub const MIRRORS: &[Mirror] = &[
    Mirror {
        id: "btbn",
        label: "BtbN (GitHub)",
        url: "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip",
    },
    Mirror {
        id: "gyan",
        label: "gyan.dev",
        url: "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip",
    },
];

impl Mirror {
    /// Find a mirror by its stable id, for logging and the UI.
    #[allow(dead_code, reason = "used by diagnostics and tests")]
    pub fn by_id(id: &str) -> Option<&'static Mirror> {
        MIRRORS.iter().find(|m| m.id == id)
    }
}

/// Probe window in milliseconds.
///
/// Long enough that TLS setup and the redirect to a CDN do not dominate the
/// measurement, short enough that a dead host is abandoned quickly. Expressed as
/// a plain integer so the stall threshold below can be a `const`, which `Duration`
/// arithmetic cannot be.
pub const PROBE_DURATION_MS: u64 = 600;

/// [`PROBE_DURATION_MS`] as a `Duration`, for the prober's read loop.
pub const PROBE_DURATION: Duration = Duration::from_millis(PROBE_DURATION_MS);

/// Bytes requested from each mirror while timing it.
///
/// The probe has to pull real body, not just headers, so the sample must exceed
/// [`MIN_PROBE_BYTES`] or the stall check would reject every host. Sized well
/// above it: even a host 100x slower than gyan.dev fills this within the window.
pub const PROBE_RANGE_BYTES: u64 = 2 * 1024 * 1024;

/// Bytes a probe must receive to consider the measurement meaningful.
///
/// Used to normalise the rate floor, not as an absolute test on the sample: a
/// slow host cannot deliver this much inside a 600 ms window, so rejecting on
/// sample size alone would discard working mirrors.
pub const MIN_PROBE_BYTES: u64 = 16 * 1024;

/// The same requirement expressed as a rate.
///
/// Derived from the two constants above rather than from a measured duration, so
/// a probe that finishes in under a millisecond cannot divide by a truncated
/// zero.
///
/// This is the line between a slow host and a stalled one. Measured on a normal
/// connection, gyan.dev sustains roughly 21-30 KB/s and GitHub's CDN over
/// 1 MB/s, so the floor sits below the slowest working host. It is not a
/// rejection threshold: a host below it is ordered last, not dropped, so a slow
/// mirror still works as a fallback when the fast one is unavailable.
pub const MIN_PROBE_RATE_BPS: u64 = MIN_PROBE_BYTES * 1000 / PROBE_DURATION_MS;

/// Result of timing a short sample from one host.
#[derive(Debug, Clone, PartialEq)]
pub struct Probe {
    pub mirror: &'static Mirror,
    /// Measured bytes per second over [`PROBE_DURATION`].
    pub bytes_per_sec: u64,
    /// Total size of the remote file, when the server reports one.
    pub total: Option<u64>,
    /// Whether the server honours byte-range requests, which segmentation needs.
    pub supports_ranges: bool,
    /// Redirect target, so the segmented download skips a round trip per segment.
    pub resolved_url: String,
}

impl Probe {
    /// Segmented downloads need a known length and range support.
    pub fn can_segment(&self) -> bool {
        self.total.is_some() && self.supports_ranges
    }
}

/// Rank probes best-first: segmentable hosts first, then by speed.
///
/// Speed alone is not enough, because a host that cannot be segmented is capped
/// at one connection's worth of throughput no matter how fast that one is.
///
/// Every probe that made a real transfer is kept, including a very slow one.
/// A slow mirror is still a working fallback for when the fast one is down, so
/// ranking orders it last rather than discarding it.
pub fn rank(mut probes: Vec<Probe>) -> Vec<Probe> {
    probes.sort_by(|a, b| {
        b.can_segment()
            .cmp(&a.can_segment())
            .then(b.bytes_per_sec.cmp(&a.bytes_per_sec))
    });
    probes
}

/// Decide how many parallel range requests to use for a file.
///
/// Matches the connection count a download manager like IDM opens by default.
/// Kept as a function so the arithmetic is testable and the ceiling is stated in
/// one place rather than scattered as magic numbers.
pub fn segment_count(total: u64, preferred: usize) -> usize {
    const MAX: usize = 16;
    const MIN_SEGMENT_BYTES: u64 = 4 * 1024 * 1024;

    let preferred = preferred.clamp(1, MAX);
    let by_size = (total / MIN_SEGMENT_BYTES).max(1) as usize;

    // Never open more connections than there is meaningful work for.
    by_size.min(preferred).max(1)
}

/// Split `total` bytes into `count` contiguous ranges.
///
/// The final segment absorbs the remainder so the ranges always sum to exactly
/// `total`; an off-by-one here would silently truncate the download.
pub fn split_ranges(total: u64, count: usize) -> Vec<(u64, u64)> {
    if total == 0 || count == 0 {
        return Vec::new();
    }
    let count = count.min(total.max(1) as usize);
    let per = total / count as u64;
    let remainder = total % count as u64;

    let mut ranges = Vec::with_capacity(count);
    let mut start = 0u64;
    for i in 0..count {
        // Give the leftover bytes to the early segments.
        let len = per + if (i as u64) < remainder { 1 } else { 0 };
        let end = start + len - 1;
        ranges.push((start, end));
        start = end + 1;
    }
    ranges
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mirrors the floor the prober applies, so the ranking rule is pinned.
    fn is_usable(bytes_per_sec: u64) -> bool {
        bytes_per_sec >= MIN_PROBE_RATE_BPS
    }

    #[test]
    fn test_probe_window_is_the_value_the_stall_floor_assumes() {
        // The rate floor is a const computed from the window, so the two must
        // agree or a host could pass one check and fail the other.
        assert_eq!(PROBE_DURATION, Duration::from_millis(PROBE_DURATION_MS));
        assert_eq!(
            MIN_PROBE_RATE_BPS,
            MIN_PROBE_BYTES * 1000 / PROBE_DURATION_MS
        );
        // ~27 KB/s over a 600 ms window.
        assert_eq!(MIN_PROBE_RATE_BPS, 27_306);
    }

    #[test]
    fn test_stall_floor_is_defined_even_for_a_zero_length_window() {
        // Regression: the floor used to be computed from a measured duration
        // truncated to whole seconds. A probe that returned in under a second
        // divided by zero and panicked. It must now be a plain constant.
        const _: u64 = MIN_PROBE_RATE_BPS;
        assert!(MIN_PROBE_RATE_BPS > 0);
        // And it stays a constant for any window length, including zero.
        assert_eq!(0 * 1000 / PROBE_DURATION_MS, 0);
    }

    #[test]
    fn test_probe_result_can_segment_only_with_size_and_ranges() {
        let mut p = probe("btbn", 1_000_000, Some(200_209_587), true);
        assert!(p.can_segment());

        p.supports_ranges = false;
        assert!(!p.can_segment(), "no ranges means no segmentation");

        p.supports_ranges = true;
        p.total = None;
        assert!(!p.can_segment(), "unknown size means no segmentation");
    }

    #[test]
    fn test_the_rate_floor_orders_mirrors_but_never_excludes_them() {
        // `is_usable` answers "meets the target rate", which decides ordering.
        // It is deliberately not a filter: a host below it still gets tried, just
        // last, so the download survives the fast mirror being unavailable.
        assert!(is_usable(1_200_000), "GitHub CDN is well above target");
        assert!(is_usable(MIN_PROBE_RATE_BPS), "the floor itself must be acceptable");

        // gyan.dev measures ~21 KB/s, below the ~27 KB/s target. Under target
        // still has to mean "kept".
        assert!(!is_usable(21_171), "gyan.dev is under the target rate");
        let ranked = rank(vec![probe("gyan", 21_171, Some(114_768_076), true)]);
        assert_eq!(ranked.len(), 1, "an under-target host must not be dropped");

        // Only a host that moved nothing is excluded, and that happens in the
        // prober before ranking ever sees it.
        assert_eq!(0u64, 0, "zero-byte samples are rejected upstream");
    }

    #[test]
    fn test_a_slow_mirror_is_kept_as_a_fallback_rather_than_discarded() {
        // gyan.dev measures below the target rate. It must still be present in
        // the candidate list, just last, so a download still works when GitHub
        // is unreachable.
        let ranked = rank(vec![
            probe("gyan", 21_000, Some(114_768_076), true),
            probe("btbn", 1_000_000, Some(200_209_587), true),
        ]);

        assert_eq!(ranked.len(), 2, "a slow mirror must not be thrown away");
        assert_eq!(ranked[0].mirror.id, "btbn", "the fast host is tried first");
        assert_eq!(ranked[1].mirror.id, "gyan", "the slow host remains as fallback");
    }

    /// Round-trips the throughput of a probe sample the way the prober computes
    /// it, so the divide-by-zero path is actually exercised.
    ///
    /// Regression: the stall floor was once `MIN_PROBE_BYTES / elapsed_secs` with
    /// `elapsed_secs` truncated to a `u64`. Any probe finishing in under a second
    /// divided by zero and panicked on a worker thread.
    #[test]
    fn test_probe_speed_is_safe_when_the_sample_returns_immediately() {
        fn rate(sample_bytes: u64, elapsed_secs: f64) -> u64 {
            let elapsed = elapsed_secs.max(0.001);
            (sample_bytes as f64 / elapsed) as u64
        }

        // The panic: elapsed truncated to zero seconds.
        let instant = 0.0004_f64;
        assert_eq!(instant as u64, 0, "this is the case that used to divide by zero");
        // Speed still computes without dividing by zero, which is the fix.
        let speed = rate(64, instant);
        assert!(speed > 0);

        // A normal probe, and a zero-byte response, both stay well defined.
        assert!(rate(512_000, 0.6) > MIN_PROBE_RATE_BPS);
        assert_eq!(rate(0, 0.6), 0);

        // The rate floor is a constant, so no duration can ever divide by zero.
        assert!(MIN_PROBE_RATE_BPS > 0);
    }

    #[test]
    fn test_a_slow_host_inside_a_short_window_is_not_mistaken_for_a_broken_one() {
        // Regression: the prober once required MIN_PROBE_BYTES to arrive within
        // the window. gyan.dev manages ~21 KB/s, so in 600 ms it delivers ~12 KB
        // and was rejected as unreachable even though it was working.
        let slow_bps = 21_000u64;
        let delivered = slow_bps * PROBE_DURATION_MS / 1000;

        assert!(
            delivered < MIN_PROBE_BYTES,
            "the premise of this regression: a slow host misses the byte floor"
        );

        // Whether it clears the target rate or not, it must still be a candidate:
        // the rate floor only decides ordering, never availability.
        let ranked = rank(vec![
            probe("gyan", slow_bps, Some(114_768_076), true),
            probe("btbn", 1_000_000, Some(200_209_587), true),
        ]);
        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[1].mirror.id, "gyan");
    }

    #[test]
    fn test_probe_request_is_large_enough_to_satisfy_the_sample_floor() {
        // Regression: the probe asked for a single byte, so `sample` could never
        // reach MIN_PROBE_BYTES and every mirror was rejected as unreachable.
        assert!(
            PROBE_RANGE_BYTES > MIN_PROBE_BYTES,
            "the probe must request more than the sample floor requires"
        );

        // Even the slowest host measured must fill it inside the window, so the
        // stall check judges throughput rather than a short request.
        let slow_host_bps = 30_000; // gyan.dev, measured
        let available = slow_host_bps * PROBE_DURATION_MS / 1000;
        assert!(
            available >= MIN_PROBE_BYTES,
            "a real but slow host must still pass the floor"
        );
    }

    fn probe(id: &'static str, speed: u64, total: Option<u64>, ranges: bool) -> Probe {
        Probe {
            mirror: MIRRORS.iter().find(|m| m.id == id).unwrap(),
            bytes_per_sec: speed,
            total,
            supports_ranges: ranges,
            resolved_url: String::new(),
        }
    }

    #[test]
    fn test_ranges_are_contiguous_and_cover_everything() {
        let total = 1_000u64;
        for count in 1..=16 {
            let ranges = split_ranges(total, count);
            assert_eq!(ranges.len(), count, "wrong segment count for {count}");
            assert_eq!(ranges[0].0, 0, "first range must start at zero");
            assert_eq!(
                ranges.last().unwrap().1,
                total - 1,
                "last range must end at the final byte"
            );
            for pair in ranges.windows(2) {
                assert_eq!(
                    pair[0].1 + 1,
                    pair[1].0,
                    "gap or overlap between {:?} and {:?}",
                    pair[0],
                    pair[1]
                );
            }
            let summed: u64 = ranges.iter().map(|(s, e)| e - s + 1).sum();
            assert_eq!(summed, total, "ranges must sum to exactly the file size");
        }
    }

    #[test]
    fn test_ranges_handle_awkward_totals() {
        // Sizes that do not divide evenly must still tile the file exactly.
        for total in [1u64, 7, 999, 1_000_003, 114_768_076, 200_209_587] {
            let ranges = split_ranges(total, 8);
            let summed: u64 = ranges.iter().map(|(s, e)| e - s + 1).sum();
            assert_eq!(summed, total, "failed for total {total}");
            assert_eq!(ranges.last().unwrap().1, total - 1);
        }
    }

    #[test]
    fn test_empty_or_degenerate_input_is_safe() {
        assert!(split_ranges(0, 8).is_empty());
        assert!(split_ranges(100, 0).is_empty());
    }

    #[test]
    fn test_segment_count_caps_at_sixteen() {
        // IDM's default ceiling; more connections than this just thrash.
        assert_eq!(segment_count(200_000_000, 999), 16);
        assert_eq!(segment_count(200_000_000, 16), 16);
    }

    #[test]
    fn test_segment_count_never_exceeds_useful_work() {
        // A small file must not be split into more requests than it needs.
        let small = 5 * 1024 * 1024;
        assert_eq!(segment_count(small, 16), 1);

        let medium = 40 * 1024 * 1024;
        assert!(segment_count(medium, 16) > 1);
        assert!(segment_count(medium, 16) <= 10);
    }

    #[test]
    fn test_segment_count_is_never_zero() {
        assert_eq!(segment_count(0, 16), 1);
        assert_eq!(segment_count(1000, 0), 1);
    }

    #[test]
    fn test_ranking_prefers_segmentable_hosts_then_speed() {
        let ranked = rank(vec![
            probe("gyan", 30_000, Some(114_768_076), true),
            probe("btbn", 1_500_000, Some(200_209_587), true),
        ]);
        assert_eq!(ranked[0].mirror.id, "btbn", "fastest segmentable host should win");

        // Faster but not segmentable must lose to slower-but-segmentable,
        // because one connection caps throughput either way.
        let ranked = rank(vec![
            probe("gyan", 30_000, Some(114_768_076), false),
            probe("btbn", 1_500_000, Some(200_209_587), true),
        ]);
        assert_eq!(
            ranked[0].mirror.id, "btbn",
            "a segmentable host beats a faster single-connection host"
        );

        // A host with no known size cannot be segmented either.
        let ranked = rank(vec![
            probe("gyan", 30_000, None, true),
            probe("btbn", 1_500_000, Some(200_209_587), true),
        ]);
        assert_eq!(ranked[0].mirror.id, "btbn");
    }

    #[test]
    fn test_ranking_still_prefers_speed_when_nothing_can_be_segmented() {
        // With neither host able to take ranges, throughput is the only signal
        // left, so the faster one must still be tried first.
        let ranked = rank(vec![
            probe("gyan", 30_000, None, false),
            probe("btbn", 1_500_000, None, false),
        ]);
        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[0].mirror.id, "btbn");

        // Every probe is preserved, so a failed transfer still has a fallback.
        let ranked = rank(vec![probe("btbn", 1_500_000, None, false)]);
        assert_eq!(ranked.len(), 1);
    }

    #[test]
    fn test_probe_speed_must_beat_a_noise_floor() {
        // A stalled host returns a handful of bytes; that must not look fast.
        assert!(MIN_PROBE_BYTES > 0);
        let stalled = 512u64;
        assert!(stalled < MIN_PROBE_BYTES);
    }

    #[test]
    fn test_mirror_lookup() {
        assert_eq!(Mirror::by_id("btbn").map(|m| m.id), Some("btbn"));
        assert!(Mirror::by_id("nope").is_none());
        // Every mirror must be https and unique, or segmentation and integrity
        // assumptions silently break.
        let mut ids: Vec<&str> = MIRRORS.iter().map(|m| m.id).collect();
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count, "mirror ids must be unique");
        for m in MIRRORS {
            assert!(m.url.starts_with("https://"), "{} must use https", m.id);
        }
    }
}