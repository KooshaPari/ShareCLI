// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Structured async shutdown for long-running sharecli tasks (C00 L4).
//!
//! Root [`CancellationToken`] fans out to background tasks; HTTP serve uses
//! `axum::serve` graceful shutdown. See `docs/ops/async-shutdown.md`.

use std::sync::Arc;

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
///
/// Every trigger also publishes `true` on the shared shutdown flag, so the
/// `/readyz` route reports 503 during the drain window instead of continuing to
/// advertise readiness. Ctrl-C previously left the flag `false`.
pub async fn serve_shutdown_signal(
    cancel: CancellationToken,
    shutdown_tx: Arc<watch::Sender<bool>>,
) {
    let child = cancel.child_token();
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("sharecli serve: shutdown requested (Ctrl-C)");
            println!("sharecli serve shutting down (Ctrl-C)");
        }
        _ = wait_for_shutdown_flag(shutdown_tx.subscribe()) => {
            info!("sharecli serve: shutdown requested (thermal critical)");
            println!("sharecli serve shutting down (thermal critical)");
        }
        _ = child.cancelled() => {
            info!("sharecli serve: shutdown requested (cancellation token)");
        }
    }
    // Mark not-ready before draining in-flight requests. `send` only fails when
    // every receiver is gone, which is not an error for shutdown.
    let _ = shutdown_tx.send(true);
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
        let shutdown_tx = Arc::new(shutdown_tx);
        let signal = tokio::spawn(serve_shutdown_signal(cancel.clone(), Arc::clone(&shutdown_tx)));
        shutdown_tx.send(true).unwrap();
        signal.await.unwrap();
        assert!(child.is_cancelled());
        assert!(*shutdown_rx.borrow(), "shutdown flag MUST be published");
    }

    #[tokio::test]
    async fn wait_for_shutdown_flag_returns_immediately_when_flag_already_set() {
        let (tx, rx) = watch::channel(true);
        drop(tx);
        // The flag is true on the first borrow; the function should return
        // without ever calling `rx.changed().await`.
        let started = std::time::Instant::now();
        wait_for_shutdown_flag(rx).await;
        let elapsed = started.elapsed();
        assert!(
            elapsed < std::time::Duration::from_millis(100),
            "expected near-instant return, took {elapsed:?}"
        );
    }

    #[tokio::test]
    async fn wait_for_shutdown_flag_terminates_when_sender_dropped() {
        let (tx, rx) = watch::channel(false);
        // Drop the sender so `rx.changed().await` returns Err and the loop exits.
        drop(tx);
        let started = std::time::Instant::now();
        wait_for_shutdown_flag(rx).await;
        let elapsed = started.elapsed();
        assert!(
            elapsed < std::time::Duration::from_millis(100),
            "expected near-instant return on dropped sender, took {elapsed:?}"
        );
    }

    #[tokio::test]
    async fn serve_shutdown_signal_terminates_via_pre_cancelled_parent_token() {
        // Pre-cancelling the parent makes the `child.cancelled()` branch of
        // `tokio::select!` fire immediately on poll. The function should
        // exit without ever blocking on the watch flag or ctrl_c.
        let cancel = CancellationToken::new();
        let child = cancel.child_token();
        cancel.cancel();
        assert!(child.is_cancelled());
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let shutdown_tx = Arc::new(shutdown_tx);
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            serve_shutdown_signal(cancel.clone(), Arc::clone(&shutdown_tx)),
        )
        .await;
        assert!(
            result.is_ok(),
            "serve_shutdown_signal should terminate when parent is pre-cancelled"
        );
        // Even after the function returns, child must be cancelled (it already was).
        assert!(child.is_cancelled());
        // A non-signal shutdown path (external cancel) MUST mark /readyz not-ready
        // so probes stop routing traffic during the drain window.
        assert!(
            *shutdown_rx.borrow(),
            "external cancellation MUST publish the shutdown flag read by /readyz"
        );
    }
}
