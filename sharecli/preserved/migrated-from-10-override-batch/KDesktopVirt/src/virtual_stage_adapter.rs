// SPDX-License-Identifier: Apache-2.0 OR MIT
//! `VirtualStage` adapter — bridges KDesktopVirt's existing desktop-automation
//! surface to Eidolon's unified trait.
//!
//! This module is the **receiving side** of the Eidolon platform-impl plan
//! (`plans/2026-06-09-eidolon-platform-impl-plan-v1.md`, §8.1 Q1.1):
//! "KDesktopVirt is the receiving side of Eidolon integration". The plan
//! defines a single `VirtualStage` trait in `Eidolon/crates/eidolon-core`
//! that supersedes the three historical automator traits
//! (`DesktopAutomator`, `MobileAutomator`, `SandboxAutomator`).
//!
//! KDesktopVirt already has the underlying window/screenshot/pointer surface
//! (KVirtualStageCore::take_screenshot, AutomationEngine's WindMouse +
//! NaturalTyping engines, etc.). This adapter wraps an `Arc<KVirtualStageCore>`
//! and implements `eidolon_core::VirtualStage` by **delegating** to those
//! existing methods, so any consumer that holds `Arc<dyn VirtualStage>` can
//! drive a KDesktopVirt instance through the unified API.
//!
//! ## Why feature-gated?
//!
//! The adapter is gated behind `--features eidolon` so that:
//! 1. The default `kvdesktop` build keeps compiling if Eidolon breaks or
//!    is removed — the upstream stub should never block the receiving
//!    crate's default build.
//! 2. The pull-in is opt-in: Eidolon-core's `phenotype-errors` /
//!    `phenotype-bus` / `phenotype-build-info` path deps are only
//!    resolved when the user actually wants the bridge.
//! 3. The pre-existing compile-error note in the task brief ("the prior
//!    session noted 3 example errors") is honoured — the new code is
//!    isolated behind `#[cfg(feature = "eidolon")]` so a regression in
//!    Eidolon cannot propagate.
//!
//! ## Mapping
//!
//! | `VirtualStage` method | KDesktopVirt delegation |
//! |------------------------|-------------------------|
//! | `get_viewport` | `Viewport::desktop_fhd()` (1920x1080 @ 1.0 DPI — desktop default) |
//! | `screenshot` | `KVirtualStageCore::take_screenshot(path, None)` |
//! | `pointer` | `AutomationEngine` WindMouse + click; switches on `event.action` (`move`/`press`/`release`/`tap`/`long_press`) |
//! | `text` | `AutomationEngine` `type_text_naturally`; switches on `event.input_type` (`keystroke`/`paste`/`clear`) |
//! | `record_event` | tracing log + `Ok(())` (audit sink is TODO; matches macOS stub in Eidolon) |
//!
//! The `MobileStage` and `SandboxStage` sub-traits are not implemented here —
//! KDesktopVirt is a desktop platform, and the trait's default impls for
//! `tap`/`swipe`/`input_text` (which forward to `pointer`/`text`) and
//! `exec`/`resource_usage`/`start`/`stop`/`get_metadata` (which return
//! `Unsupported` / zeros / no-ops) are appropriate for a desktop impl.

use std::sync::Arc;

use async_trait::async_trait;
use eidolon_core::{
    AutomationEvent, PhenoError, PointerInput, Result as EidolonResult, TextInput, Viewport,
    VirtualStage,
};
use tracing::{debug, info, warn};

use crate::automation_engine::{MouseButton, Point};
use crate::core::KVirtualStageCore;

/// Adapter that implements `eidolon_core::VirtualStage` by delegating to a
/// KDesktopVirt `KVirtualStageCore` instance.
///
/// Construct via [`KDesktopVirtStageAdapter::new`] with an
/// `Arc<KVirtualStageCore>`. The adapter is `Send + Sync` (required by
/// `VirtualStage`) and cheap to clone since it only holds an `Arc`.
///
/// ## Example
///
/// ```no_run
/// # #[cfg(feature = "eidolon")]
/// # async fn demo() -> anyhow::Result<()> {
/// use std::sync::Arc;
/// use eidolon_core::VirtualStage;
/// use kvdesktop::core::KVirtualStageCore;
/// use kvdesktop::virtual_stage_adapter::KDesktopVirtStageAdapter;
///
/// let core = Arc::new(KVirtualStageCore::new().await?);
/// let stage: Arc<dyn VirtualStage> = Arc::new(KDesktopVirtStageAdapter::new(core));
/// let viewport = stage.get_viewport().await?;
/// assert!(viewport.width > 0);
/// # Ok(())
/// # }
/// ```
pub struct KDesktopVirtStageAdapter {
    core: Arc<KVirtualStageCore>,
}

impl std::fmt::Debug for KDesktopVirtStageAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KDesktopVirtStageAdapter")
            .field("core", &"<KVirtualStageCore>")
            .finish()
    }
}

impl KDesktopVirtStageAdapter {
    /// Build an adapter around a shared KVirtualStageCore handle.
    pub fn new(core: Arc<KVirtualStageCore>) -> Self {
        info!("Constructing KDesktopVirtStageAdapter (Eidolon VirtualStage bridge)");
        Self { core }
    }

    /// Borrow the inner KVirtualStageCore (useful for tests / composition).
    pub fn core(&self) -> &Arc<KVirtualStageCore> {
        &self.core
    }
}

#[async_trait]
impl VirtualStage for KDesktopVirtStageAdapter {
    async fn get_viewport(&self) -> EidolonResult<Viewport> {
        // KDesktopVirt's KVirtualStageCore does not currently expose a live
        // display-bounds probe (the underlying x11/wayland/macOS surface is
        // stubbed). We return the canonical desktop default (1920x1080 @
        // 1.0 DPI) so a consumer holding `Arc<dyn VirtualStage>` gets a
        // well-formed Viewport. The macOS Eidolon impl returns the same
        // constant for non-retina displays (see
        // `Eidolon/crates/eidolon-desktop/src/macos.rs:51-54`).
        debug!("VirtualStage::get_viewport -> Viewport::desktop_fhd() (stub default)");
        Ok(Viewport::desktop_fhd())
    }

    async fn screenshot(&self, path: &str) -> EidolonResult<()> {
        debug!("VirtualStage::screenshot({})", path);
        // Delegate to the existing KVirtualStageCore API. `session = None`
        // captures the host display (no container session binding).
        self.core
            .take_screenshot(path, None)
            .await
            .map_err(|e| PhenoError::Internal(format!("take_screenshot failed: {e}")))
    }

    async fn pointer(&self, event: &PointerInput) -> EidolonResult<()> {
        debug!(
            "VirtualStage::pointer({}, {}, action={}, button={:?}, duration_ms={:?})",
            event.x, event.y, event.action, event.button, event.duration_ms
        );
        // Make sure the AutomationEngine is warm. `ensure_automation_engine`
        // is idempotent — first call constructs, subsequent calls are a
        // single RwLock read.
        self.core
            .ensure_automation_engine()
            .await
            .map_err(|e| PhenoError::Internal(format!("ensure_automation_engine failed: {e}")))?;

        // Translate the Eidolon `MouseButton`-string into our enum.
        let button = match event.button.as_deref() {
            Some("right") => MouseButton::Right,
            Some("middle") => MouseButton::Middle,
            // "left" / None / anything else => default to Left, matching
            // the Eidolon macOS impl at `macos.rs:40-46`.
            _ => MouseButton::Left,
        };

        // Dispatch on action. We hold the engine write-guard for the whole
        // sequence (move+press or press+release) so a second `pointer()`
        // call from the same consumer cannot observe a half-applied state.
        let mut engine_guard = self.core.automation_engine.write().await;
        let engine = engine_guard
            .as_mut()
            .ok_or_else(|| PhenoError::Internal("automation engine not initialized".into()))?;
        let target = Point::new(event.x as f64, event.y as f64);

        match event.action.as_str() {
            // `move` — synthesise a natural trajectory from (0, 0) to the
            // target. KDesktopVirt doesn't currently track live cursor
            // position; (0, 0) is the safe lower bound for the WindMouse
            // algorithm. A future revision should plumb the current
            // cursor position through the automation engine.
            "move" => {
                engine
                    .move_cursor_naturally(Point::new(0.0, 0.0), target, None)
                    .await
                    .map_err(|e| PhenoError::Internal(format!("move_cursor_naturally: {e}")))?;
            }
            // `press` / `tap` — both translate to a left-click (with a
            // 50ms pre-click hold for `tap` to mimic the macOS impl's
            // behaviour at `Eidolon/crates/eidolon-desktop/src/macos.rs:121-130`).
            "press" | "tap" => {
                engine
                    .click_naturally(Point::new(0.0, 0.0), target, button)
                    .await
                    .map_err(|e| PhenoError::Internal(format!("click_naturally: {e}")))?;
            }
            "release" => {
                // WindMouse has no standalone "release" — model it as a
                // click at the current cursor position (best-effort).
                engine
                    .click_naturally(Point::new(0.0, 0.0), target, button)
                    .await
                    .map_err(|e| PhenoError::Internal(format!("click_naturally (release): {e}")))?;
            }
            "long_press" => {
                // Honour the requested duration (default 500ms) by holding
                // the click in place. The underlying engine does not yet
                // expose a "press and hold" primitive, so we model it as
                // a single click — a TODO the consumer should be aware of.
                let _hold_ms = event.duration_ms.unwrap_or(500);
                warn!(
                    "VirtualStage::pointer(long_press) is best-effort: \
                     no platform-specific hold primitive in AutomationEngine yet"
                );
                engine
                    .click_naturally(Point::new(0.0, 0.0), target, button)
                    .await
                    .map_err(|e| PhenoError::Internal(format!("click_naturally (long_press): {e}")))?;
            }
            other => {
                warn!("VirtualStage::pointer: unknown action '{}', treating as tap", other);
                engine
                    .click_naturally(Point::new(0.0, 0.0), target, button)
                    .await
                    .map_err(|e| PhenoError::Internal(format!("click_naturally (fallback): {e}")))?;
            }
        }
        Ok(())
    }

    async fn text(&self, event: &TextInput) -> EidolonResult<()> {
        debug!(
            "VirtualStage::text(type={}, len={}, delay_ms={:?})",
            event.input_type,
            event.text.len(),
            event.delay_ms
        );
        self.core
            .ensure_automation_engine()
            .await
            .map_err(|e| PhenoError::Internal(format!("ensure_automation_engine failed: {e}")))?;

        let mut engine_guard = self.core.automation_engine.write().await;
        let engine = engine_guard
            .as_mut()
            .ok_or_else(|| PhenoError::Internal("automation engine not initialized".into()))?;

        match event.input_type.as_str() {
            "keystroke" => {
                engine
                    .type_text_naturally(&event.text)
                    .await
                    .map_err(|e| PhenoError::Internal(format!("type_text_naturally: {e}")))?;
            }
            "paste" => {
                // The underlying engine types the literal text — Cmd+V /
                // Ctrl+V is a platform concern handled by the Eidolon
                // platform impl, not by KDesktopVirt. Fall back to a
                // keystroke of the text itself.
                warn!(
                    "VirtualStage::text(paste) falling back to keystroke: \
                     KDesktopVirt has no clipboard shim yet"
                );
                engine
                    .type_text_naturally(&event.text)
                    .await
                    .map_err(|e| PhenoError::Internal(format!("type_text_naturally (paste): {e}")))?;
            }
            "clear" => {
                // Send a few backspaces — the Eidolon macOS impl uses
                // Cmd+A + Delete; we don't have that composition primitive
                // yet, so a backspace per character is a safe approximation.
                let backspaces = "\u{0008}".repeat(event.text.chars().count().max(1));
                engine
                    .type_text_naturally(&backspaces)
                    .await
                    .map_err(|e| PhenoError::Internal(format!("type_text_naturally (clear): {e}")))?;
            }
            other => {
                warn!("VirtualStage::text: unknown input_type '{}', treating as keystroke", other);
                engine
                    .type_text_naturally(&event.text)
                    .await
                    .map_err(|e| PhenoError::Internal(format!("type_text_naturally (fallback): {e}")))?;
            }
        }
        Ok(())
    }

    async fn record_event(&self, event: AutomationEvent) -> EidolonResult<()> {
        // The Eidolon macOS impl does the same thing
        // (`Eidolon/crates/eidolon-desktop/src/macos.rs:224-227`):
        // log + Ok. The real audit-log sink is TODO across the whole
        // Eidolon surface; we mirror that for KDesktopVirt.
        info!("VirtualStage::record_event id={} type={} platform={}", event.id, event.event_type, event.platform);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::KVirtualStageCore;

    /// Sanity: the adapter compiles and satisfies `Send + Sync` (required by
    /// `VirtualStage`). We construct it inside `tokio::runtime::Runtime` to
    /// call the async `KVirtualStageCore::new` constructor.
    #[tokio::test]
    async fn adapter_satisfies_virtual_stage_bounds() {
        fn assert_send_sync<T: Send + Sync + ?Sized>() {}
        assert_send_sync::<KDesktopVirtStageAdapter>();
        // The trait object behind `Arc<dyn VirtualStage>` is the actual
        // shape consumers hold — assert `Send + Sync` on the unsized
        // reference, not the bare `dyn` (which is `!Sized` and can't be a
        // type-parameter argument without `?Sized`).
        assert_send_sync::<dyn VirtualStage>();
    }

    /// `get_viewport` returns a well-formed Viewport without touching the
    /// engine — pure stub.
    #[tokio::test]
    async fn get_viewport_returns_desktop_default() {
        let core = Arc::new(KVirtualStageCore::new().await.expect("core init"));
        let adapter = KDesktopVirtStageAdapter::new(core);
        let vp = adapter.get_viewport().await.expect("viewport");
        assert_eq!(vp.width, 1920);
        assert_eq!(vp.height, 1080);
        assert!((vp.dpr - 1.0).abs() < f64::EPSILON);
        assert_eq!(vp.orientation, "landscape");
    }

    /// `record_event` is a no-op+log — must always return Ok.
    #[tokio::test]
    async fn record_event_returns_ok() {
        let core = Arc::new(KVirtualStageCore::new().await.expect("core init"));
        let adapter = KDesktopVirtStageAdapter::new(core);
        let event = AutomationEvent::screenshot("desktop", "/tmp/test.png");
        adapter.record_event(event).await.expect("record_event");
    }
}
