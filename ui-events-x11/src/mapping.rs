// Copyright 2026 the UI Events Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Plain-value conversions from XInput 2 into [`ui-events`] types.
//!
//! No `x11rb` dependency.
//! Keysym and keymap work live in [`ui-events-xkb`].
//!
//! [`ui-events-xkb`]: https://docs.rs/ui-events-xkb/
//! [`ui-events`]: https://docs.rs/ui-events/

use dpi::PhysicalPosition;
use ui_events::keyboard::{Code, Modifiers};
use ui_events::pointer::PointerButton;
use ui_events_xkb::mapping::{code_from_evdev_scancode, modifiers_from_active_mods};

/// X11 keycode minus this equals the underlying evdev scancode.
///
/// Keycodes `0`..`7` are reserved.
/// Assumes an XKB/evdev keymap (the usual modern Linux case).
const KEYCODE_OFFSET: u32 = 8;

// X11 real-modifier bits (X.h / default real_modifiers). XI2 `mods.effective`.
const SHIFT_MASK: u32 = 1 << 0;
const LOCK_MASK: u32 = 1 << 1;
const CONTROL_MASK: u32 = 1 << 2;
const MOD1_MASK: u32 = 1 << 3; // Alt
const MOD2_MASK: u32 = 1 << 4; // Num Lock
const MOD4_MASK: u32 = 1 << 6; // Super / Meta
const MOD5_MASK: u32 = 1 << 7; // ISO Level3 / AltGraph

/// Map an X11/XKB keycode to a physical [`Code`].
///
/// Subtracts the evdev offset and looks up in the shared table.
/// Reserved low keycodes map to [`Code::Unidentified`].
/// Assumes Linux evdev rules as used with `libxkbcommon`.
pub fn code_from_keycode(keycode: u32) -> Code {
    // Keycodes below the offset wrap to an unmapped scancode (`Unidentified`).
    code_from_evdev_scancode(keycode.wrapping_sub(KEYCODE_OFFSET))
}

/// `FP1616` pair to a physical-pixel position.
pub fn position_from_fp1616(x: i32, y: i32) -> PhysicalPosition<f64> {
    PhysicalPosition::new(fp1616_to_f64(x), fp1616_to_f64(y))
}

/// One `FP1616` (signed 16.16) value to `f64`.
pub fn fp1616_to_f64(value: i32) -> f64 {
    const FP1616_ONE: f64 = 65536.0;
    f64::from(value) / FP1616_ONE
}

/// Successive absolute pinch scales to a [`PointerGesture::Pinch`] fraction.
///
/// XInput scale is relative to gesture start (`1.0` at begin).
/// Pinch is incremental: `current / previous - 1.0`.
/// Non-finite or non-positive samples are ignored.
///
/// [`PointerGesture::Pinch`]: ui_events::pointer::PointerGesture::Pinch
pub fn pinch_scale_fraction(previous: f64, current: f64) -> f32 {
    let previous = if previous.is_finite() && previous > 0.0 {
        previous
    } else {
        1.0
    };
    let current = if current.is_finite() && current > 0.0 {
        current
    } else {
        previous
    };
    #[expect(
        clippy::cast_possible_truncation,
        reason = "pinch fractions are small; f32 is the PointerGesture component type"
    )]
    {
        (current / previous - 1.0) as f32
    }
}

/// Clockwise degrees to [`PointerGesture::Rotate`] radians.
///
/// Non-finite input becomes `0.0`.
///
/// [`PointerGesture::Rotate`]: ui_events::pointer::PointerGesture::Rotate
pub fn rotation_radians_from_degrees(degrees: f64) -> f32 {
    let degrees = if degrees.is_finite() { degrees } else { 0.0 };
    #[expect(
        clippy::cast_possible_truncation,
        reason = "rotation deltas are small; f32 is the PointerGesture component type"
    )]
    {
        degrees.to_radians() as f32
    }
}

/// XI2 effective-modifier mask to [`Modifiers`].
///
/// Uses default real-modifier assignment: Alt=Mod1, NumLock=Mod2, Super=Mod4,
/// AltGraph=Mod5.
/// Exotic remaps are not reinterpreted.
/// For pointer/touch (mask only); the keyboard reducer with `xkb` uses keymap state.
pub fn modifiers_from_xi_mask(effective: u32) -> Modifiers {
    modifiers_from_active_mods(
        effective & CONTROL_MASK != 0,
        effective & MOD1_MASK != 0,
        effective & SHIFT_MASK != 0,
        effective & MOD4_MASK != 0,
        effective & LOCK_MASK != 0,
        effective & MOD2_MASK != 0,
        effective & MOD5_MASK != 0,
    )
}

/// XI2 button number to a [`PointerButton`].
///
/// `1`/`2`/`3` are primary/auxiliary/secondary.
/// `4`..`7` are legacy wheel (not buttons; pointer reducer maps or drops them).
/// `8`/`9` are X1/X2.
/// `10`..`35` map to [`B7`]..[`B32`]; higher details have no slot in
/// [`PointerButtons`].
///
/// [`B7`]: PointerButton::B7
/// [`B32`]: PointerButton::B32
/// [`PointerButtons`]: ui_events::pointer::PointerButtons
pub fn pointer_button_from_detail(detail: u32) -> Option<PointerButton> {
    Some(match detail {
        1 => PointerButton::Primary,
        2 => PointerButton::Auxiliary,
        3 => PointerButton::Secondary,
        // 4-7: legacy wheel
        8 => PointerButton::X1,
        9 => PointerButton::X2,
        // 10..=35: B7..=B32
        10 => PointerButton::B7,
        11 => PointerButton::B8,
        12 => PointerButton::B9,
        13 => PointerButton::B10,
        14 => PointerButton::B11,
        15 => PointerButton::B12,
        16 => PointerButton::B13,
        17 => PointerButton::B14,
        18 => PointerButton::B15,
        19 => PointerButton::B16,
        20 => PointerButton::B17,
        21 => PointerButton::B18,
        22 => PointerButton::B19,
        23 => PointerButton::B20,
        24 => PointerButton::B21,
        25 => PointerButton::B22,
        26 => PointerButton::B23,
        27 => PointerButton::B24,
        28 => PointerButton::B25,
        29 => PointerButton::B26,
        30 => PointerButton::B27,
        31 => PointerButton::B28,
        32 => PointerButton::B29,
        33 => PointerButton::B30,
        34 => PointerButton::B31,
        35 => PointerButton::B32,
        _ => return None,
    })
}

/// Legacy wheel button (`4`..`7`) to a one-line scroll delta.
///
/// `4` up, `5` down, `6` left, `7` right.
/// Signs match winit/AppKit content-direction: positive `y` moves content down,
/// positive `x` moves content right.
/// Prefer scroll-class valuators when present; drop `POINTER_EMULATED` notches.
pub fn legacy_wheel_line_delta(detail: u32) -> Option<(f32, f32)> {
    match detail {
        4 => Some((0.0, 1.0)),
        5 => Some((0.0, -1.0)),
        6 => Some((1.0, 0.0)),
        7 => Some((-1.0, 0.0)),
        _ => None,
    }
}

/// `FP3232` (`integral` + `frac`) to `f64`.
///
/// Fraction is always added (works for negative two's-complement values).
pub fn fp3232_to_f64(integral: i32, frac: u32) -> f64 {
    const FP3232_FRAC: f64 = 4_294_967_296.0;
    f64::from(integral) + f64::from(frac) / FP3232_FRAC
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keycode_maps_through_evdev_offset() {
        assert_eq!(code_from_keycode(38), Code::KeyA);
        assert_eq!(code_from_keycode(9), Code::Escape);
        assert_eq!(code_from_keycode(0), Code::Unidentified);
        assert_eq!(code_from_keycode(7), Code::Unidentified);
        assert_eq!(code_from_keycode(8), Code::Unidentified);
    }

    #[test]
    fn fp1616_converts_to_physical_pixels() {
        let position = position_from_fp1616(100 * 65536 + 32768, 200 * 65536);
        assert_eq!(position.x, 100.5);
        assert_eq!(position.y, 200.0);
        let negative = position_from_fp1616(-65536, 0);
        assert_eq!(negative.x, -1.0);
        assert_eq!(negative.y, 0.0);
        assert_eq!(fp1616_to_f64(65536), 1.0);
    }

    #[test]
    fn pinch_scale_fraction_is_relative_to_previous() {
        assert!((pinch_scale_fraction(1.0, 1.1) - 0.1).abs() < 1e-6);
        assert!((pinch_scale_fraction(1.1, 1.21) - 0.1).abs() < 1e-5);
        assert!((pinch_scale_fraction(1.0, 0.5) + 0.5).abs() < 1e-6);
    }

    #[test]
    fn rotation_converts_degrees_to_radians() {
        assert!((rotation_radians_from_degrees(90.0) - core::f32::consts::FRAC_PI_2).abs() < 1e-6);
        assert_eq!(rotation_radians_from_degrees(f64::NAN), 0.0);
    }

    #[test]
    fn button_numbers_map_and_skip_legacy_scroll() {
        assert_eq!(pointer_button_from_detail(1), Some(PointerButton::Primary));
        assert_eq!(
            pointer_button_from_detail(2),
            Some(PointerButton::Auxiliary)
        );
        assert_eq!(
            pointer_button_from_detail(3),
            Some(PointerButton::Secondary)
        );
        for scroll in 4..=7 {
            assert_eq!(pointer_button_from_detail(scroll), None);
        }
        assert_eq!(pointer_button_from_detail(8), Some(PointerButton::X1));
        assert_eq!(pointer_button_from_detail(9), Some(PointerButton::X2));
        assert_eq!(pointer_button_from_detail(10), Some(PointerButton::B7));
        assert_eq!(pointer_button_from_detail(35), Some(PointerButton::B32));
        assert_eq!(pointer_button_from_detail(0), None);
        assert_eq!(pointer_button_from_detail(36), None);
    }

    #[test]
    fn legacy_wheel_buttons_become_line_deltas() {
        assert_eq!(legacy_wheel_line_delta(4), Some((0.0, 1.0)));
        assert_eq!(legacy_wheel_line_delta(5), Some((0.0, -1.0)));
        assert_eq!(legacy_wheel_line_delta(6), Some((1.0, 0.0)));
        assert_eq!(legacy_wheel_line_delta(7), Some((-1.0, 0.0)));
        assert_eq!(legacy_wheel_line_delta(1), None);
    }

    #[test]
    fn xi_mask_maps_standard_real_modifiers() {
        let shift_ctrl = modifiers_from_xi_mask(SHIFT_MASK | CONTROL_MASK);
        assert!(shift_ctrl.shift());
        assert!(shift_ctrl.ctrl());
        assert!(!shift_ctrl.alt());

        let locks = modifiers_from_xi_mask(LOCK_MASK | MOD2_MASK | MOD5_MASK);
        assert!(locks.contains(Modifiers::CAPS_LOCK));
        assert!(locks.contains(Modifiers::NUM_LOCK));
        assert!(locks.contains(Modifiers::ALT_GRAPH));

        let meta_alt = modifiers_from_xi_mask(MOD1_MASK | MOD4_MASK);
        assert!(meta_alt.alt());
        assert!(meta_alt.meta());
    }

    #[test]
    fn fp3232_converts_including_negatives() {
        assert_eq!(fp3232_to_f64(5, 0), 5.0);
        assert_eq!(fp3232_to_f64(3, 1 << 31), 3.5);
        assert_eq!(fp3232_to_f64(-1, 1 << 31), -0.5);
        assert_eq!(fp3232_to_f64(-2, 0), -2.0);
    }
}
