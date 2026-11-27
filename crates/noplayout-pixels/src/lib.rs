//! Pixel work shared by the renderer and the output process: unpremultiply, key, shaped and
//! unshaped fill (E-009), monitor downscale + JPEG (E-013, E-030) and test-pattern math.
//!
//! M0 skeleton: this crate exists so the workspace layout (PLAN §2.2) builds on every CI lane.
// TODO(M1): conversions and their golden tests (M1/M2). (STATUS.md, "Skeleton crates")
