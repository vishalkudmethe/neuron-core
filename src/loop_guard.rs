//! Loop Guardian — detects and terminates repetitive/looping agent behavior.
//!
//! Maintains a sliding window of recent operation keys. When the same key
//! appears more than `threshold` times within `window_sec` seconds, the
//! guardian fires an alert and signals the caller to pause/halt.

use chrono::{DateTime, Utc};
use colored::Colorize;
use std::collections::VecDeque;
use tracing::warn;

// ─── Per-event record ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct EventRecord {
    key:  String,
    when: DateTime<Utc>,
}

// ─── LoopGuard ────────────────────────────────────────────────────────────────

/// Sliding-window loop detector.
pub struct LoopGuard {
    /// Sliding window of recent events (bounded).
    window:         VecDeque<EventRecord>,
    /// Window duration in seconds.
    window_sec:     i64,
    /// How many occurrences of the same key triggers a loop alert.
    threshold:      u32,
    /// Total loops detected across lifetime of this guard.
    total_detected: u32,
}

impl LoopGuard {
    pub fn new(window_sec: u64, threshold: u32) -> Self {
        Self {
            window:         VecDeque::with_capacity(256),
            window_sec:     window_sec as i64,
            threshold,
            total_detected: 0,
        }
    }

    /// Record an event. Returns `true` if a loop was detected (caller should halt/pause).
    pub fn record(&mut self, key: &str) -> bool {
        let now = Utc::now();

        // Evict events outside the sliding window
        while let Some(front) = self.window.front() {
            if (now - front.when).num_seconds() > self.window_sec {
                self.window.pop_front();
            } else {
                break;
            }
        }

        // Add new event
        self.window.push_back(EventRecord { key: key.to_string(), when: now });

        // Count occurrences of this key in current window
        let count = self.window.iter().filter(|e| e.key == key).count() as u32;

        if count >= self.threshold {
            self.total_detected += 1;
            self.emit_alert(key, count);
            return true;
        }

        false
    }

    /// Reset window after a pause (allows continuation after halting).
    pub fn reset(&mut self) {
        self.window.clear();
    }

    /// Summary of guard state.
    pub fn stats(&self) -> LoopGuardStats {
        LoopGuardStats {
            window_size:    self.window.len(),
            total_detected: self.total_detected,
            window_sec:     self.window_sec,
            threshold:      self.threshold,
        }
    }

    fn emit_alert(&self, key: &str, count: u32) {
        let banner = "═".repeat(60);
        println!("\n  {}", banner.bright_red());
        println!(
            "  {} LOOP GUARDIAN ALERT",
            "⚡⚡⚡".bright_red().bold()
        );
        println!("  {}", banner.bright_red());
        println!(
            "  Pattern detected {} times in {}s window:",
            count.to_string().bright_red().bold(),
            self.window_sec
        );
        println!("  Key: {}", key.yellow());
        println!(
            "  Action: {} — Pausing for 5 seconds …",
            "HALTING WATCHER".red().bold()
        );
        println!("  {}\n", banner.bright_red());

        warn!(
            "Loop guardian fired: key='{}' count={} (threshold={})",
            key, count, self.threshold
        );
    }
}

// ─── Stats ────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct LoopGuardStats {
    pub window_size:    usize,
    pub total_detected: u32,
    pub window_sec:     i64,
    pub threshold:      u32,
}
