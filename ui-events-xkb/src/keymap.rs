// Copyright 2026 the UI Events Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! An [`xkbcommon`] keymap and keyboard state, shared by the Unix backends.
//!
//! Wayland loads compiled keymap text with [`set_keymap_from_string`].
//! X11 can build one from `_XKB_RULES_NAMES` via [`set_keymap_from_names`].
//!
//! [`set_keymap_from_string`]: XkbKeymapState::set_keymap_from_string
//! [`set_keymap_from_names`]: XkbKeymapState::set_keymap_from_names

use std::string::String;

use ui_events::keyboard::{Code, Key, Location, Modifiers, NamedKey};
use xkbcommon::xkb;

use crate::mapping;

/// An XKB keymap and the keyboard state derived from it.
///
/// Until a keymap is loaded, [`resolve`](Self::resolve) returns the keymap-less
/// fallback and [`modifiers`](Self::modifiers) returns `None`.
pub struct XkbKeymapState {
    /// The XKB context used to compile keymaps.
    context: xkb::Context,
    /// Keyboard state for the current keymap, if any.
    state: Option<xkb::State>,
}

impl Default for XkbKeymapState {
    fn default() -> Self {
        Self {
            context: xkb::Context::new(xkb::CONTEXT_NO_FLAGS),
            state: None,
        }
    }
}

impl core::fmt::Debug for XkbKeymapState {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("XkbKeymapState")
            .field("keymap_loaded", &self.state.is_some())
            .finish_non_exhaustive()
    }
}

impl XkbKeymapState {
    /// Empty state: fresh XKB context, no keymap.
    pub fn new() -> Self {
        Self::default()
    }

    /// Compile a keymap from XKB v1 text.
    ///
    /// On failure the previous state is kept.
    /// Returns whether a keymap was installed.
    pub fn set_keymap_from_string(&mut self, keymap: &str) -> bool {
        let Some(keymap) = xkb::Keymap::new_from_string(
            &self.context,
            String::from(keymap),
            xkb::KEYMAP_FORMAT_TEXT_V1,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        ) else {
            return false;
        };
        self.state = Some(xkb::State::new(&keymap));
        true
    }

    /// Compile a keymap from RMLVO layout names.
    ///
    /// Empty fields use `libxkbcommon` defaults.
    /// On failure the previous state is kept.
    /// Returns whether a keymap was installed.
    pub fn set_keymap_from_names(
        &mut self,
        rules: &str,
        model: &str,
        layout: &str,
        variant: &str,
        options: Option<&str>,
    ) -> bool {
        let Some(keymap) = xkb::Keymap::new_from_names(
            &self.context,
            rules,
            model,
            layout,
            variant,
            options.map(String::from),
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        ) else {
            return false;
        };
        self.state = Some(xkb::State::new(&keymap));
        true
    }

    /// Apply serialized modifier and layout state.
    ///
    /// Wayland reports only the locked layout group (pass 0 for depressed and
    /// latched). XInput 2 events carry all three.
    /// No-op until a keymap is loaded.
    pub fn update_mask(
        &mut self,
        depressed_mods: u32,
        latched_mods: u32,
        locked_mods: u32,
        depressed_layout: u32,
        latched_layout: u32,
        locked_layout: u32,
    ) {
        if let Some(state) = self.state.as_mut() {
            state.update_mask(
                depressed_mods,
                latched_mods,
                locked_mods,
                depressed_layout,
                latched_layout,
                locked_layout,
            );
        }
    }

    /// The modifier set derived from the keymap state, or `None` if no keymap
    /// has been loaded yet.
    pub fn modifiers(&self) -> Option<Modifiers> {
        let state = self.state.as_ref()?;
        Some(mapping::modifiers_from_active_mods(
            state.mod_name_is_active(xkb::MOD_NAME_CTRL, xkb::STATE_MODS_EFFECTIVE),
            state.mod_name_is_active(xkb::MOD_NAME_ALT, xkb::STATE_MODS_EFFECTIVE),
            state.mod_name_is_active(xkb::MOD_NAME_SHIFT, xkb::STATE_MODS_EFFECTIVE),
            state.mod_name_is_active(xkb::MOD_NAME_LOGO, xkb::STATE_MODS_EFFECTIVE),
            state.mod_name_is_active(xkb::MOD_NAME_CAPS, xkb::STATE_MODS_EFFECTIVE),
            state.mod_name_is_active(xkb::MOD_NAME_NUM, xkb::STATE_MODS_EFFECTIVE),
            state.mod_name_is_active(xkb::MOD_NAME_ISO_LEVEL3_SHIFT, xkb::STATE_MODS_EFFECTIVE),
        ))
    }

    /// Resolve an XKB keycode into its logical [`Key`] and [`Location`].
    ///
    /// `keycode` is the evdev scancode plus 8.
    /// `code` is the physical [`Code`], used for [`Location`].
    /// Without a keymap: [`NamedKey::Unidentified`] at [`Location::Standard`].
    pub fn resolve(&self, keycode: u32, code: Code) -> (Key, Location) {
        match self.state.as_ref() {
            Some(state) => {
                let keycode = xkb::Keycode::new(keycode);
                let keysym = state.key_get_one_sym(keycode);
                let text = state.key_get_utf8(keycode);
                (
                    mapping::key_from_keysym(keysym.raw(), &text),
                    mapping::location_from_code(code),
                )
            }
            None => (Key::Named(NamedKey::Unidentified), Location::Standard),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEYCODE_A: u32 = 30 + 8;
    const KEYCODE_ENTER: u32 = 28 + 8;
    const KEYCODE_LEFTSHIFT: u32 = 42 + 8;

    fn us_state() -> Option<XkbKeymapState> {
        let mut state = XkbKeymapState::new();
        state
            .set_keymap_from_names("", "", "us", "", None)
            .then_some(state)
    }

    #[test]
    fn empty_state_has_no_keymap_and_resolves_to_fallback() {
        let state = XkbKeymapState::new();
        assert_eq!(state.modifiers(), None);
        assert_eq!(
            state.resolve(KEYCODE_A, Code::KeyA),
            (Key::Named(NamedKey::Unidentified), Location::Standard)
        );
    }

    #[test]
    fn names_keymap_resolves_typed_text() {
        let Some(state) = us_state() else {
            return;
        };
        let (key, location) = state.resolve(KEYCODE_A, Code::KeyA);
        assert_eq!(key, Key::Character("a".into()));
        assert_eq!(location, Location::Standard);
    }

    #[test]
    fn names_keymap_resolves_named_key_and_side_location() {
        let Some(state) = us_state() else {
            return;
        };
        let (enter, _) = state.resolve(KEYCODE_ENTER, Code::Enter);
        assert_eq!(enter, Key::Named(NamedKey::Enter));
        let (shift, location) = state.resolve(KEYCODE_LEFTSHIFT, Code::ShiftLeft);
        assert_eq!(shift, Key::Named(NamedKey::Shift));
        assert_eq!(location, Location::Left);
    }

    #[test]
    fn update_mask_activates_modifiers_from_keymap_state() {
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let Some(keymap) = xkb::Keymap::new_from_names(
            &context,
            "",
            "",
            "us",
            "",
            None,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        ) else {
            return;
        };
        let shift_index = keymap.mod_get_index(xkb::MOD_NAME_SHIFT);
        if shift_index == xkb::MOD_INVALID {
            return;
        }
        let mut state = XkbKeymapState::new();
        assert!(state.set_keymap_from_names("", "", "us", "", None));

        assert_eq!(state.modifiers(), Some(Modifiers::empty()));

        state.update_mask(1_u32 << shift_index, 0, 0, 0, 0, 0);
        assert!(state.modifiers().expect("keymap loaded").shift());
    }
}
