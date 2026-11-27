use crate::conformance::sample;
use crate::pixels::{H, Image, W, check_ramp, diff};
use serde_json::json;

#[test]
fn sample_prefers_defaults_then_enums_then_types() {
    let s = json!({"type": "object", "properties": {
        "team": {"type": "string", "enum": ["away", "home"], "default": "home"},
        "side": {"type": "string", "enum": ["l", "r"]},
        "points": {"type": "integer", "minimum": -3},
        "n": {"type": "number"},
        "on": {"type": "boolean"},
        "items": {"type": "array"},
        "label": {"type": "string"},
        "nested": {"type": "object", "properties": {"x": {"type": "integer", "default": 7}}}
    }});
    assert_eq!(
        sample(&s),
        json!({"team": "home", "side": "l", "points": -3, "n": 1, "on": false,
               "items": [], "label": "", "nested": {"x": 7}})
    );
}

fn blank() -> Image {
    Image(vec![0; W * H * 4])
}

#[test]
fn diff_ignores_colour_under_low_alpha_and_counts_the_rest() {
    let (a, mut b) = (blank(), blank());
    b.0[..4].copy_from_slice(&[255, 0, 0, 10]); // colour noise under alpha 10 is not a difference
    assert_eq!(diff(&a, &b, 2), (1, 10)); // but alpha 10 vs 0 is
    let mut c = blank();
    c.0[..4].copy_from_slice(&[200, 0, 0, 10]);
    assert_eq!(diff(&b, &c, 2), (0, 0));
    c.0[4..8].copy_from_slice(&[0, 0, 0, 255]);
    let mut d = blank();
    d.0[..4].copy_from_slice(&[200, 0, 0, 10]);
    d.0[4..8].copy_from_slice(&[2, 0, 0, 255]);
    assert_eq!(diff(&c, &d, 2), (0, 2));
}

#[test]
fn ramp_check_accepts_nominal_and_names_a_bad_patch() {
    let mut img = blank();
    for i in 0..16usize {
        let (x, y) = (64 + i * 80 + 32, 96);
        let a = (17 * i) as u8;
        img.0[(y * W + x) * 4..(y * W + x) * 4 + 4].copy_from_slice(&[255, 255, 255, a]);
    }
    assert!(check_ramp(&img).is_empty());
    let (x, y) = (64 + 5 * 80 + 32, 96);
    img.0[(y * W + x) * 4 + 3] = 87; // nominal 85
    let f = check_ramp(&img);
    assert_eq!(f.len(), 1, "{f:?}");
    assert!(f[0].contains("patch 5"));
}
