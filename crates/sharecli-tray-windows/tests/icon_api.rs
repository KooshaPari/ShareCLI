//! FR-007: the native tray uses the public tray-icon RGBA API on every platform.

#[test]
fn public_icon_api_accepts_rgba_pixels() {
    assert!(tray_icon::Icon::from_rgba(vec![16, 96, 200, 255], 1, 1).is_ok());
}

#[test]
#[cfg(target_os = "windows")]
fn public_icon_api_rejects_incorrect_dimensions() {
    assert!(matches!(
        tray_icon::Icon::from_rgba(vec![16, 96, 200, 255], 2, 1),
        Err(tray_icon::BadIcon::DimensionsVsPixelCount { .. })
    ));
}
