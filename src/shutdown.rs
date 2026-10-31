// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Structured async shutdown for long-running sharecli tasks (C00 L4).
//!
//! Root [`CancellationToken`] fans out to background tasks; HTTP serve uses
//! `axum::serve` graceful shutdown. See `docs/ops/async-shutdown.md`.

use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use tracing::info;

/// Wait until the shutdown watch channel is set to `true`.
pub async fn wait_for_shutdown_flag(mut rx: watch::Receiver<bool>) {
    loop {
        if *rx.borrow() {
            return;
        }
        if rx.changed().await.is_err() {
            return;
        }
        if *rx.borrow() {
            return;
        }
    }
}

/// Drive graceful HTTP shutdown: SIGINT (Ctrl-C), thermal critical, or external cancel.
pub async fn serve_shutdown_signal(cancel: CancellationToken, shutdown_rx: watch::Receiver<bool>) {
    let child = cancel.child_token();
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("sharecli serve: shutdown requested (Ctrl-C)");
            println!("sharecli serve shutting down (Ctrl-C)");
        }
        _ = wait_for_shutdown_flag(shutdown_rx) => {
            info!("sharecli serve: shutdown requested (thermal critical)");
            println!("sharecli serve shutting down (thermal critical)");
        }
        _ = child.cancelled() => {
            info!("sharecli serve: shutdown requested (cancellation token)");
        }
    }
    cancel.cancel();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn shutdown_flag_wakes_waiter() {
        let (tx, rx) = watch::channel(false);
        let waiter = tokio::spawn(async move { wait_for_shutdown_flag(rx).await });
        tx.send(true).unwrap();
        waiter.await.unwrap();
    }

    #[tokio::test]
    async fn serve_shutdown_signal_cancels_root_token() {
        let cancel = CancellationToken::new();
        let child = cancel.child_token();
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let signal = tokio::spawn(serve_shutdown_signal(cancel.clone(), shutdown_rx));
        shutdown_tx.send(true).unwrap();
        signal.await.unwrap();
        assert!(child.is_cancelled());
    }
}
