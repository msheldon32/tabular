//! Progress reporting for long-running operations
//!
//! Operations can report progress via a callback, which the UI can use
//! to display status updates.
//!
//! # Example
//! ```ignore
//! let progress = app.start_progress("Sorting", row_count);
//! for (i, item) in items.iter().enumerate() {
//!     // do work...
//!     progress.set(i);
//! }
//! app.clear_progress();
//! ```

#![allow(dead_code)]

use std::sync::atomic::{AtomicUsize, AtomicBool, Ordering};
use std::sync::Arc;

/// Progress tracker for long-running operations
#[derive(Clone)]
pub struct Progress {
    current: Arc<AtomicUsize>,
    total: Arc<AtomicUsize>,
    cancelled: Arc<AtomicBool>,
}

impl Progress {
    pub fn new(total: usize) -> Self {
        Self {
            current: Arc::new(AtomicUsize::new(0)),
            total: Arc::new(AtomicUsize::new(total)),
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Update progress (thread-safe)
    #[inline]
    pub fn set(&self, current: usize) {
        self.current.store(current, Ordering::Relaxed);
    }

    /// Increment progress by 1 (thread-safe)
    #[inline]
    pub fn inc(&self) {
        self.current.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment progress by n (thread-safe)
    #[inline]
    pub fn inc_by(&self, n: usize) {
        self.current.fetch_add(n, Ordering::Relaxed);
    }

    /// Get current progress value
    #[inline]
    pub fn current(&self) -> usize {
        self.current.load(Ordering::Relaxed)
    }

    /// Get total value
    #[inline]
    pub fn total(&self) -> usize {
        self.total.load(Ordering::Relaxed)
    }

    /// Get progress as a percentage (0-100)
    pub fn percent(&self) -> usize {
        let total = self.total();
        if total == 0 {
            return 100;
        }
        (self.current() * 100) / total
    }

    /// Request cancellation (thread-safe)
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    /// Check if cancelled (thread-safe)
    #[inline]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    /// Format progress for display
    pub fn format(&self, operation: &str) -> String {
        let pct = self.percent();
        if pct >= 100 {
            format!("{}: done", operation)
        } else {
            format!("{}: {}%", operation, pct)
        }
    }
}

impl Default for Progress {
    fn default() -> Self {
        Self::new(0)
    }
}

/// Trait for operations that can report progress
pub trait ProgressReporter {
    fn report_progress(&mut self, current: usize, total: usize);
}

/// A no-op progress reporter for when progress isn't needed
pub struct NoProgress;

impl ProgressReporter for NoProgress {
    #[inline]
    fn report_progress(&mut self, _current: usize, _total: usize) {}
}

/// Progress reporter that updates a Progress instance
pub struct ProgressUpdater {
    progress: Progress,
}

impl ProgressUpdater {
    pub fn new(progress: Progress) -> Self {
        Self { progress }
    }
}

impl ProgressReporter for ProgressUpdater {
    #[inline]
    fn report_progress(&mut self, current: usize, _total: usize) {
        self.progress.set(current);
    }
}
