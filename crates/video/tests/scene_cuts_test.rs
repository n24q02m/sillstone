//! Tests for video scene cut detection: 0/1 frames, static scenes, and synthetic cut sequences.

use sillstone_h264::Frame;
use sillstone_video::{SceneCutParams, scene_cuts};

fn create_test_frame(luma_val: u8, width: u32, height: u32) -> Frame {
    let y = vec![luma_val; (width * height) as usize];
    let cb = vec![128u8; ((width / 2) * (height / 2)) as usize];
    let cr = vec![128u8; ((width / 2) * (height / 2)) as usize];
    Frame {
        width,
        height,
        y,
        cb,
        cr,
    }
}

#[test]
fn test_scene_cuts_zero_and_one_frame() {
    assert_eq!(scene_cuts(&[], SceneCutParams::default()), Vec::<usize>::new());

    let single = [create_test_frame(128, 64, 48)];
    assert_eq!(scene_cuts(&single, SceneCutParams::default()), Vec::<usize>::new());
}

#[test]
fn test_scene_cuts_static_scene() {
    let frames: Vec<Frame> = (0..20).map(|_| create_test_frame(100, 64, 48)).collect();
    assert_eq!(scene_cuts(&frames, SceneCutParams::default()), Vec::<usize>::new());
}

#[test]
fn test_scene_cuts_synthetic_hard_cuts() {
    // 5 frames dark (16), 5 frames bright (235), 5 frames mid (128)
    let mut frames = Vec::new();
    for _ in 0..5 {
        frames.push(create_test_frame(16, 64, 48));
    }
    for _ in 0..5 {
        frames.push(create_test_frame(235, 64, 48));
    }
    for _ in 0..5 {
        frames.push(create_test_frame(128, 64, 48));
    }

    let cuts = scene_cuts(&frames, SceneCutParams::default());
    assert_eq!(cuts, vec![5, 10], "Hard cuts must be detected at frames 5 and 10");
}
