//! Shared progress bar / spinner helpers built on [`indicatif`].

use std::time::Duration;
use indicatif::{ProgressBar, ProgressStyle};

/// Create a determinate progress bar with a header message.
pub fn progress_bar(total: u64, msg: impl Into<String>) -> ProgressBar {
    let pb = ProgressBar::new(total)
        .with_style(
            ProgressStyle::default_bar()
                .template(
                    "{msg}\n{spinner:.green} [{elapsed}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})",
                )
                .unwrap()
                .progress_chars("━╸─"),
        )
        .with_message(msg.into());
    
    pb.enable_steady_tick(Duration::from_millis(100));
    pb
}

/// Create an indeterminate spinner with a message.
pub fn progress_spinner(msg: impl Into<String>) -> ProgressBar {
    let pb = ProgressBar::new_spinner()
        .with_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} [{elapsed}] {msg}")
                .unwrap(),
        )
        .with_message(msg.into());
    
    pb.enable_steady_tick(Duration::from_millis(100));
    pb
}
