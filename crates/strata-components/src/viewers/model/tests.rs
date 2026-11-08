use std::f32::consts::{FRAC_PI_2, TAU};

use super::{CameraView, Encoding, Mesh, ModelFile, ModelKind, ModelState, Stage};

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

fn stl() -> ModelFile {
    ModelFile {
        name: "bracket.stl".into(),
        extension: "stl".into(),
        position: None,
        kind: ModelKind::Mesh,
    }
}

fn mesh(view: CameraView) -> Mesh {
    Mesh {
        view,
        encoding: Encoding::BinaryStl,
        triangles: 12_480,
    }
}

/// A mesh loaded at Front left.
fn loaded() -> ModelState {
    let mut st = ModelState::default();
    st.open(stl());
    st.image([512.0, 512.0], Some(mesh(CameraView::FrontLeft)));
    st
}

/// The render for the view a step asked for lands.
fn render(st: &mut ModelState) {
    let view = st.camera();
    st.image([512.0, 512.0], Some(mesh(view)));
}

#[test]
fn next_turns_clockwise_and_previous_the_other_way() {
    use CameraView::*;
    let mut v = FrontLeft;
    let mut seen = vec![];
    for _ in 0..4 {
        v = v.step(1);
        seen.push(v);
    }
    assert_eq!(seen, [BackLeft, BackRight, FrontRight, FrontLeft]);
    assert_eq!(FrontLeft.step(-1), FrontRight);
}

#[test]
fn the_indicator_puts_front_left_lower_left_and_back_right_upper_right() {
    // Screen space, y down: a positive sine is below the centre.
    let at = |v: CameraView| (v.angle().cos(), v.angle().sin());
    let (x, y) = at(CameraView::FrontLeft);
    assert!(x < 0.0 && y > 0.0);
    let (x, y) = at(CameraView::BackRight);
    assert!(x > 0.0 && y < 0.0);
    let (x, y) = at(CameraView::FrontRight);
    assert!(x > 0.0 && y > 0.0);
    let (x, y) = at(CameraView::BackLeft);
    assert!(x < 0.0 && y < 0.0);
}

#[test]
fn each_step_takes_the_shorter_turn_and_a_lap_keeps_going_round() {
    let mut st = loaded();
    let start = st.angle();
    st.step(-1).expect("steps");
    // Front left to Front right is a quarter turn, not three quarters the other way.
    assert!(
        near(st.angle() - start, -FRAC_PI_2),
        "{}",
        st.angle() - start
    );
    render(&mut st);
    let start = st.angle();
    for _ in 0..4 {
        st.step(1).expect("steps");
        render(&mut st);
    }
    // Four Nexts are a whole lap the same way: the unwrapped angle grows by a turn.
    assert!(near(st.angle() - start, TAU), "{}", st.angle() - start);
}

#[test]
fn a_press_before_the_first_picture_or_during_a_render_does_nothing() {
    let mut st = ModelState::default();
    st.open(stl());
    assert_eq!(st.step(1), None);
    assert_eq!(st.camera(), CameraView::FrontLeft);
    st.image([512.0, 512.0], Some(mesh(CameraView::FrontLeft)));
    assert_eq!(st.step(1), Some(CameraView::BackLeft));
    assert!(st.rendering());
    assert_eq!(st.step(1), None, "ignored while BackLeft renders");
    assert_eq!(st.camera(), CameraView::BackLeft);
}

#[test]
fn a_failed_render_keeps_the_picture_returns_the_camera_and_retries() {
    let mut st = loaded();
    st.step(1);
    st.progress(Stage::Rendering { triangles: 12_480 });
    st.view_failed("The sandboxed preview renderer failed");
    assert!(st.has_image() && !st.rendering());
    assert_eq!(st.camera(), CameraView::FrontLeft, "back to the view shown");
    assert_eq!(st.error(), Some("The sandboxed preview renderer failed"));
    // The button tries the same view again.
    assert_eq!(st.step(1), Some(CameraView::BackLeft));
    assert_eq!(st.error(), None);
}

#[test]
fn a_new_file_starts_over_at_front_left_and_loading() {
    let mut st = loaded();
    st.step(1);
    render(&mut st);
    st.step(1);
    assert_eq!(st.camera(), CameraView::BackRight);
    st.open(ModelFile {
        name: "gear.3mf".into(),
        extension: "3mf".into(),
        position: None,
        kind: ModelKind::Mesh,
    });
    assert_eq!(st.camera(), CameraView::FrontLeft);
    assert!(near(st.angle(), CameraView::FrontLeft.angle()));
    assert_eq!(st.busy(), Some("Loading preview…"));
    assert_eq!(st.format_badge(), "3MF");
    assert_eq!(st.triangle_badge(), None);
}

#[test]
fn the_badges_follow_the_load() {
    let mut st = ModelState::default();
    st.open(stl());
    assert_eq!(st.format_badge(), "STL");
    st.progress(Stage::Rendering { triangles: 1 });
    assert_eq!(st.triangle_badge().as_deref(), Some("1 triangle"));
    assert_eq!(st.busy(), Some("Rendering 1 triangle…"));
    st.image([512.0, 512.0], Some(mesh(CameraView::FrontLeft)));
    assert_eq!(st.format_badge(), "Binary STL");
    assert_eq!(st.triangle_badge().as_deref(), Some("12,480 triangles"));
    assert_eq!(st.busy(), None);
}

#[test]
fn an_embedded_picture_shows_alone_once_loaded() {
    let mut st = ModelState::default();
    st.open(ModelFile {
        name: "benchy.bgcode".into(),
        extension: "bgcode".into(),
        position: None,
        kind: ModelKind::Embedded,
    });
    assert!(!st.plain());
    assert_eq!(st.format_badge(), "BGCODE");
    st.progress(Stage::Thumbnail);
    assert_eq!(st.busy(), Some("Reading thumbnail…"));
    st.image([300.0, 300.0], None);
    assert!(st.plain());
    assert!(st.busy().is_none());
}
