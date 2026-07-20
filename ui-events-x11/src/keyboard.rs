// Copyright 2026 the UI Events Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Reduces XInput 2 key events into [`KeyboardEvent`]s.
//!
//! Keep one [`KeyboardEventReducer`] per keyboard.
//! Feed each XInput 2 key event; press and release each yield one event.
//! Physical [`Code`] comes from the keycode.
//! Auto-repeat (`KEY_REPEAT`) is [`KeyState::Down`] with `repeat` set.
//!
//! Without a keymap, the logical key is
//! [`Key::Named`]\([`NamedKey::Unidentified`]) at [`Location::Standard`], and
//! modifiers come from held physical modifier keys.
//! With `xkb`, install a keymap via
//! [`KeyboardEventReducer::set_keymap_from_names`] or
//! [`KeyboardEventReducer::set_keymap_from_string`] for logical keys, text, and
//! full modifier state (including locks and `AltGraph`).
//!
//! # Focus loss
//!
//! Releases outside the focused window are not delivered.
//! [`reduce`](KeyboardEventReducer::reduce) synthesizes [`KeyState::Up`] for
//! every held key on `XinputFocusOut`.
//! Call [`focus_lost`](KeyboardEventReducer::focus_lost) for any other focus-loss path.
//!
//! [`Key::Named`]: ui_events::keyboard::Key::Named
//! [`NamedKey::Unidentified`]: ui_events::keyboard::NamedKey::Unidentified
//! [`Location::Standard`]: ui_events::keyboard::Location::Standard

use alloc::vec::Vec;

use ui_events::keyboard::{Code, KeyState, KeyboardEvent, Modifiers};
#[cfg(not(feature = "xkb"))]
use ui_events::keyboard::{Key, Location, NamedKey};
#[cfg(feature = "xkb")]
use ui_events_xkb::keymap::XkbKeymapState;
use ui_events_xkb::mapping::modifiers_from_bools;
use x11rb::protocol::Event;
use x11rb::protocol::xinput::{KeyEventFlags, KeyPressEvent, KeyReleaseEvent};

use crate::mapping;

/// Reduces an XInput 2 key event stream into [`KeyboardEvent`]s.
///
/// One reducer per keyboard.
/// See the [module documentation](self).
#[derive(Debug, Default)]
pub struct KeyboardEventReducer {
    /// Held X11 keycodes (cleared by [`focus_lost`](Self::focus_lost)).
    pressed: Vec<u32>,
    /// XKB state when the `xkb` feature is enabled.
    #[cfg(feature = "xkb")]
    xkb: XkbKeymapState,
}

impl KeyboardEventReducer {
    /// Reduce a decoded [`Event`] into zero or more [`KeyboardEvent`]s.
    ///
    /// `KeyPress` / `KeyRelease` each yield one event.
    /// `XinputFocusOut` releases every held key (see [focus loss](self#focus-loss)).
    /// Other events return empty.
    pub fn reduce(&mut self, event: &Event) -> Vec<KeyboardEvent> {
        match event {
            Event::XinputKeyPress(event) => alloc::vec![self.reduce_key_press(event)],
            Event::XinputKeyRelease(event) => alloc::vec![self.reduce_key_release(event)],
            Event::XinputFocusOut(_) => self.focus_lost(),
            _ => Vec::new(),
        }
    }

    fn reduce_key_press(&mut self, event: &KeyPressEvent) -> KeyboardEvent {
        self.key(event, KeyState::Down)
    }

    fn reduce_key_release(&mut self, event: &KeyReleaseEvent) -> KeyboardEvent {
        self.key(event, KeyState::Up)
    }

    /// Synthesize [`KeyState::Up`] for every held key and clear tracking.
    ///
    /// Use when focus leaves and X11 will not deliver the matching releases
    /// (or rely on [`Self::reduce`] for `XinputFocusOut`).
    /// Releases follow press order; each event's modifiers exclude that key.
    pub fn focus_lost(&mut self) -> Vec<KeyboardEvent> {
        let held = core::mem::take(&mut self.pressed);
        let mut events = Vec::with_capacity(held.len());
        for (index, keycode) in held.iter().copied().enumerate() {
            self.pressed.clear();
            self.pressed.extend_from_slice(&held[index + 1..]);

            let code = mapping::code_from_keycode(keycode);
            #[cfg(feature = "xkb")]
            let (key, location) = self.xkb.resolve(keycode, code);
            #[cfg(not(feature = "xkb"))]
            let (key, location) = (Key::Named(NamedKey::Unidentified), Location::Standard);

            events.push(KeyboardEvent {
                state: KeyState::Up,
                key,
                code,
                location,
                modifiers: self.modifiers_from_pressed(),
                repeat: false,
                is_composing: false,
            });
        }
        debug_assert!(
            self.pressed.is_empty(),
            "focus_lost must leave no keys recorded as down"
        );
        #[cfg(feature = "xkb")]
        self.xkb.update_mask(0, 0, 0, 0, 0, 0);
        events
    }

    /// Install a keymap from RMLVO names (`_XKB_RULES_NAMES`).
    ///
    /// Empty fields use `libxkbcommon` defaults.
    /// Returns whether a keymap was installed.
    #[cfg(feature = "xkb")]
    pub fn set_keymap_from_names(
        &mut self,
        rules: &str,
        model: &str,
        layout: &str,
        variant: &str,
        options: Option<&str>,
    ) -> bool {
        self.xkb
            .set_keymap_from_names(rules, model, layout, variant, options)
    }

    /// Install a keymap from XKB v1 text.
    ///
    /// Returns whether a keymap was installed.
    #[cfg(feature = "xkb")]
    pub fn set_keymap_from_string(&mut self, keymap: &str) -> bool {
        self.xkb.set_keymap_from_string(keymap)
    }

    /// Current modifiers (same as stamped on emitted events).
    ///
    /// With `xkb`, includes lock state and `AltGraph`.
    /// Without a keymap, derived from held physical modifier keys.
    pub fn modifiers(&self) -> Modifiers {
        self.modifiers_from_pressed()
    }

    /// Modifiers from the keymap, or from held physical keys.
    fn modifiers_from_pressed(&self) -> Modifiers {
        #[cfg(feature = "xkb")]
        if let Some(modifiers) = self.xkb.modifiers() {
            return modifiers;
        }
        let (mut ctrl, mut alt, mut shift, mut meta) = (false, false, false, false);
        for &keycode in &self.pressed {
            match mapping::code_from_keycode(keycode) {
                Code::ControlLeft | Code::ControlRight => ctrl = true,
                Code::AltLeft | Code::AltRight => alt = true,
                Code::ShiftLeft | Code::ShiftRight => shift = true,
                Code::MetaLeft | Code::MetaRight => meta = true,
                _ => {}
            }
        }
        modifiers_from_bools(ctrl, alt, shift, meta)
    }

    /// Update pressed-key state and build the event.
    fn key(&mut self, event: &KeyPressEvent, state: KeyState) -> KeyboardEvent {
        let keycode = event.detail;

        if state == KeyState::Down {
            if !self.pressed.contains(&keycode) {
                self.pressed.push(keycode);
            }
        } else {
            self.pressed.retain(|&held| held != keycode);
        }

        #[cfg(feature = "xkb")]
        self.xkb.update_mask(
            event.mods.base,
            event.mods.latched,
            event.mods.locked,
            u32::from(event.group.base),
            u32::from(event.group.latched),
            u32::from(event.group.locked),
        );

        let code = mapping::code_from_keycode(keycode);
        let repeat = state == KeyState::Down
            && (u32::from(event.flags) & u32::from(KeyEventFlags::KEY_REPEAT)) != 0;

        #[cfg(feature = "xkb")]
        let (key, location) = self.xkb.resolve(keycode, code);
        #[cfg(not(feature = "xkb"))]
        let (key, location) = (Key::Named(NamedKey::Unidentified), Location::Standard);

        KeyboardEvent {
            state,
            key,
            code,
            location,
            modifiers: self.modifiers_from_pressed(),
            repeat,
            is_composing: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use ui_events::keyboard::{Key, Location, NamedKey};
    use x11rb::protocol::xinput::KeyEventFlags;
    #[cfg(feature = "xkb")]
    use x11rb::protocol::xinput::{GroupInfo, ModifierInfo};

    use super::*;

    const KEYCODE_A: u32 = 38;
    const KEYCODE_LEFTCTRL: u32 = 37;
    const KEYCODE_LEFTSHIFT: u32 = 50;
    const KEYCODE_RIGHTSHIFT: u32 = 62;
    #[cfg(feature = "xkb")]
    const KEYCODE_ENTER: u32 = 36;

    fn key_event(keycode: u32) -> KeyPressEvent {
        KeyPressEvent {
            detail: keycode,
            ..Default::default()
        }
    }

    fn repeat_event(keycode: u32) -> KeyPressEvent {
        KeyPressEvent {
            detail: keycode,
            flags: KeyEventFlags::KEY_REPEAT,
            ..Default::default()
        }
    }

    #[test]
    fn key_press_maps_physical_code_and_down_state() {
        let mut reducer = KeyboardEventReducer::default();
        let event = reducer.reduce_key_press(&key_event(KEYCODE_A));

        assert_eq!(event.state, KeyState::Down);
        assert_eq!(event.code, Code::KeyA);
        assert_eq!(event.key, Key::Named(NamedKey::Unidentified));
        assert_eq!(event.location, Location::Standard);
        assert!(!event.repeat);
        assert!(!event.is_composing);
        assert_eq!(event.modifiers, Modifiers::empty());
    }

    #[test]
    fn key_release_maps_up_state() {
        let mut reducer = KeyboardEventReducer::default();
        let _ = reducer.reduce_key_press(&key_event(KEYCODE_A));
        let event = reducer.reduce_key_release(&key_event(KEYCODE_A));

        assert_eq!(event.state, KeyState::Up);
        assert_eq!(event.code, Code::KeyA);
    }

    #[test]
    fn key_repeat_flag_sets_repeat() {
        let mut reducer = KeyboardEventReducer::default();
        let event = reducer.reduce_key_press(&repeat_event(KEYCODE_A));
        assert_eq!(event.state, KeyState::Down);
        assert!(event.repeat);
        let up = reducer.reduce_key_release(&repeat_event(KEYCODE_A));
        assert!(!up.repeat);
    }

    #[test]
    fn reduce_dispatches_key_events_and_ignores_others() {
        let mut reducer = KeyboardEventReducer::default();
        let press = reducer.reduce(&Event::XinputKeyPress(key_event(KEYCODE_A)));
        assert_eq!(press.len(), 1);
        assert_eq!(press[0].state, KeyState::Down);
        assert_eq!(press[0].code, Code::KeyA);

        let release = reducer.reduce(&Event::XinputKeyRelease(key_event(KEYCODE_A)));
        assert_eq!(release.len(), 1);
        assert_eq!(release[0].state, KeyState::Up);

        assert!(
            reducer
                .reduce(&Event::XinputMotion(Default::default()))
                .is_empty()
        );
    }

    #[test]
    fn modifiers_track_physical_modifier_keys() {
        let mut reducer = KeyboardEventReducer::default();

        let ctrl_down = reducer.reduce_key_press(&key_event(KEYCODE_LEFTCTRL));
        assert!(ctrl_down.modifiers.ctrl());
        assert_eq!(ctrl_down.code, Code::ControlLeft);

        let a_down = reducer.reduce_key_press(&key_event(KEYCODE_A));
        assert!(a_down.modifiers.ctrl());

        let ctrl_up = reducer.reduce_key_release(&key_event(KEYCODE_LEFTCTRL));
        assert!(!ctrl_up.modifiers.ctrl());
    }

    #[test]
    fn left_and_right_modifier_stay_active_until_both_released() {
        let mut reducer = KeyboardEventReducer::default();
        let _ = reducer.reduce_key_press(&key_event(KEYCODE_LEFTSHIFT));
        let _ = reducer.reduce_key_press(&key_event(KEYCODE_RIGHTSHIFT));

        let left_up = reducer.reduce_key_release(&key_event(KEYCODE_LEFTSHIFT));
        assert!(left_up.modifiers.shift());

        let right_up = reducer.reduce_key_release(&key_event(KEYCODE_RIGHTSHIFT));
        assert!(!right_up.modifiers.shift());
    }

    #[test]
    fn focus_lost_releases_held_keys_and_clears_modifiers() {
        let mut reducer = KeyboardEventReducer::default();
        let _ = reducer.reduce_key_press(&key_event(KEYCODE_LEFTSHIFT));
        let _ = reducer.reduce_key_press(&key_event(KEYCODE_A));
        assert!(reducer.modifiers().shift());

        let released = reducer.focus_lost();
        assert_eq!(released.len(), 2);
        assert!(released.iter().all(|e| e.state == KeyState::Up));
        assert_eq!(released[0].code, Code::ShiftLeft);
        assert!(!released[0].modifiers.shift());
        assert_eq!(released[1].code, Code::KeyA);
        assert_eq!(released[1].modifiers, Modifiers::empty());
        assert_eq!(reducer.modifiers(), Modifiers::empty());
        assert!(reducer.focus_lost().is_empty());
    }

    #[test]
    fn focus_lost_modifiers_reflect_remaining_keys() {
        let mut reducer = KeyboardEventReducer::default();
        let _ = reducer.reduce_key_press(&key_event(KEYCODE_LEFTCTRL));
        let _ = reducer.reduce_key_press(&key_event(KEYCODE_LEFTSHIFT));

        let released = reducer.focus_lost();
        assert_eq!(released[0].code, Code::ControlLeft);
        assert!(released[0].modifiers.shift());
        assert!(!released[0].modifiers.ctrl());
        assert_eq!(released[1].code, Code::ShiftLeft);
        assert_eq!(released[1].modifiers, Modifiers::empty());
    }

    #[test]
    fn reduce_focus_out_synthesizes_releases() {
        let mut reducer = KeyboardEventReducer::default();
        let _ = reducer.reduce_key_press(&key_event(KEYCODE_LEFTCTRL));
        let events = reducer.reduce(&Event::XinputFocusOut(Default::default()));
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].state, KeyState::Up);
        assert_eq!(events[0].code, Code::ControlLeft);
        assert_eq!(reducer.modifiers(), Modifiers::empty());
    }

    #[test]
    fn unmapped_keycode_is_unidentified() {
        let mut reducer = KeyboardEventReducer::default();
        let event = reducer.reduce_key_press(&key_event(8));
        assert_eq!(event.code, Code::Unidentified);
    }

    #[cfg(feature = "xkb")]
    fn key_event_with_mods(keycode: u32, effective_mods: u32) -> KeyPressEvent {
        KeyPressEvent {
            detail: keycode,
            mods: ModifierInfo {
                base: effective_mods,
                effective: effective_mods,
                ..Default::default()
            },
            group: GroupInfo::default(),
            ..Default::default()
        }
    }

    #[cfg(feature = "xkb")]
    fn us_reducer() -> Option<KeyboardEventReducer> {
        let mut reducer = KeyboardEventReducer::default();
        reducer
            .set_keymap_from_names("", "", "us", "", None)
            .then_some(reducer)
    }

    #[cfg(feature = "xkb")]
    #[test]
    fn xkb_resolves_typed_text() {
        let Some(mut reducer) = us_reducer() else {
            return;
        };
        let event = reducer.reduce_key_press(&key_event(KEYCODE_A));
        assert_eq!(event.key, Key::Character("a".into()));
        assert_eq!(event.code, Code::KeyA);
        assert_eq!(event.location, Location::Standard);
    }

    #[cfg(feature = "xkb")]
    #[test]
    fn xkb_resolves_named_key_and_side_location() {
        let Some(mut reducer) = us_reducer() else {
            return;
        };
        let enter = reducer.reduce_key_press(&key_event(KEYCODE_ENTER));
        assert_eq!(enter.key, Key::Named(NamedKey::Enter));

        let shift = reducer.reduce_key_press(&key_event(KEYCODE_LEFTSHIFT));
        assert_eq!(shift.key, Key::Named(NamedKey::Shift));
        assert_eq!(shift.location, Location::Left);
    }

    #[cfg(feature = "xkb")]
    #[test]
    fn xkb_shift_from_event_mods_yields_uppercase_and_modifier() {
        let Some(mut reducer) = us_reducer() else {
            return;
        };
        let event = reducer.reduce_key_press(&key_event_with_mods(KEYCODE_A, 0x1));
        assert!(event.modifiers.shift());
        assert_eq!(event.key, Key::Character("A".into()));
    }
}
