//! Sliding-window statistics for the per-node packet analysis.

use std::collections::VecDeque;

use serde::Serialize;
use uuid::Uuid;

/// Recent probe results of one node; `None` samples are lost probes.
#[derive(Debug, Clone)]
pub struct NodeWindow {
    capacity: usize,
    samples: VecDeque<Option<u32>>,
    sent: u64,
    lost: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeStats {
    pub profile_id: Uuid,
    pub last: Option<u32>,
    pub avg: Option<f64>,
    pub min: Option<u32>,
    pub max: Option<u32>,
    /// Mean absolute difference between consecutive successful samples (RFC 3550 style jitter).
    pub jitter: Option<f64>,
    /// Lost probes in the window, percent.
    pub loss: f64,
    pub sent: u64,
    pub lost: u64,
    /// The window, oldest first, `-1` for lost probes (for sparklines).
    pub history: Vec<i64>,
}

impl NodeWindow {
    pub fn new(capacity: usize) -> Self {
        NodeWindow { capacity: capacity.max(2), samples: VecDeque::new(), sent: 0, lost: 0 }
    }

    pub fn resize(&mut self, capacity: usize) {
        self.capacity = capacity.max(2);
        while self.samples.len() > self.capacity {
            self.samples.pop_front();
        }
    }

    pub fn record(&mut self, sample: Option<u32>) {
        self.sent += 1;
        if sample.is_none() {
            self.lost += 1;
        }
        if self.samples.len() == self.capacity {
            self.samples.pop_front();
        }
        self.samples.push_back(sample);
    }

    pub fn stats(&self, profile_id: Uuid) -> NodeStats {
        let ok: Vec<u32> = self.samples.iter().flatten().copied().collect();
        let lost_in_window = self.samples.iter().filter(|s| s.is_none()).count();
        let jitter = (ok.len() >= 2).then(|| ok.windows(2).map(|w| (w[1] as f64 - w[0] as f64).abs()).sum::<f64>() / (ok.len() - 1) as f64);
        NodeStats {
            profile_id,
            last: self.samples.back().copied().flatten(),
            avg: (!ok.is_empty()).then(|| ok.iter().map(|&v| v as f64).sum::<f64>() / ok.len() as f64),
            min: ok.iter().min().copied(),
            max: ok.iter().max().copied(),
            jitter,
            loss: if self.samples.is_empty() { 0.0 } else { lost_in_window as f64 * 100.0 / self.samples.len() as f64 },
            sent: self.sent,
            lost: self.lost,
            history: self.samples.iter().map(|s| s.map_or(-1, i64::from)).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_window_stats() {
        let mut w = NodeWindow::new(4);
        for s in [Some(100), Some(120), None, Some(110), Some(130)] {
            w.record(s);
        }
        let s = w.stats(Uuid::nil());
        assert_eq!(s.history, vec![120, -1, 110, 130]);
        assert_eq!(s.last, Some(130));
        assert_eq!(s.min, Some(110));
        assert_eq!(s.max, Some(130));
        assert!((s.avg.unwrap() - 120.0).abs() < 1e-9);
        assert!((s.jitter.unwrap() - 15.0).abs() < 1e-9);
        assert!((s.loss - 25.0).abs() < 1e-9);
        assert_eq!((s.sent, s.lost), (5, 1));
    }
}
