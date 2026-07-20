// Copyright 2026 the UI Events Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Reduces XInput 2 pointer events into [`PointerEvent`]s.
//!
//! Keep one [`PointerEventReducer`] per pointer.
//! Feed each XInput 2 pointer event for buttons (with click counts), moves,
//! crossings, and scrolls.
//! `POINTER_EMULATED` events (scroll or touch emulated as buttons) are ignored.
//! Non-emulated wheel buttons `4`..`7` become one-line [`ScrollDelta`]s on press.
//!
//! Prefer scroll-class valuators from `XIQueryDevice`, configured with
//! [`set_scroll_axes`].
//! Baselines are per `sourceid` so slaves on one master stay independent.
//! Until axes are set, only non-emulated legacy wheel buttons produce scroll.
//!
//! Valuator deltas are `-(new - old) / increment` line units.
//! The sign flip matches winit and AppKit content-direction
//! (positive Y: content moves down).
//! XI2 maps `-increment` to button 4 and `+increment` to button 5; after the
//! flip those are content-up and content-down.
//! A negative `increment` (inverted axis) is applied by the division.
//! libinput natural scrolling is already applied before clients see valuators.
//!
//! Positions are physical pixels.
//! [`reduce`] takes scale factor and host-clock monotonic nanoseconds (not the
//! server ms stamp).
//! Modifiers come from the event's effective mask.
//!
//! # Pointer identity
//!
//! The master pointer is always [`PointerId::PRIMARY`].
//! `sourceid` is stored as [`PointerInfo::persistent_device_id`].
//! Use one reducer per master when there are several.
//!
//! [`reduce`]: PointerEventReducer::reduce
//! [`set_scroll_axes`]: PointerEventReducer::set_scroll_axes
//! [`PointerState`]: ui_events::pointer::PointerState

use alloc::{vec, vec::Vec};

use ui_events::ScrollDelta;
use ui_events::pointer::{
    PersistentDeviceId, PointerButton, PointerButtonEvent, PointerEvent, PointerId, PointerInfo,
    PointerScrollEvent, PointerState, PointerType, PointerUpdate,
};
use x11rb::protocol::Event;
use x11rb::protocol::xinput::{ButtonPressEvent, DeviceId, EnterEvent, Fp3232, PointerEventFlags};

use crate::mapping;
use crate::tap::TapCounter;

/// Pressure while any button is held (XInput 2 does not report pressure).
const ACTIVE_PRESSURE: f32 = 0.5;
/// Pressure when no buttons are held.
const RELEASED_PRESSURE: f32 = 0.0;

/// Direction of a scroll-class valuator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollAxisKind {
    /// Contributes to [`ScrollDelta`] `y`.
    Vertical,
    /// Contributes to [`ScrollDelta`] `x`.
    Horizontal,
}

/// One scroll-class valuator from `XIQueryDevice`.
///
/// Pass a set to [`PointerEventReducer::set_scroll_axes`].
/// `increment` is valuator units per line (from `XIScrollClassInfo` via
/// [`mapping::fp3232_to_f64`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollAxis {
    /// Valuator number on pointer events.
    pub number: u16,
    /// Valuator delta per scroll unit.
    pub increment: f64,
    /// Axis orientation.
    pub kind: ScrollAxisKind,
}

/// Configured axis plus last sample for one source device.
#[derive(Clone, Copy, Debug)]
struct ScrollAxisState {
    axis: ScrollAxis,
    /// First sample sets the baseline only.
    last_value: Option<f64>,
}

/// Scroll baselines for one `sourceid` feeding a master pointer.
#[derive(Clone, Debug)]
struct SourceScrollState {
    sourceid: DeviceId,
    axes: Vec<ScrollAxisState>,
}

/// Reduces an XInput 2 pointer event stream into [`PointerEvent`]s.
///
/// One reducer per seat pointer.
/// Configure scroll with [`set_scroll_axes`].
/// See the [module documentation](self).
///
/// [`set_scroll_axes`]: PointerEventReducer::set_scroll_axes
#[derive(Debug, Default)]
pub struct PointerEventReducer {
    primary_state: PointerState,
    counter: TapCounter,
    last_seen_time: Option<u64>,
    /// False until a position is observed (first sample still emits Move).
    positioned: bool,
    scroll_axes: Vec<ScrollAxis>,
    scroll_sources: Vec<SourceScrollState>,
}

impl PointerEventReducer {
    /// Replace the scroll-class axis list and clear baselines.
    ///
    /// Until at least one axis is set, only non-emulated wheel buttons scroll.
    pub fn set_scroll_axes(&mut self, axes: &[ScrollAxis]) {
        self.scroll_axes = axes.to_vec();
        self.scroll_sources.clear();
    }

    /// Reduce a decoded [`Event`] into zero or more [`PointerEvent`]s.
    ///
    /// Translates `ButtonPress`/`ButtonRelease`, `Motion`, `Enter`, and `Leave`.
    /// One motion can yield both Move and Scroll.
    /// `scale_factor` and host-clock `time` (ns) are stamped on every
    /// [`PointerState`]; `time` must not regress.
    pub fn reduce(&mut self, scale_factor: f64, event: &Event, time: u64) -> Vec<PointerEvent> {
        self.check_time_monotonic(time);
        self.primary_state.time = time;
        self.primary_state.scale_factor = scale_factor;

        match event {
            Event::XinputButtonPress(event) => self.button(event, true),
            Event::XinputButtonRelease(event) => self.button(event, false),
            Event::XinputMotion(event) => self.motion(event),
            Event::XinputEnter(event) => vec![self.enter(event)],
            Event::XinputLeave(event) => self.leave(event),
            _ => Vec::new(),
        }
    }

    /// Primary mouse identity; `sourceid` is the slave that produced the event.
    fn primary_mouse(sourceid: DeviceId) -> PointerInfo {
        PointerInfo {
            pointer_id: Some(PointerId::PRIMARY),
            persistent_device_id: PersistentDeviceId::new(u64::from(sourceid)),
            pointer_type: PointerType::Mouse,
        }
    }

    /// Apply the event's position and effective-modifier mask to primary state.
    fn apply_common(&mut self, event_x: i32, event_y: i32, mods_effective: u32) {
        self.primary_state.position = mapping::position_from_fp1616(event_x, event_y);
        self.primary_state.modifiers = mapping::modifiers_from_xi_mask(mods_effective);
        self.positioned = true;
    }

    /// Button press/release, or a non-emulated legacy wheel as scroll.
    fn button(&mut self, event: &ButtonPressEvent, pressed: bool) -> Vec<PointerEvent> {
        if pointer_emulated(event.flags) {
            return Vec::new();
        }

        self.apply_common(event.event_x, event.event_y, event.mods.effective);

        let pointer = Self::primary_mouse(event.sourceid);

        if let Some((dx, dy)) = mapping::legacy_wheel_line_delta(event.detail) {
            if !pressed {
                return Vec::new();
            }
            return vec![PointerEvent::Scroll(PointerScrollEvent {
                pointer,
                delta: ScrollDelta::LineDelta(dx, dy),
                state: self.primary_state.clone(),
            })];
        }

        let Some(button) = mapping::pointer_button_from_detail(event.detail) else {
            return Vec::new();
        };

        let pointer_event = if pressed {
            self.primary_state.buttons.insert(button);
            self.primary_state.pressure = ACTIVE_PRESSURE;
            PointerEvent::Down(PointerButtonEvent {
                button: Some(button),
                pointer,
                state: self.primary_state.clone(),
            })
        } else {
            self.primary_state.buttons.remove(button);
            if self.primary_state.buttons.is_empty() {
                self.primary_state.pressure = RELEASED_PRESSURE;
            }
            PointerEvent::Up(PointerButtonEvent {
                button: Some(button),
                pointer,
                state: self.primary_state.clone(),
            })
        };

        let scale = self.primary_state.scale_factor;
        vec![self.counter.attach_count(scale, pointer_event)]
    }

    /// Motion: Move if position changed, Scroll if a valuator changed.
    fn motion(&mut self, event: &ButtonPressEvent) -> Vec<PointerEvent> {
        if pointer_emulated(event.flags) {
            return Vec::new();
        }

        let mut events = Vec::new();
        let pointer = Self::primary_mouse(event.sourceid);

        let new_position = mapping::position_from_fp1616(event.event_x, event.event_y);
        self.primary_state.modifiers = mapping::modifiers_from_xi_mask(event.mods.effective);
        let moved = !self.positioned || new_position != self.primary_state.position;
        self.primary_state.position = new_position;
        self.positioned = true;
        if moved {
            let scale = self.primary_state.scale_factor;
            events.push(self.counter.attach_count(
                scale,
                PointerEvent::Move(PointerUpdate {
                    pointer,
                    current: self.primary_state.clone(),
                    coalesced: Vec::new(),
                    predicted: Vec::new(),
                }),
            ));
        }

        if let Some(delta) = self.scroll_delta(event) {
            events.push(PointerEvent::Scroll(PointerScrollEvent {
                pointer,
                delta,
                state: self.primary_state.clone(),
            }));
        }

        events
    }

    /// Scroll delta from valuators; updates per-source baselines.
    fn scroll_delta(&mut self, event: &ButtonPressEvent) -> Option<ScrollDelta> {
        if self.scroll_axes.is_empty() {
            return None;
        }
        let source = self.source_scroll(event.sourceid);
        let (mut dx, mut dy) = (0.0_f32, 0.0_f32);
        for state in &mut source.axes {
            let Some(value) =
                valuator_value(&event.valuator_mask, &event.axisvalues, state.axis.number)
            else {
                continue;
            };
            if let Some(last) = state.last_value {
                // X11 valuator sign is opposite winit/AppKit content-direction.
                let delta = -line_delta(value - last, state.axis.increment);
                match state.axis.kind {
                    ScrollAxisKind::Vertical => dy += delta,
                    ScrollAxisKind::Horizontal => dx += delta,
                }
            }
            state.last_value = Some(value);
        }
        (dx != 0.0 || dy != 0.0).then_some(ScrollDelta::LineDelta(dx, dy))
    }

    fn source_scroll(&mut self, sourceid: DeviceId) -> &mut SourceScrollState {
        if let Some(index) = self
            .scroll_sources
            .iter()
            .position(|s| s.sourceid == sourceid)
        {
            return &mut self.scroll_sources[index];
        }
        self.scroll_sources.push(SourceScrollState {
            sourceid,
            axes: self
                .scroll_axes
                .iter()
                .map(|&axis| ScrollAxisState {
                    axis,
                    last_value: None,
                })
                .collect(),
        });
        self.scroll_sources
            .last_mut()
            .expect("just pushed a source scroll state")
    }

    fn enter(&mut self, event: &EnterEvent) -> PointerEvent {
        self.apply_common(event.event_x, event.event_y, event.mods.effective);
        self.reset_scroll();
        PointerEvent::Enter(Self::primary_mouse(event.sourceid))
    }

    /// Synthetic Ups for held buttons, then reset scroll baselines.
    ///
    /// Releases outside the window are often missing without a grab.
    fn leave(&mut self, event: &EnterEvent) -> Vec<PointerEvent> {
        self.apply_common(event.event_x, event.event_y, event.mods.effective);
        self.reset_scroll();

        let pointer = Self::primary_mouse(event.sourceid);
        let scale = self.primary_state.scale_factor;
        let mut events = Vec::new();

        for button in HELD_BUTTON_CANDIDATES {
            if !self.primary_state.buttons.contains(button) {
                continue;
            }
            self.primary_state.buttons.remove(button);
            if self.primary_state.buttons.is_empty() {
                self.primary_state.pressure = RELEASED_PRESSURE;
            }
            events.push(self.counter.attach_count(
                scale,
                PointerEvent::Up(PointerButtonEvent {
                    button: Some(button),
                    pointer,
                    state: self.primary_state.clone(),
                }),
            ));
        }
        self.primary_state.buttons.clear();
        self.primary_state.pressure = RELEASED_PRESSURE;

        events.push(
            self.counter
                .attach_count(scale, PointerEvent::Leave(pointer)),
        );
        events
    }

    fn reset_scroll(&mut self) {
        self.scroll_sources.clear();
    }

    fn check_time_monotonic(&mut self, time: u64) {
        if let Some(previous) = self.last_seen_time {
            debug_assert!(
                time >= previous,
                "PointerEventReducer::reduce timestamps must be monotonic nanoseconds"
            );
        }
        self.last_seen_time = Some(time);
    }
}

fn pointer_emulated(flags: PointerEventFlags) -> bool {
    u32::from(flags) & u32::from(PointerEventFlags::POINTER_EMULATED) != 0
}

/// Buttons scanned for synthetic Ups on leave (wheel buttons never enter the set).
const HELD_BUTTON_CANDIDATES: [PointerButton; 31] = [
    PointerButton::Primary,
    PointerButton::Secondary,
    PointerButton::Auxiliary,
    PointerButton::X1,
    PointerButton::X2,
    PointerButton::B7,
    PointerButton::B8,
    PointerButton::B9,
    PointerButton::B10,
    PointerButton::B11,
    PointerButton::B12,
    PointerButton::B13,
    PointerButton::B14,
    PointerButton::B15,
    PointerButton::B16,
    PointerButton::B17,
    PointerButton::B18,
    PointerButton::B19,
    PointerButton::B20,
    PointerButton::B21,
    PointerButton::B22,
    PointerButton::B23,
    PointerButton::B24,
    PointerButton::B25,
    PointerButton::B26,
    PointerButton::B27,
    PointerButton::B28,
    PointerButton::B29,
    PointerButton::B30,
    PointerButton::B31,
    PointerButton::B32,
];

/// Valuator change divided by increment, as an `f32` line delta.
#[expect(
    clippy::cast_possible_truncation,
    reason = "scroll deltas are small; f32 is the LineDelta component type"
)]
fn line_delta(delta_value: f64, increment: f64) -> f32 {
    if increment == 0.0 {
        return 0.0;
    }
    (delta_value / increment) as f32
}

/// Value of valuator `number` from an event's mask/`axisvalues`, if present.
fn valuator_value(mask: &[u32], values: &[Fp3232], number: u16) -> Option<f64> {
    let word = usize::from(number / 32);
    let bit = u32::from(number % 32);
    let word_value = *mask.get(word)?;
    if word_value & (1 << bit) == 0 {
        return None;
    }
    // Index among set valuators: popcount of earlier mask bits.
    let mut index = 0_usize;
    for &earlier in &mask[..word] {
        index += earlier.count_ones() as usize;
    }
    index += (word_value & ((1 << bit) - 1)).count_ones() as usize;
    values
        .get(index)
        .map(|value| mapping::fp3232_to_f64(value.integral, value.frac))
}

#[cfg(test)]
mod tests {
    use dpi::PhysicalPosition;
    use ui_events::keyboard::Modifiers;
    use ui_events::pointer::PointerButton;
    use x11rb::protocol::xinput::ModifierInfo;

    use super::*;

    fn px(pixels: i32) -> i32 {
        pixels * 65536
    }

    fn units(value: i32) -> Fp3232 {
        Fp3232 {
            integral: value,
            frac: 0,
        }
    }

    fn button_event(detail: u32, x: i32, y: i32) -> ButtonPressEvent {
        ButtonPressEvent {
            detail,
            event_x: px(x),
            event_y: px(y),
            ..Default::default()
        }
    }

    fn button_event_mods(detail: u32, x: i32, y: i32, effective: u32) -> ButtonPressEvent {
        ButtonPressEvent {
            detail,
            event_x: px(x),
            event_y: px(y),
            mods: ModifierInfo {
                effective,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn scroll_motion(x: i32, y: i32, number: u16, value: i32) -> ButtonPressEvent {
        ButtonPressEvent {
            event_x: px(x),
            event_y: px(y),
            valuator_mask: vec![1 << number],
            axisvalues: vec![units(value)],
            ..Default::default()
        }
    }

    fn scroll_motion_from(
        sourceid: DeviceId,
        x: i32,
        y: i32,
        number: u16,
        value: i32,
    ) -> ButtonPressEvent {
        ButtonPressEvent {
            sourceid,
            event_x: px(x),
            event_y: px(y),
            valuator_mask: vec![1 << number],
            axisvalues: vec![units(value)],
            ..Default::default()
        }
    }

    #[test]
    fn button_press_and_release_track_state() {
        let mut reducer = PointerEventReducer::default();

        let down = reducer.reduce(1.0, &Event::XinputButtonPress(button_event(1, 3, 4)), 1);
        let PointerEvent::Down(down) = &down[0] else {
            panic!("expected a button-down event");
        };
        assert_eq!(down.button, Some(PointerButton::Primary));
        assert_eq!(down.state.position, PhysicalPosition::new(3.0, 4.0));
        assert_eq!(down.state.count, 1);
        assert_eq!(down.state.pressure, ACTIVE_PRESSURE);

        let up = reducer.reduce(1.0, &Event::XinputButtonRelease(button_event(1, 3, 4)), 2);
        let PointerEvent::Up(up) = &up[0] else {
            panic!("expected a button-up event");
        };
        assert_eq!(up.button, Some(PointerButton::Primary));
        assert_eq!(up.state.pressure, RELEASED_PRESSURE);
    }

    #[test]
    fn chorded_release_keeps_pressure_until_last_button() {
        let mut reducer = PointerEventReducer::default();
        let _ = reducer.reduce(1.0, &Event::XinputButtonPress(button_event(1, 0, 0)), 1);
        let _ = reducer.reduce(1.0, &Event::XinputButtonPress(button_event(3, 0, 0)), 2);
        let mid = reducer.reduce(1.0, &Event::XinputButtonRelease(button_event(1, 0, 0)), 3);
        let PointerEvent::Up(up) = &mid[0] else {
            panic!("expected up");
        };
        assert_eq!(up.state.pressure, ACTIVE_PRESSURE);
        assert!(up.state.buttons.contains(PointerButton::Secondary));

        let last = reducer.reduce(1.0, &Event::XinputButtonRelease(button_event(3, 0, 0)), 4);
        let PointerEvent::Up(up) = &last[0] else {
            panic!("expected up");
        };
        assert_eq!(up.state.pressure, RELEASED_PRESSURE);
    }

    #[test]
    fn emulated_button_events_are_ignored() {
        let mut reducer = PointerEventReducer::default();
        let event = ButtonPressEvent {
            detail: 4,
            flags: PointerEventFlags::POINTER_EMULATED,
            ..Default::default()
        };
        assert!(
            reducer
                .reduce(1.0, &Event::XinputButtonPress(event), 1)
                .is_empty()
        );
    }

    #[test]
    fn non_emulated_wheel_button_becomes_scroll_not_down() {
        let mut reducer = PointerEventReducer::default();
        let events = reducer.reduce(1.0, &Event::XinputButtonPress(button_event(5, 1, 1)), 1);
        assert_eq!(events.len(), 1);
        let PointerEvent::Scroll(scroll) = &events[0] else {
            panic!("expected a scroll event, got {:?}", events[0]);
        };
        assert_eq!(scroll.delta, ScrollDelta::LineDelta(0.0, -1.0));

        assert!(
            reducer
                .reduce(1.0, &Event::XinputButtonRelease(button_event(5, 1, 1)), 2)
                .is_empty()
        );
    }

    #[test]
    fn unknown_button_detail_emits_nothing() {
        let mut reducer = PointerEventReducer::default();
        assert!(
            reducer
                .reduce(1.0, &Event::XinputButtonPress(button_event(99, 0, 0)), 1)
                .is_empty()
        );
    }

    #[test]
    fn modifiers_propagate_into_pointer_state() {
        let mut reducer = PointerEventReducer::default();
        let down = reducer.reduce(
            1.0,
            &Event::XinputButtonPress(button_event_mods(1, 0, 0, 0x1 | 0x4)),
            1,
        );
        let PointerEvent::Down(down) = &down[0] else {
            panic!("expected a button-down event");
        };
        assert!(down.state.modifiers.shift());
        assert!(down.state.modifiers.ctrl());
        assert!(!down.state.modifiers.alt());
        assert_eq!(down.state.modifiers, Modifiers::SHIFT | Modifiers::CONTROL);
    }

    #[test]
    fn motion_emits_move_with_position() {
        let mut reducer = PointerEventReducer::default();
        let events = reducer.reduce(1.0, &Event::XinputMotion(button_event(0, 5, 6)), 1);
        let PointerEvent::Move(update) = &events[0] else {
            panic!("expected a move event");
        };
        assert_eq!(update.current.position, PhysicalPosition::new(5.0, 6.0));
    }

    #[test]
    fn repeated_press_at_same_spot_counts_up() {
        let mut reducer = PointerEventReducer::default();
        let _ = reducer.reduce(1.0, &Event::XinputButtonPress(button_event(1, 3, 4)), 1);
        let _ = reducer.reduce(1.0, &Event::XinputButtonRelease(button_event(1, 3, 4)), 2);
        let second = reducer.reduce(1.0, &Event::XinputButtonPress(button_event(1, 3, 4)), 3);
        let PointerEvent::Down(down) = &second[0] else {
            panic!("expected a button-down event");
        };
        assert_eq!(down.state.count, 2);
    }

    #[test]
    fn left_then_right_click_are_not_a_double_click() {
        let mut reducer = PointerEventReducer::default();
        let _ = reducer.reduce(1.0, &Event::XinputButtonPress(button_event(1, 3, 4)), 1);
        let _ = reducer.reduce(1.0, &Event::XinputButtonRelease(button_event(1, 3, 4)), 2);
        let right = reducer.reduce(1.0, &Event::XinputButtonPress(button_event(3, 3, 4)), 3);
        let PointerEvent::Down(down) = &right[0] else {
            panic!("expected a button-down event");
        };
        assert_eq!(down.button, Some(PointerButton::Secondary));
        assert_eq!(down.state.count, 1);
    }

    #[test]
    fn scroll_valuator_baseline_then_delta() {
        let mut reducer = PointerEventReducer::default();
        reducer.set_scroll_axes(&[ScrollAxis {
            number: 3,
            increment: 1.0,
            kind: ScrollAxisKind::Vertical,
        }]);

        let first = reducer.reduce(1.0, &Event::XinputMotion(scroll_motion(10, 10, 3, 5)), 1);
        assert!(
            !first
                .iter()
                .any(|event| matches!(event, PointerEvent::Scroll(_)))
        );

        let second = reducer.reduce(1.0, &Event::XinputMotion(scroll_motion(10, 10, 3, 8)), 2);
        assert!(
            !second
                .iter()
                .any(|event| matches!(event, PointerEvent::Move(_)))
        );
        let scroll = second
            .iter()
            .find_map(|event| match event {
                PointerEvent::Scroll(scroll) => Some(scroll),
                _ => None,
            })
            .expect("a scroll event");
        assert_eq!(scroll.delta, ScrollDelta::LineDelta(0.0, -3.0));
    }

    #[test]
    fn two_sourceids_keep_independent_scroll_baselines() {
        let mut reducer = PointerEventReducer::default();
        reducer.set_scroll_axes(&[ScrollAxis {
            number: 3,
            increment: 1.0,
            kind: ScrollAxisKind::Vertical,
        }]);

        let _ = reducer.reduce(
            1.0,
            &Event::XinputMotion(scroll_motion_from(12, 0, 0, 3, 100)),
            1,
        );
        let _ = reducer.reduce(
            1.0,
            &Event::XinputMotion(scroll_motion_from(13, 0, 0, 3, 0)),
            2,
        );
        let mouse = reducer.reduce(
            1.0,
            &Event::XinputMotion(scroll_motion_from(13, 0, 0, 3, 2)),
            3,
        );
        let scroll = mouse
            .iter()
            .find_map(|event| match event {
                PointerEvent::Scroll(scroll) => Some(scroll),
                _ => None,
            })
            .expect("mouse scroll");
        assert_eq!(scroll.delta, ScrollDelta::LineDelta(0.0, -2.0));

        let pad = reducer.reduce(
            1.0,
            &Event::XinputMotion(scroll_motion_from(12, 0, 0, 3, 105)),
            4,
        );
        let scroll = pad
            .iter()
            .find_map(|event| match event {
                PointerEvent::Scroll(scroll) => Some(scroll),
                _ => None,
            })
            .expect("touchpad scroll");
        assert_eq!(scroll.delta, ScrollDelta::LineDelta(0.0, -5.0));
    }

    #[test]
    fn precision_sub_increment_scroll_is_fractional() {
        let mut reducer = PointerEventReducer::default();
        reducer.set_scroll_axes(&[ScrollAxis {
            number: 3,
            increment: 8.0,
            kind: ScrollAxisKind::Vertical,
        }]);

        let _ = reducer.reduce(1.0, &Event::XinputMotion(scroll_motion(0, 0, 3, 0)), 1);
        let quarter = reducer.reduce(1.0, &Event::XinputMotion(scroll_motion(0, 0, 3, 2)), 2);
        let scroll = quarter
            .iter()
            .find_map(|event| match event {
                PointerEvent::Scroll(scroll) => Some(scroll),
                _ => None,
            })
            .expect("a scroll event");
        assert_eq!(scroll.delta, ScrollDelta::LineDelta(0.0, -0.25));

        let eighth = reducer.reduce(1.0, &Event::XinputMotion(scroll_motion(0, 0, 3, 3)), 3);
        let scroll = eighth
            .iter()
            .find_map(|event| match event {
                PointerEvent::Scroll(scroll) => Some(scroll),
                _ => None,
            })
            .expect("a scroll event");
        assert_eq!(scroll.delta, ScrollDelta::LineDelta(0.0, -0.125));
    }

    #[test]
    fn horizontal_scroll_uses_x_and_respects_sign() {
        let mut reducer = PointerEventReducer::default();
        reducer.set_scroll_axes(&[ScrollAxis {
            number: 2,
            increment: 2.0,
            kind: ScrollAxisKind::Horizontal,
        }]);

        let _ = reducer.reduce(1.0, &Event::XinputMotion(scroll_motion(0, 0, 2, 10)), 1);
        let events = reducer.reduce(1.0, &Event::XinputMotion(scroll_motion(0, 0, 2, 6)), 2);
        let scroll = events
            .iter()
            .find_map(|event| match event {
                PointerEvent::Scroll(scroll) => Some(scroll),
                _ => None,
            })
            .expect("a scroll event");
        assert_eq!(scroll.delta, ScrollDelta::LineDelta(2.0, 0.0));
    }

    #[test]
    fn enter_and_leave_translate_and_seed_position() {
        let mut reducer = PointerEventReducer::default();
        let enter = EnterEvent {
            event_x: px(7),
            event_y: px(8),
            ..Default::default()
        };
        let events = reducer.reduce(1.0, &Event::XinputEnter(enter), 1);
        assert!(matches!(&events[0], PointerEvent::Enter(_)));

        reducer.set_scroll_axes(&[ScrollAxis {
            number: 3,
            increment: 1.0,
            kind: ScrollAxisKind::Vertical,
        }]);
        let after = reducer.reduce(1.0, &Event::XinputMotion(scroll_motion(7, 8, 3, 0)), 2);
        assert!(
            !after
                .iter()
                .any(|event| matches!(event, PointerEvent::Move(_)))
        );

        let leave = EnterEvent {
            event_x: px(7),
            event_y: px(8),
            ..Default::default()
        };
        let events = reducer.reduce(1.0, &Event::XinputLeave(leave), 3);
        assert!(matches!(events.last(), Some(PointerEvent::Leave(_))));
    }

    #[test]
    fn leave_with_held_button_emits_synthetic_up() {
        let mut reducer = PointerEventReducer::default();
        let _ = reducer.reduce(1.0, &Event::XinputButtonPress(button_event(1, 0, 0)), 1);
        assert!(
            reducer
                .primary_state
                .buttons
                .contains(PointerButton::Primary)
        );

        let leave = EnterEvent {
            event_x: px(0),
            event_y: px(0),
            sourceid: 9,
            ..Default::default()
        };
        let events = reducer.reduce(1.0, &Event::XinputLeave(leave), 2);
        assert!(
            matches!(
                &events[0],
                PointerEvent::Up(up) if up.button == Some(PointerButton::Primary)
            ),
            "expected synthetic Up first, got {:?}",
            events[0]
        );
        assert!(matches!(events.last(), Some(PointerEvent::Leave(_))));
        assert!(reducer.primary_state.buttons.is_empty());
    }

    #[test]
    fn pointer_is_primary_and_carries_sourceid() {
        let mut reducer = PointerEventReducer::default();
        let event = ButtonPressEvent {
            detail: 1,
            sourceid: 17,
            event_x: px(1),
            event_y: px(2),
            ..Default::default()
        };
        let down = reducer.reduce(1.0, &Event::XinputButtonPress(event), 1);
        let PointerEvent::Down(down) = &down[0] else {
            panic!("expected down");
        };
        assert!(down.pointer.is_primary_pointer());
        assert_eq!(
            down.pointer.persistent_device_id,
            PersistentDeviceId::new(17)
        );
        assert_eq!(down.pointer.pointer_type, PointerType::Mouse);
    }

    #[test]
    #[should_panic(expected = "monotonic")]
    fn non_monotonic_time_panics_in_debug() {
        let mut reducer = PointerEventReducer::default();
        let _ = reducer.reduce(1.0, &Event::XinputMotion(button_event(0, 1, 1)), 10);
        let _ = reducer.reduce(1.0, &Event::XinputMotion(button_event(0, 2, 2)), 5);
    }
}
