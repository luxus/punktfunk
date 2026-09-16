const PIPELINE: &str = include_str!("../src/native/stream/pipeline.rs");

#[test]
fn native_output_keeps_the_client_refresh() {
    let display_mode = PIPELINE
        .split_once("pub(super) fn display_mode_for")
        .expect("find display_mode_for")
        .1
        .split_once("\n}\n")
        .expect("find the end of display_mode_for")
        .0;

    assert!(
        !display_mode.contains("vdisplay_hz_mult"),
        "display_mode_for applies PUNKTFUNK_VDISPLAY_HZ_MULT, so the output is not the client's WxH@Hz"
    );
}
