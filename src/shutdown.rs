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
        let (_shutdown_tx, shutdown_rx) = watch::channel(false);
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            serve_shutdown_signal(cancel.clone(), shutdown_rx),
        )
        .await;
        assert!(
            result.is_ok(),
            "serve_shutdown_signal should terminate when parent is pre-cancelled"
        );
        // Even after the function returns, child must be cancelled (it already was).
        assert!(child.is_cancelled());
    }
}
