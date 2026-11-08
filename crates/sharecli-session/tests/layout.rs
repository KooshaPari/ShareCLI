// SPDX-License-Identifier: Apache-2.0 OR MIT
// FR:003 — Session layout round-trip + validation tests
use std::time::{SystemTime, UNIX_EPOCH};

use sharecli_session::{LayoutAxis, LayoutNode, LayoutSnapshot, SessionStore};

fn snapshot() -> LayoutSnapshot {
    LayoutSnapshot {
        id: "daily".to_string(),
        terminal: "ghostty".to_string(),
        captured_at: "2026-07-31T08:00:00Z".to_string(),
        root: LayoutNode::Split {
            axis: LayoutAxis::Horizontal,
            ratio_millis: 500,
            children: vec![
                LayoutNode::Pane { surface_id: "ghostty:1".to_string() },
                LayoutNode::Pane { surface_id: "ghostty:2".to_string() },
            ],
        },
    }
}

#[test]
fn layout_snapshot_round_trips_across_store_reopen() {
    let suffix = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let path = std::env::temp_dir().join(format!("sharecli-layout-{suffix}.sqlite"));
    let expected = snapshot();
    let store = SessionStore::open(&path).unwrap();
    store.save_layout(&expected).unwrap();
    drop(store);

    let reopened = SessionStore::open(&path).unwrap();
    assert_eq!(reopened.get_layout("daily").unwrap(), Some(expected));
    let _ = std::fs::remove_file(path);
}

#[test]
fn invalid_split_ratio_is_rejected_before_persistence() {
    let store = SessionStore::open_memory().unwrap();
    let mut invalid = snapshot();
    invalid.root = LayoutNode::Split {
        axis: LayoutAxis::Vertical,
        ratio_millis: 0,
        children: vec![
            LayoutNode::Pane { surface_id: "ghostty:1".to_string() },
            LayoutNode::Pane { surface_id: "ghostty:2".to_string() },
        ],
    };

    assert!(store.save_layout(&invalid).unwrap_err().to_string().contains("ratio"));
    assert_eq!(store.get_layout("daily").unwrap(), None);
}

#[test]
fn duplicate_surface_is_rejected() {
    let store = SessionStore::open_memory().unwrap();
    let mut invalid = snapshot();
    invalid.root = LayoutNode::Split {
        axis: LayoutAxis::Horizontal,
        ratio_millis: 500,
        children: vec![
            LayoutNode::Pane { surface_id: "ghostty:1".to_string() },
            LayoutNode::Pane { surface_id: "ghostty:1".to_string() },
        ],
    };

    assert!(store.save_layout(&invalid).unwrap_err().to_string().contains("more than once"));
}
