// Copyright 2026 the UI Events Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Conversions from XKB input primitives into [`ui-events`] types.
//!
//! Functions take plain values (evdev scancodes, XKB keysyms, physical
//! [`Code`]s, modifier booleans) and do not use `xkbcommon`.
//! Live keymap resolution is [`crate::keymap`], under the `xkb` feature.
//!
//! [`ui-events`]: https://docs.rs/ui-events/

use ui_events::keyboard::{Code, Key, Location, Modifiers, NamedKey};

/// Build a [`Modifiers`] set from individual modifier booleans.
///
/// For keymap-less tracking of physical modifier keys.
/// The keymap-aware counterpart is [`modifiers_from_active_mods`].
pub fn modifiers_from_bools(ctrl: bool, alt: bool, shift: bool, meta: bool) -> Modifiers {
    let mut m = Modifiers::default();
    if ctrl {
        m.insert(Modifiers::CONTROL);
    }
    if alt {
        m.insert(Modifiers::ALT);
    }
    if shift {
        m.insert(Modifiers::SHIFT);
    }
    if meta {
        m.insert(Modifiers::META);
    }
    m
}

/// Map an evdev keyboard scancode (`KEY_*`) to its physical [`Code`].
///
/// Wayland reports evdev scancodes directly. An XKB keycode is the same value
/// plus 8.
/// Layout-independent; matches the winit X11/Wayland table.
/// `KEY_LEFTMETA`/`KEY_RIGHTMETA` map to [`Code::MetaLeft`]/[`Code::MetaRight`].
/// Unmapped scancodes (multimedia, browser, power, and gaps) are
/// [`Code::Unidentified`].
///
/// Keymap resolution of the same key uses [`crate::keymap`] with keycode
/// `scancode + 8`.
pub fn code_from_evdev_scancode(scancode: u32) -> Code {
    match scancode {
        // Alphanumeric and surrounding keys.
        1 => Code::Escape,
        2 => Code::Digit1,
        3 => Code::Digit2,
        4 => Code::Digit3,
        5 => Code::Digit4,
        6 => Code::Digit5,
        7 => Code::Digit6,
        8 => Code::Digit7,
        9 => Code::Digit8,
        10 => Code::Digit9,
        11 => Code::Digit0,
        12 => Code::Minus,
        13 => Code::Equal,
        14 => Code::Backspace,
        15 => Code::Tab,
        16 => Code::KeyQ,
        17 => Code::KeyW,
        18 => Code::KeyE,
        19 => Code::KeyR,
        20 => Code::KeyT,
        21 => Code::KeyY,
        22 => Code::KeyU,
        23 => Code::KeyI,
        24 => Code::KeyO,
        25 => Code::KeyP,
        26 => Code::BracketLeft,
        27 => Code::BracketRight,
        28 => Code::Enter,
        29 => Code::ControlLeft,
        30 => Code::KeyA,
        31 => Code::KeyS,
        32 => Code::KeyD,
        33 => Code::KeyF,
        34 => Code::KeyG,
        35 => Code::KeyH,
        36 => Code::KeyJ,
        37 => Code::KeyK,
        38 => Code::KeyL,
        39 => Code::Semicolon,
        40 => Code::Quote,
        41 => Code::Backquote,
        42 => Code::ShiftLeft,
        43 => Code::Backslash,
        44 => Code::KeyZ,
        45 => Code::KeyX,
        46 => Code::KeyC,
        47 => Code::KeyV,
        48 => Code::KeyB,
        49 => Code::KeyN,
        50 => Code::KeyM,
        51 => Code::Comma,
        52 => Code::Period,
        53 => Code::Slash,
        54 => Code::ShiftRight,
        55 => Code::NumpadMultiply,
        56 => Code::AltLeft,
        57 => Code::Space,
        58 => Code::CapsLock,
        // F1..F10.
        59 => Code::F1,
        60 => Code::F2,
        61 => Code::F3,
        62 => Code::F4,
        63 => Code::F5,
        64 => Code::F6,
        65 => Code::F7,
        66 => Code::F8,
        67 => Code::F9,
        68 => Code::F10,
        // Locks and keypad.
        69 => Code::NumLock,
        70 => Code::ScrollLock,
        71 => Code::Numpad7,
        72 => Code::Numpad8,
        73 => Code::Numpad9,
        74 => Code::NumpadSubtract,
        75 => Code::Numpad4,
        76 => Code::Numpad5,
        77 => Code::Numpad6,
        78 => Code::NumpadAdd,
        79 => Code::Numpad1,
        80 => Code::Numpad2,
        81 => Code::Numpad3,
        82 => Code::Numpad0,
        83 => Code::NumpadDecimal,
        // International, F11/F12, editing and navigation.
        85 => Code::Lang5,
        86 => Code::IntlBackslash,
        87 => Code::F11,
        88 => Code::F12,
        89 => Code::IntlRo,
        90 => Code::Lang3,
        91 => Code::Lang4,
        92 => Code::Convert,
        93 => Code::KanaMode,
        94 => Code::NonConvert,
        96 => Code::NumpadEnter,
        97 => Code::ControlRight,
        98 => Code::NumpadDivide,
        99 => Code::PrintScreen,
        100 => Code::AltRight,
        102 => Code::Home,
        103 => Code::ArrowUp,
        104 => Code::PageUp,
        105 => Code::ArrowLeft,
        106 => Code::ArrowRight,
        107 => Code::End,
        108 => Code::ArrowDown,
        109 => Code::PageDown,
        110 => Code::Insert,
        111 => Code::Delete,
        113 => Code::AudioVolumeMute,
        114 => Code::AudioVolumeDown,
        115 => Code::AudioVolumeUp,
        117 => Code::NumpadEqual,
        119 => Code::Pause,
        121 => Code::NumpadComma,
        122 => Code::Lang1,
        123 => Code::Lang2,
        124 => Code::IntlYen,
        // Meta (super) and Menu.
        125 => Code::MetaLeft,
        126 => Code::MetaRight,
        127 => Code::ContextMenu,
        // Media keys and F13..F24.
        163 => Code::MediaTrackNext,
        164 => Code::MediaPlayPause,
        165 => Code::MediaTrackPrevious,
        166 => Code::MediaStop,
        183 => Code::F13,
        184 => Code::F14,
        185 => Code::F15,
        186 => Code::F16,
        187 => Code::F17,
        188 => Code::F18,
        189 => Code::F19,
        190 => Code::F20,
        191 => Code::F21,
        192 => Code::F22,
        193 => Code::F23,
        194 => Code::F24,
        _ => Code::Unidentified,
    }
}

// Named-key XKB keysyms from `xkbcommon/xkbcommon-keysyms.h`.
// Typing keys use Unicode/Latin-1 keysyms and are not listed.
const KEY_ISO_LEVEL3_SHIFT: u32 = 0xfe03;
const KEY_ISO_LEVEL3_LATCH: u32 = 0xfe04;
const KEY_ISO_LEVEL3_LOCK: u32 = 0xfe05;
const KEY_ISO_NEXT_GROUP: u32 = 0xfe08;
const KEY_ISO_PREV_GROUP: u32 = 0xfe0a;
const KEY_ISO_FIRST_GROUP: u32 = 0xfe0c;
const KEY_ISO_LAST_GROUP: u32 = 0xfe0e;
const KEY_ISO_LEFT_TAB: u32 = 0xfe20;
const KEY_ISO_ENTER: u32 = 0xfe34;
const KEY_BACKSPACE: u32 = 0xff08;
const KEY_TAB: u32 = 0xff09;
const KEY_CLEAR: u32 = 0xff0b;
const KEY_RETURN: u32 = 0xff0d;
const KEY_PAUSE: u32 = 0xff13;
const KEY_SCROLL_LOCK: u32 = 0xff14;
const KEY_SYS_REQ: u32 = 0xff15;
const KEY_ESCAPE: u32 = 0xff1b;
const KEY_MULTI_KEY: u32 = 0xff20;
const KEY_KANJI: u32 = 0xff21;
const KEY_MUHENKAN: u32 = 0xff22;
const KEY_HENKAN_MODE: u32 = 0xff23;
const KEY_ROMAJI: u32 = 0xff24;
const KEY_HIRAGANA: u32 = 0xff25;
const KEY_KATAKANA: u32 = 0xff26;
const KEY_HIRAGANA_KATAKANA: u32 = 0xff27;
const KEY_ZENKAKU: u32 = 0xff28;
const KEY_HANKAKU: u32 = 0xff29;
const KEY_ZENKAKU_HANKAKU: u32 = 0xff2a;
const KEY_KANA_LOCK: u32 = 0xff2d;
const KEY_EISU_TOGGLE: u32 = 0xff30;
const KEY_HANGUL: u32 = 0xff31;
const KEY_HANGUL_HANJA: u32 = 0xff34;
const KEY_HOME: u32 = 0xff50;
const KEY_LEFT: u32 = 0xff51;
const KEY_UP: u32 = 0xff52;
const KEY_RIGHT: u32 = 0xff53;
const KEY_DOWN: u32 = 0xff54;
const KEY_PAGE_UP: u32 = 0xff55;
const KEY_PAGE_DOWN: u32 = 0xff56;
const KEY_END: u32 = 0xff57;
const KEY_SELECT: u32 = 0xff60;
const KEY_PRINT: u32 = 0xff61;
const KEY_EXECUTE: u32 = 0xff62;
const KEY_INSERT: u32 = 0xff63;
const KEY_UNDO: u32 = 0xff65;
const KEY_REDO: u32 = 0xff66;
const KEY_MENU: u32 = 0xff67;
const KEY_FIND: u32 = 0xff68;
const KEY_CANCEL: u32 = 0xff69;
const KEY_HELP: u32 = 0xff6a;
const KEY_BREAK: u32 = 0xff6b;
const KEY_MODE_SWITCH: u32 = 0xff7e;
const KEY_NUM_LOCK: u32 = 0xff7f;
const KEY_KP_TAB: u32 = 0xff89;
const KEY_KP_ENTER: u32 = 0xff8d;
const KEY_KP_F1: u32 = 0xff91;
const KEY_KP_F2: u32 = 0xff92;
const KEY_KP_F3: u32 = 0xff93;
const KEY_KP_F4: u32 = 0xff94;
const KEY_KP_HOME: u32 = 0xff95;
const KEY_KP_LEFT: u32 = 0xff96;
const KEY_KP_UP: u32 = 0xff97;
const KEY_KP_RIGHT: u32 = 0xff98;
const KEY_KP_DOWN: u32 = 0xff99;
const KEY_KP_PAGE_UP: u32 = 0xff9a;
const KEY_KP_PAGE_DOWN: u32 = 0xff9b;
const KEY_KP_END: u32 = 0xff9c;
const KEY_KP_INSERT: u32 = 0xff9e;
const KEY_KP_DELETE: u32 = 0xff9f;
const KEY_F1: u32 = 0xffbe;
const KEY_F2: u32 = 0xffbf;
const KEY_F3: u32 = 0xffc0;
const KEY_F4: u32 = 0xffc1;
const KEY_F5: u32 = 0xffc2;
const KEY_F6: u32 = 0xffc3;
const KEY_F7: u32 = 0xffc4;
const KEY_F8: u32 = 0xffc5;
const KEY_F9: u32 = 0xffc6;
const KEY_F10: u32 = 0xffc7;
const KEY_F11: u32 = 0xffc8;
const KEY_F12: u32 = 0xffc9;
const KEY_F13: u32 = 0xffca;
const KEY_F14: u32 = 0xffcb;
const KEY_F15: u32 = 0xffcc;
const KEY_F16: u32 = 0xffcd;
const KEY_F17: u32 = 0xffce;
const KEY_F18: u32 = 0xffcf;
const KEY_F19: u32 = 0xffd0;
const KEY_F20: u32 = 0xffd1;
const KEY_F21: u32 = 0xffd2;
const KEY_F22: u32 = 0xffd3;
const KEY_F23: u32 = 0xffd4;
const KEY_F24: u32 = 0xffd5;
const KEY_F25: u32 = 0xffd6;
const KEY_F26: u32 = 0xffd7;
const KEY_F27: u32 = 0xffd8;
const KEY_F28: u32 = 0xffd9;
const KEY_F29: u32 = 0xffda;
const KEY_F30: u32 = 0xffdb;
const KEY_F31: u32 = 0xffdc;
const KEY_F32: u32 = 0xffdd;
const KEY_F33: u32 = 0xffde;
const KEY_F34: u32 = 0xffdf;
const KEY_F35: u32 = 0xffe0;
const KEY_SHIFT_L: u32 = 0xffe1;
const KEY_SHIFT_R: u32 = 0xffe2;
const KEY_CONTROL_L: u32 = 0xffe3;
const KEY_CONTROL_R: u32 = 0xffe4;
const KEY_CAPS_LOCK: u32 = 0xffe5;
const KEY_META_L: u32 = 0xffe7;
const KEY_META_R: u32 = 0xffe8;
const KEY_ALT_L: u32 = 0xffe9;
const KEY_ALT_R: u32 = 0xffea;
const KEY_SUPER_L: u32 = 0xffeb;
const KEY_SUPER_R: u32 = 0xffec;
const KEY_HYPER_L: u32 = 0xffed;
const KEY_HYPER_R: u32 = 0xffee;
const KEY_DELETE: u32 = 0xffff;

/// Map an XKB keysym to a W3C [`NamedKey`] for a non-typing key.
///
/// Letters, digits, punctuation, and space return `None`; [`key_from_keysym`]
/// uses their text instead.
/// Keypad digit/operator keysyms also return `None` (Num Lock off reports
/// `KP_Left` and similar, which are named here).
/// Super, Hyper, and Meta all map to [`NamedKey::Meta`].
pub fn named_key_from_keysym(keysym: u32) -> Option<NamedKey> {
    Some(match keysym {
        KEY_BACKSPACE => NamedKey::Backspace,
        KEY_TAB | KEY_KP_TAB | KEY_ISO_LEFT_TAB => NamedKey::Tab,
        KEY_CLEAR => NamedKey::Clear,
        KEY_RETURN | KEY_KP_ENTER | KEY_ISO_ENTER => NamedKey::Enter,
        KEY_PAUSE | KEY_BREAK => NamedKey::Pause,
        KEY_SCROLL_LOCK => NamedKey::ScrollLock,
        KEY_SYS_REQ | KEY_PRINT => NamedKey::PrintScreen,
        KEY_ESCAPE => NamedKey::Escape,
        KEY_DELETE | KEY_KP_DELETE => NamedKey::Delete,

        // Input method and group.
        KEY_MULTI_KEY => NamedKey::Compose,
        KEY_MODE_SWITCH => NamedKey::ModeChange,
        KEY_ISO_NEXT_GROUP => NamedKey::GroupNext,
        KEY_ISO_PREV_GROUP => NamedKey::GroupPrevious,
        KEY_ISO_FIRST_GROUP => NamedKey::GroupFirst,
        KEY_ISO_LAST_GROUP => NamedKey::GroupLast,

        // Japanese and Korean.
        KEY_KANJI => NamedKey::KanjiMode,
        KEY_MUHENKAN => NamedKey::NonConvert,
        KEY_HENKAN_MODE => NamedKey::Convert,
        KEY_ROMAJI => NamedKey::Romaji,
        KEY_HIRAGANA => NamedKey::Hiragana,
        KEY_KATAKANA => NamedKey::Katakana,
        KEY_HIRAGANA_KATAKANA => NamedKey::HiraganaKatakana,
        KEY_ZENKAKU => NamedKey::Zenkaku,
        KEY_HANKAKU => NamedKey::Hankaku,
        KEY_ZENKAKU_HANKAKU => NamedKey::ZenkakuHankaku,
        KEY_KANA_LOCK => NamedKey::KanaMode,
        KEY_EISU_TOGGLE => NamedKey::Alphanumeric,
        KEY_HANGUL => NamedKey::HangulMode,
        KEY_HANGUL_HANJA => NamedKey::HanjaMode,

        // Navigation and editing (including keypad with Num Lock off).
        KEY_HOME | KEY_KP_HOME => NamedKey::Home,
        KEY_LEFT | KEY_KP_LEFT => NamedKey::ArrowLeft,
        KEY_UP | KEY_KP_UP => NamedKey::ArrowUp,
        KEY_RIGHT | KEY_KP_RIGHT => NamedKey::ArrowRight,
        KEY_DOWN | KEY_KP_DOWN => NamedKey::ArrowDown,
        KEY_PAGE_UP | KEY_KP_PAGE_UP => NamedKey::PageUp,
        KEY_PAGE_DOWN | KEY_KP_PAGE_DOWN => NamedKey::PageDown,
        KEY_END | KEY_KP_END => NamedKey::End,
        KEY_INSERT | KEY_KP_INSERT => NamedKey::Insert,
        KEY_SELECT => NamedKey::Select,
        KEY_EXECUTE => NamedKey::Execute,
        KEY_UNDO => NamedKey::Undo,
        KEY_REDO => NamedKey::Redo,
        KEY_MENU => NamedKey::ContextMenu,
        KEY_FIND => NamedKey::Find,
        KEY_CANCEL => NamedKey::Cancel,
        KEY_HELP => NamedKey::Help,
        KEY_NUM_LOCK => NamedKey::NumLock,

        // Function keys, including keypad F1..F4.
        KEY_F1 | KEY_KP_F1 => NamedKey::F1,
        KEY_F2 | KEY_KP_F2 => NamedKey::F2,
        KEY_F3 | KEY_KP_F3 => NamedKey::F3,
        KEY_F4 | KEY_KP_F4 => NamedKey::F4,
        KEY_F5 => NamedKey::F5,
        KEY_F6 => NamedKey::F6,
        KEY_F7 => NamedKey::F7,
        KEY_F8 => NamedKey::F8,
        KEY_F9 => NamedKey::F9,
        KEY_F10 => NamedKey::F10,
        KEY_F11 => NamedKey::F11,
        KEY_F12 => NamedKey::F12,
        KEY_F13 => NamedKey::F13,
        KEY_F14 => NamedKey::F14,
        KEY_F15 => NamedKey::F15,
        KEY_F16 => NamedKey::F16,
        KEY_F17 => NamedKey::F17,
        KEY_F18 => NamedKey::F18,
        KEY_F19 => NamedKey::F19,
        KEY_F20 => NamedKey::F20,
        KEY_F21 => NamedKey::F21,
        KEY_F22 => NamedKey::F22,
        KEY_F23 => NamedKey::F23,
        KEY_F24 => NamedKey::F24,
        KEY_F25 => NamedKey::F25,
        KEY_F26 => NamedKey::F26,
        KEY_F27 => NamedKey::F27,
        KEY_F28 => NamedKey::F28,
        KEY_F29 => NamedKey::F29,
        KEY_F30 => NamedKey::F30,
        KEY_F31 => NamedKey::F31,
        KEY_F32 => NamedKey::F32,
        KEY_F33 => NamedKey::F33,
        KEY_F34 => NamedKey::F34,
        KEY_F35 => NamedKey::F35,

        // Modifiers.
        KEY_SHIFT_L | KEY_SHIFT_R => NamedKey::Shift,
        KEY_CONTROL_L | KEY_CONTROL_R => NamedKey::Control,
        KEY_CAPS_LOCK => NamedKey::CapsLock,
        KEY_ALT_L | KEY_ALT_R => NamedKey::Alt,
        KEY_META_L | KEY_META_R | KEY_SUPER_L | KEY_SUPER_R | KEY_HYPER_L | KEY_HYPER_R => {
            NamedKey::Meta
        }
        KEY_ISO_LEVEL3_SHIFT | KEY_ISO_LEVEL3_LATCH | KEY_ISO_LEVEL3_LOCK => NamedKey::AltGraph,

        _ => return None,
    })
}

/// Resolve an XKB keysym and its typed text into a logical [`Key`].
///
/// Named keys win even when `text` is a control character (Enter, Escape).
/// Otherwise non-empty printable `text` is [`Key::Character`].
/// Anything else is [`NamedKey::Unidentified`].
///
/// `text` comes from `xkb_state_key_get_utf8`.
pub fn key_from_keysym(keysym: u32, text: &str) -> Key {
    if let Some(named) = named_key_from_keysym(keysym) {
        return Key::Named(named);
    }
    if !text.is_empty() && !text.chars().any(char::is_control) {
        return Key::Character(text.into());
    }
    Key::Named(NamedKey::Unidentified)
}

/// Map a physical [`Code`] to its W3C keyboard [`Location`].
///
/// Sided modifiers are Left/Right. Keypad keys are Numpad.
/// Everything else, including `NumLock`, is Standard.
pub fn location_from_code(code: Code) -> Location {
    match code {
        Code::ShiftLeft | Code::ControlLeft | Code::AltLeft | Code::MetaLeft => Location::Left,
        Code::ShiftRight | Code::ControlRight | Code::AltRight | Code::MetaRight => Location::Right,
        Code::Numpad0
        | Code::Numpad1
        | Code::Numpad2
        | Code::Numpad3
        | Code::Numpad4
        | Code::Numpad5
        | Code::Numpad6
        | Code::Numpad7
        | Code::Numpad8
        | Code::Numpad9
        | Code::NumpadAdd
        | Code::NumpadSubtract
        | Code::NumpadMultiply
        | Code::NumpadDivide
        | Code::NumpadDecimal
        | Code::NumpadComma
        | Code::NumpadEnter
        | Code::NumpadEqual => Location::Numpad,
        _ => Location::Standard,
    }
}

/// Assemble a [`Modifiers`] set from active XKB modifiers.
///
/// Keymap-aware counterpart to [`modifiers_from_bools`]: includes lock states
/// and Alt Graph. Each flag is `xkb_state_mod_name_is_active` for that name.
pub fn modifiers_from_active_mods(
    ctrl: bool,
    alt: bool,
    shift: bool,
    meta: bool,
    caps_lock: bool,
    num_lock: bool,
    alt_graph: bool,
) -> Modifiers {
    let mut m = modifiers_from_bools(ctrl, alt, shift, meta);
    if caps_lock {
        m.insert(Modifiers::CAPS_LOCK);
    }
    if num_lock {
        m.insert(Modifiers::NUM_LOCK);
    }
    if alt_graph {
        m.insert(Modifiers::ALT_GRAPH);
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifiers_from_bools_sets_expected_bits() {
        let mods = modifiers_from_bools(true, false, true, false);
        assert!(mods.ctrl());
        assert!(!mods.alt());
        assert!(mods.shift());
        assert!(!mods.meta());
    }

    #[test]
    fn evdev_scancodes_map_to_expected_physical_codes() {
        assert_eq!(code_from_evdev_scancode(30), Code::KeyA);
        assert_eq!(code_from_evdev_scancode(1), Code::Escape);
        assert_eq!(code_from_evdev_scancode(28), Code::Enter);
        assert_eq!(code_from_evdev_scancode(57), Code::Space);
        assert_eq!(code_from_evdev_scancode(2), Code::Digit1);
        assert_eq!(code_from_evdev_scancode(11), Code::Digit0);
        assert_eq!(code_from_evdev_scancode(82), Code::Numpad0);
        assert_eq!(code_from_evdev_scancode(96), Code::NumpadEnter);
        assert_eq!(code_from_evdev_scancode(59), Code::F1);
        assert_eq!(code_from_evdev_scancode(88), Code::F12);
        assert_eq!(code_from_evdev_scancode(183), Code::F13);
        assert_eq!(code_from_evdev_scancode(194), Code::F24);
    }

    #[test]
    fn evdev_modifier_scancodes_map_to_sided_codes() {
        assert_eq!(code_from_evdev_scancode(29), Code::ControlLeft);
        assert_eq!(code_from_evdev_scancode(97), Code::ControlRight);
        assert_eq!(code_from_evdev_scancode(42), Code::ShiftLeft);
        assert_eq!(code_from_evdev_scancode(54), Code::ShiftRight);
        assert_eq!(code_from_evdev_scancode(56), Code::AltLeft);
        assert_eq!(code_from_evdev_scancode(100), Code::AltRight);
        assert_eq!(code_from_evdev_scancode(125), Code::MetaLeft);
        assert_eq!(code_from_evdev_scancode(126), Code::MetaRight);
    }

    #[test]
    fn unmapped_evdev_scancodes_are_unidentified() {
        assert_eq!(code_from_evdev_scancode(0), Code::Unidentified);
        assert_eq!(code_from_evdev_scancode(84), Code::Unidentified);
        assert_eq!(code_from_evdev_scancode(116), Code::Unidentified);
        assert_eq!(code_from_evdev_scancode(u32::MAX), Code::Unidentified);
    }

    #[test]
    fn keysym_named_keys_map_to_expected_named_keys() {
        assert_eq!(named_key_from_keysym(KEY_ESCAPE), Some(NamedKey::Escape));
        assert_eq!(named_key_from_keysym(KEY_RETURN), Some(NamedKey::Enter));
        assert_eq!(named_key_from_keysym(KEY_KP_ENTER), Some(NamedKey::Enter));
        assert_eq!(
            named_key_from_keysym(KEY_BACKSPACE),
            Some(NamedKey::Backspace)
        );
        assert_eq!(named_key_from_keysym(KEY_F1), Some(NamedKey::F1));
        assert_eq!(named_key_from_keysym(KEY_F24), Some(NamedKey::F24));
        assert_eq!(named_key_from_keysym(KEY_LEFT), Some(NamedKey::ArrowLeft));
        assert_eq!(
            named_key_from_keysym(KEY_KP_LEFT),
            Some(NamedKey::ArrowLeft)
        );
        assert_eq!(named_key_from_keysym(KEY_SHIFT_L), Some(NamedKey::Shift));
        assert_eq!(named_key_from_keysym(KEY_SHIFT_R), Some(NamedKey::Shift));
        assert_eq!(named_key_from_keysym(KEY_SUPER_L), Some(NamedKey::Meta));
        assert_eq!(named_key_from_keysym(KEY_META_R), Some(NamedKey::Meta));
        assert_eq!(
            named_key_from_keysym(KEY_ISO_LEVEL3_SHIFT),
            Some(NamedKey::AltGraph)
        );
        assert_eq!(named_key_from_keysym(KEY_NUM_LOCK), Some(NamedKey::NumLock));
    }

    #[test]
    fn typing_keysyms_are_not_named() {
        assert_eq!(named_key_from_keysym(0x61), None); // 'a'
        assert_eq!(named_key_from_keysym(0x41), None); // 'A'
        assert_eq!(named_key_from_keysym(0x31), None); // '1'
        assert_eq!(named_key_from_keysym(0x20), None); // space
        assert_eq!(named_key_from_keysym(0), None); // NoSymbol
    }

    #[test]
    fn key_from_keysym_prefers_named_then_text() {
        assert_eq!(
            key_from_keysym(KEY_ESCAPE, "\u{1b}"),
            Key::Named(NamedKey::Escape)
        );
        assert_eq!(key_from_keysym(0x61, "a"), Key::Character("a".into()));
        assert_eq!(key_from_keysym(0x20, " "), Key::Character(" ".into()));
        assert_eq!(
            key_from_keysym(0, "\u{1b}"),
            Key::Named(NamedKey::Unidentified)
        );
        assert_eq!(key_from_keysym(0, ""), Key::Named(NamedKey::Unidentified));
    }

    #[test]
    fn location_from_code_classifies_sides_and_numpad() {
        assert_eq!(location_from_code(Code::ShiftLeft), Location::Left);
        assert_eq!(location_from_code(Code::ControlRight), Location::Right);
        assert_eq!(location_from_code(Code::MetaLeft), Location::Left);
        assert_eq!(location_from_code(Code::Numpad5), Location::Numpad);
        assert_eq!(location_from_code(Code::NumpadEnter), Location::Numpad);
        assert_eq!(location_from_code(Code::KeyA), Location::Standard);
        assert_eq!(location_from_code(Code::NumLock), Location::Standard);
    }

    #[test]
    fn modifiers_from_active_mods_adds_locks_and_alt_graph() {
        let mods = modifiers_from_active_mods(true, false, true, false, true, false, true);
        assert!(mods.contains(Modifiers::CONTROL));
        assert!(mods.contains(Modifiers::SHIFT));
        assert!(mods.contains(Modifiers::CAPS_LOCK));
        assert!(mods.contains(Modifiers::ALT_GRAPH));
        assert!(!mods.contains(Modifiers::ALT));
        assert!(!mods.contains(Modifiers::META));
        assert!(!mods.contains(Modifiers::NUM_LOCK));
    }

    #[cfg(feature = "xkb")]
    #[test]
    fn keysym_constants_match_xkbcommon() {
        use xkbcommon::xkb::keysyms;
        assert_eq!(KEY_BACKSPACE, keysyms::KEY_BackSpace);
        assert_eq!(KEY_TAB, keysyms::KEY_Tab);
        assert_eq!(KEY_RETURN, keysyms::KEY_Return);
        assert_eq!(KEY_ESCAPE, keysyms::KEY_Escape);
        assert_eq!(KEY_DELETE, keysyms::KEY_Delete);
        assert_eq!(KEY_HOME, keysyms::KEY_Home);
        assert_eq!(KEY_LEFT, keysyms::KEY_Left);
        assert_eq!(KEY_END, keysyms::KEY_End);
        assert_eq!(KEY_KP_ENTER, keysyms::KEY_KP_Enter);
        assert_eq!(KEY_KP_LEFT, keysyms::KEY_KP_Left);
        assert_eq!(KEY_NUM_LOCK, keysyms::KEY_Num_Lock);
        assert_eq!(KEY_F1, keysyms::KEY_F1);
        assert_eq!(KEY_F12, keysyms::KEY_F12);
        assert_eq!(KEY_F35, keysyms::KEY_F35);
        assert_eq!(KEY_SHIFT_L, keysyms::KEY_Shift_L);
        assert_eq!(KEY_CONTROL_R, keysyms::KEY_Control_R);
        assert_eq!(KEY_SUPER_L, keysyms::KEY_Super_L);
        assert_eq!(KEY_ALT_R, keysyms::KEY_Alt_R);
        assert_eq!(KEY_ISO_LEVEL3_SHIFT, keysyms::KEY_ISO_Level3_Shift);
        assert_eq!(KEY_ISO_LEFT_TAB, keysyms::KEY_ISO_Left_Tab);
    }
}
