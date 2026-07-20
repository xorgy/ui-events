// Copyright 2026 the UI Events Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Shared XKB keysym and keymap mapping for the [`ui-events`] Wayland and X11
//! adapters.
//!
//! [`mapping`] converts evdev scancodes, XKB keysyms, and modifier flags into
//! [`ui-events`] keyboard types. It is always compiled and does no FFI.
//! [`keymap`] wraps an `xkbcommon` keymap and state ([`XkbKeymapState`]) for
//! logical keys, text, and modifiers. It is behind the `xkb` feature and
//! links `libxkbcommon`.
//!
//! [`mapping`] uses evdev scancodes; [`keymap`] uses XKB keycodes (scancode
//! plus 8). Each adapter applies its own offset.
//!
//! [`XkbKeymapState`]: keymap::XkbKeymapState
//! [`ui-events`]: https://docs.rs/ui-events/

// LINEBENDER LINT SET - lib.rs - v3
// See https://linebender.org/wiki/canonical-lints/
// These lints shouldn't apply to examples or tests.
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
// These lints shouldn't apply to examples.
#![warn(clippy::print_stdout, clippy::print_stderr)]
// Targeting e.g. 32-bit means structs containing usize can give false positives for 64-bit.
#![cfg_attr(target_pointer_width = "64", warn(clippy::trivially_copy_pass_by_ref))]
// END LINEBENDER LINT SET
#![no_std]

#[cfg(feature = "xkb")]
extern crate std;

pub mod mapping;

#[cfg(feature = "xkb")]
pub mod keymap;
