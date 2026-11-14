use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, io::BufRead};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TimeBucket {
    pub start_at: DateTime<Utc>,
    pub request_count: u64,
    pub failed_count: u64,
    pub duration_count: u64,
    pub duration_sum_ms: f64,
    pub duration_max_ms: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BucketIngest {
    pub buckets: Vec<TimeBucket>,
    pub malformed_lines: u64,
    pub truncated: bool,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BucketError {
    #[error("bucket size must be greater than zero")]
    InvalidBucketSize,
    #[error("maximum bucket count must be greater than zero")]
    InvalidBucketLimit,
}

#[derive(Debug, Deserialize)]
struct K6Point {
    metric: String,
    data: K6PointData,
}

#[derive(Debug, Deserialize)]
struct K6PointData {
    time: DateTime<Utc>,
    value: f64,
}

pub fn ingest_jsonl<R: BufRead>(
    reader: R,
    bucket_seconds: u64,
    max_buckets: usize,
) -> Result<BucketIngest, BucketError> {
    if bucket_seconds == 0 {
        return Err(BucketError::InvalidBucketSize);
    }
    if max_buckets == 0 {
        return Err(BucketError::InvalidBucketLimit);
    }
    let mut buckets = BTreeMap::<i64, TimeBucket>::new();
    let mut malformed_lines = 0;
    let mut truncated = false;
    for line in reader.lines() {
        let line = match line {
            Ok(line) => line,
            Err(_) => {
                malformed_lines += 1;
                continue;
            }
        };
        let point: K6Point = match serde_json::from_str(&line) {
            Ok(point) => point,
            Err(_) => {
                malformed_lines += 1;
                continue;
            }
        };
        if !point.data.value.is_finite() || point.data.value < 0.0 {
            malformed_lines += 1;
            continue;
        }
        let epoch = point.data.time.timestamp();
        let bucket_start = epoch - epoch.rem_euclid(bucket_seconds as i64);
        if !buckets.contains_key(&bucket_start) && buckets.len() >= max_buckets {
            truncated = true;
            continue;
        }
        let start_at = DateTime::<Utc>::from_timestamp(bucket_start, 0)
            .expect("bucket timestamp is within chrono range");
        let bucket = buckets.entry(bucket_start).or_insert_with(|| TimeBucket {
            start_at,
            request_count: 0,
            failed_count: 0,
            duration_count: 0,
            duration_sum_ms: 0.0,
            duration_max_ms: None,
        });
        match point.metric.as_str() {
            "http_reqs" => bucket.request_count += 1,
            "http_req_failed" if point.data.value > 0.0 => bucket.failed_count += 1,
            "http_req_duration" => {
                bucket.duration_count += 1;
                bucket.duration_sum_ms += point.data.value;
                bucket.duration_max_ms =
                    Some(bucket.duration_max_ms.unwrap_or(0.0).max(point.data.value));
            }
            _ => {}
        }
    }
    Ok(BucketIngest {
        buckets: buckets.into_values().collect(),
        malformed_lines,
        truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn fixture() -> Cursor<Vec<u8>> {
        Cursor::new(
            br#"{"metric":"http_reqs","data":{"time":"2026-01-01T00:00:01Z","value":1}}
{"metric":"http_req_failed","data":{"time":"2026-01-01T00:00:02Z","value":1}}
{"metric":"http_req_duration","data":{"time":"2026-01-01T00:00:03Z","value":40}}
not-json
{"metric":"http_reqs","data":{"time":"2026-01-01T00:00:12Z","value":1}}
"#
            .to_vec(),
        )
    }

    #[test]
    fn parses_points_into_sorted_bounded_buckets() {
        let result = ingest_jsonl(fixture(), 10, 2).unwrap();
        assert_eq!(result.buckets.len(), 2);
        assert_eq!(result.buckets[0].request_count, 1);
        assert_eq!(result.buckets[0].failed_count, 1);
        assert_eq!(result.buckets[0].duration_sum_ms, 40.0);
        assert_eq!(result.malformed_lines, 1);
        assert!(!result.truncated);
    }

    #[test]
    fn caps_memory_and_marks_truncation() {
        let input = (0..3)
            .map(|second| format!(r#"{{"metric":"http_reqs","data":{{"time":"2026-01-01T00:00:{second:02}Z","value":1}}}}"#))
            .collect::<Vec<_>>()
            .join("\n");
        let result = ingest_jsonl(Cursor::new(input), 1, 2).unwrap();
        assert_eq!(result.buckets.len(), 2);
        assert!(result.truncated);
    }

    #[test]
    fn rejects_unbounded_configuration() {
        assert_eq!(
            ingest_jsonl(Cursor::new(Vec::<u8>::new()), 0, 1),
            Err(BucketError::InvalidBucketSize)
        );
        assert_eq!(
            ingest_jsonl(Cursor::new(Vec::<u8>::new()), 1, 0),
            Err(BucketError::InvalidBucketLimit)
        );
    }
}
