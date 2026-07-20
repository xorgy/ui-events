// Copyright 2026 the UI Events Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Reduces XInput 2 touch events into [`PointerEvent`]s.
//!
//! Keep one [`TouchEventReducer`] per touch device.
//! `TouchBegin` / `Update` / `End` map to Down / Move / Up with
//! [`PointerType::Touch`] and no button.
//!
//! # Pointer identity and primary
//!
//! Each contact keeps a stable [`PointerId`]:
//!
//! - first begin while none are active claims [`PointerId::PRIMARY`];
//! - others use `touch_id + 2` (id `1` is reserved for primary);
//! - `sourceid` is [`PointerInfo::persistent_device_id`] when non-zero;
//! - survivors are not promoted when primary lifts;
//! - after primary ends with contacts still down, nothing is primary until all
//!   contacts end (a later begin while survivors remain is non-primary).
//!
//! That last rule follows Pointer Events multi-touch primary behavior.
//!
//! Duplicate `TouchBegin` for an active tracking id is ignored.
//! Modifiers come from the event's effective mask.
//!
//! [`PointerType::Touch`]: ui_events::pointer::PointerType::Touch
//! [Pointer Events]: https://www.w3.org/TR/pointerevents3/#the-primary-pointer

use alloc::{vec, vec::Vec};

use ui_events::pointer::{
    PersistentDeviceId, PointerButtonEvent, PointerEvent, PointerId, PointerInfo, PointerState,
    PointerType, PointerUpdate,
};
use x11rb::protocol::Event;
use x11rb::protocol::xinput::{DeviceId, TouchBeginEvent};

use crate::mapping;
use crate::tap::TapCounter;

/// Pressure while a contact is down (XInput 2 does not report pressure).
const ACTIVE_PRESSURE: f32 = 0.5;
/// Pressure after release.
const RELEASED_PRESSURE: f32 = 0.0;
/// Offset so non-primary ids never alias [`PointerId::PRIMARY`] (`1`).
const POINTER_ID_OFFSET: u64 = 2;

/// Active contact and its assigned identity.
#[derive(Clone, Copy, Debug)]
struct TouchContact {
    touch_id: u32,
    pointer: PointerInfo,
}

/// Reduces an XInput 2 touch event stream into [`PointerEvent`]s.
///
/// One reducer per touch device.
/// See the [module documentation](self).
#[derive(Debug, Default)]
pub struct TouchEventReducer {
    contacts: Vec<TouchContact>,
    primary_touch_id: Option<u32>,
    counter: TapCounter,
    last_seen_time: Option<u64>,
}

impl TouchEventReducer {
    /// Reduce a decoded [`Event`] into zero or one touch [`PointerEvent`].
    ///
    /// `TouchBegin` / `Update` / `End` become Down / Move / Up. Other events are empty.
    /// `scale_factor` and host-clock `time` (ns) are stamped on every
    /// [`PointerState`]; `time` must not regress.
    pub fn reduce(&mut self, scale_factor: f64, event: &Event, time: u64) -> Vec<PointerEvent> {
        self.check_time_monotonic(time);
        match event {
            Event::XinputTouchBegin(event) => self.begin(event, time, scale_factor),
            Event::XinputTouchUpdate(event) => self.update(event, time, scale_factor),
            Event::XinputTouchEnd(event) => self.end(event, time, scale_factor),
            _ => Vec::new(),
        }
    }

    /// Establish a contact and emit Down.
    fn begin(
        &mut self,
        event: &TouchBeginEvent,
        time: u64,
        scale_factor: f64,
    ) -> Vec<PointerEvent> {
        let touch_id = event.detail;
        if self.contacts.iter().any(|c| c.touch_id == touch_id) {
            return Vec::new();
        }
        let is_primary = self.contacts.is_empty();
        if is_primary {
            self.primary_touch_id = Some(touch_id);
        }
        let pointer = touch_pointer_info(touch_id, is_primary, event.sourceid);
        self.contacts.push(TouchContact { touch_id, pointer });
        let state = contact_state(event, time, ACTIVE_PRESSURE, scale_factor);
        vec![self.counter.attach_count(
            state.scale_factor,
            PointerEvent::Down(PointerButtonEvent {
                button: None,
                pointer,
                state,
            }),
        )]
    }

    /// Emit Move for an active contact.
    fn update(
        &mut self,
        event: &TouchBeginEvent,
        time: u64,
        scale_factor: f64,
    ) -> Vec<PointerEvent> {
        let Some(pointer) = self.pointer_for(event.detail) else {
            return Vec::new();
        };
        let state = contact_state(event, time, ACTIVE_PRESSURE, scale_factor);
        vec![self.counter.attach_count(
            state.scale_factor,
            PointerEvent::Move(PointerUpdate {
                pointer,
                current: state,
                coalesced: Vec::new(),
                predicted: Vec::new(),
            }),
        )]
    }

    /// Emit Up and drop the contact.
    fn end(&mut self, event: &TouchBeginEvent, time: u64, scale_factor: f64) -> Vec<PointerEvent> {
        let touch_id = event.detail;
        let Some(index) = self.contacts.iter().position(|c| c.touch_id == touch_id) else {
            return Vec::new();
        };
        let pointer = self.contacts.remove(index).pointer;
        if self.primary_touch_id == Some(touch_id) {
            self.primary_touch_id = None;
        }
        let state = contact_state(event, time, RELEASED_PRESSURE, scale_factor);
        vec![self.counter.attach_count(
            state.scale_factor,
            PointerEvent::Up(PointerButtonEvent {
                button: None,
                pointer,
                state,
            }),
        )]
    }

    /// Look up the stable [`PointerInfo`] of an active contact.
    fn pointer_for(&self, touch_id: u32) -> Option<PointerInfo> {
        self.contacts
            .iter()
            .find(|c| c.touch_id == touch_id)
            .map(|c| c.pointer)
    }

    /// Debug-assert that caller timestamps do not regress.
    fn check_time_monotonic(&mut self, time: u64) {
        if let Some(previous) = self.last_seen_time {
            debug_assert!(
                time >= previous,
                "TouchEventReducer::reduce timestamps must be monotonic nanoseconds"
            );
        }
        self.last_seen_time = Some(time);
    }
}

fn contact_state(
    event: &TouchBeginEvent,
    time: u64,
    pressure: f32,
    scale_factor: f64,
) -> PointerState {
    PointerState {
        time,
        position: mapping::position_from_fp1616(event.event_x, event.event_y),
        pressure,
        scale_factor,
        modifiers: mapping::modifiers_from_xi_mask(event.mods.effective),
        ..PointerState::default()
    }
}

fn touch_pointer_info(touch_id: u32, is_primary: bool, sourceid: DeviceId) -> PointerInfo {
    let pointer_id = if is_primary {
        Some(PointerId::PRIMARY)
    } else {
        PointerId::new(u64::from(touch_id).saturating_add(POINTER_ID_OFFSET))
    };
    PointerInfo {
        pointer_id,
        persistent_device_id: PersistentDeviceId::new(u64::from(sourceid)),
        pointer_type: PointerType::Touch,
    }
}

#[cfg(test)]
mod tests {
    use dpi::PhysicalPosition;
    use x11rb::protocol::xinput::ModifierInfo;

    use super::*;

    fn px(pixels: i32) -> i32 {
        pixels * 65536
    }

    fn touch_event(touch_id: u32, x: i32, y: i32) -> TouchBeginEvent {
        TouchBeginEvent {
            detail: touch_id,
            event_x: px(x),
            event_y: px(y),
            ..Default::default()
        }
    }

    fn touch_event_mods(touch_id: u32, x: i32, y: i32, effective: u32) -> TouchBeginEvent {
        TouchBeginEvent {
            detail: touch_id,
            event_x: px(x),
            event_y: px(y),
            mods: ModifierInfo {
                effective,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn touch_begin_is_primary_with_position_and_pressure() {
        let mut reducer = TouchEventReducer::default();
        let events = reducer.reduce(1.0, &Event::XinputTouchBegin(touch_event(7, 3, 4)), 1);

        assert_eq!(events.len(), 1);
        let PointerEvent::Down(down) = &events[0] else {
            panic!("expected a touch down, got {:?}", events[0]);
        };
        assert!(down.pointer.is_primary_pointer());
        assert_eq!(down.pointer.pointer_type, PointerType::Touch);
        assert_eq!(down.button, None);
        assert_eq!(down.state.position, PhysicalPosition::new(3.0, 4.0));
        assert_eq!(down.state.pressure, ACTIVE_PRESSURE);
        assert_eq!(down.state.count, 1);
    }

    #[test]
    fn update_moves_the_contact_and_keeps_identity() {
        let mut reducer = TouchEventReducer::default();
        let _ = reducer.reduce(1.0, &Event::XinputTouchBegin(touch_event(7, 0, 0)), 1);
        let events = reducer.reduce(1.0, &Event::XinputTouchUpdate(touch_event(7, 5, 6)), 2);

        let PointerEvent::Move(update) = &events[0] else {
            panic!("expected a touch move, got {:?}", events[0]);
        };
        assert!(update.pointer.is_primary_pointer());
        assert_eq!(update.current.position, PhysicalPosition::new(5.0, 6.0));
    }

    #[test]
    fn concurrent_contacts_are_offset_past_primary() {
        let mut reducer = TouchEventReducer::default();
        let first = reducer.reduce(1.0, &Event::XinputTouchBegin(touch_event(7, 0, 0)), 1);
        let second = reducer.reduce(1.0, &Event::XinputTouchBegin(touch_event(9, 0, 0)), 2);

        let PointerEvent::Down(primary) = &first[0] else {
            panic!("expected a touch down");
        };
        let PointerEvent::Down(secondary) = &second[0] else {
            panic!("expected a touch down");
        };
        assert!(primary.pointer.is_primary_pointer());
        assert!(!secondary.pointer.is_primary_pointer());
        assert_eq!(secondary.pointer.pointer_id, PointerId::new(11));
    }

    #[test]
    fn ending_a_contact_forgets_it_and_reports_up() {
        let mut reducer = TouchEventReducer::default();
        let _ = reducer.reduce(1.0, &Event::XinputTouchBegin(touch_event(7, 0, 0)), 1);
        let up = reducer.reduce(1.0, &Event::XinputTouchEnd(touch_event(7, 0, 0)), 2);

        let PointerEvent::Up(event) = &up[0] else {
            panic!("expected a touch up, got {:?}", up[0]);
        };
        assert!(event.pointer.is_primary_pointer());
        assert_eq!(event.state.pressure, RELEASED_PRESSURE);

        let next = reducer.reduce(1.0, &Event::XinputTouchBegin(touch_event(20, 0, 0)), 3);
        assert!(next[0].is_primary_pointer());
    }

    #[test]
    fn primary_lifting_does_not_promote_a_survivor() {
        let mut reducer = TouchEventReducer::default();
        let _ = reducer.reduce(1.0, &Event::XinputTouchBegin(touch_event(7, 0, 0)), 1);
        let second = reducer.reduce(1.0, &Event::XinputTouchBegin(touch_event(9, 0, 0)), 2);
        let PointerEvent::Down(secondary) = &second[0] else {
            panic!("expected a touch down");
        };
        let secondary_id = secondary.pointer.pointer_id;

        let _ = reducer.reduce(1.0, &Event::XinputTouchEnd(touch_event(7, 0, 0)), 3);
        let moved = reducer.reduce(1.0, &Event::XinputTouchUpdate(touch_event(9, 1, 1)), 4);
        let PointerEvent::Move(update) = &moved[0] else {
            panic!("expected a touch move");
        };
        assert!(!update.pointer.is_primary_pointer());
        assert_eq!(update.pointer.pointer_id, secondary_id);
    }

    #[test]
    fn third_contact_while_survivor_remains_is_not_primary() {
        let mut reducer = TouchEventReducer::default();
        let _ = reducer.reduce(1.0, &Event::XinputTouchBegin(touch_event(7, 0, 0)), 1);
        let _ = reducer.reduce(1.0, &Event::XinputTouchBegin(touch_event(9, 0, 0)), 2);
        let _ = reducer.reduce(1.0, &Event::XinputTouchEnd(touch_event(7, 0, 0)), 3);
        let third = reducer.reduce(1.0, &Event::XinputTouchBegin(touch_event(11, 0, 0)), 4);
        let PointerEvent::Down(down) = &third[0] else {
            panic!("expected a touch down");
        };
        assert!(!down.pointer.is_primary_pointer());
        assert_eq!(
            down.pointer.pointer_id,
            PointerId::new(u64::from(11_u32) + POINTER_ID_OFFSET)
        );

        let _ = reducer.reduce(1.0, &Event::XinputTouchEnd(touch_event(9, 0, 0)), 5);
        let _ = reducer.reduce(1.0, &Event::XinputTouchEnd(touch_event(11, 0, 0)), 6);
        let fresh = reducer.reduce(1.0, &Event::XinputTouchBegin(touch_event(20, 0, 0)), 7);
        assert!(fresh[0].is_primary_pointer());
    }

    #[test]
    fn two_nearby_simultaneous_touches_both_start_at_count_one() {
        let mut reducer = TouchEventReducer::default();
        let first = reducer.reduce(1.0, &Event::XinputTouchBegin(touch_event(1, 10, 10)), 1);
        let second = reducer.reduce(1.0, &Event::XinputTouchBegin(touch_event(2, 12, 10)), 2);

        let PointerEvent::Down(a) = &first[0] else {
            panic!("expected down");
        };
        let PointerEvent::Down(b) = &second[0] else {
            panic!("expected down");
        };
        assert_eq!(a.state.count, 1);
        assert_eq!(b.state.count, 1);

        let up_a = reducer.reduce(1.0, &Event::XinputTouchEnd(touch_event(1, 10, 10)), 3);
        let up_b = reducer.reduce(1.0, &Event::XinputTouchEnd(touch_event(2, 12, 10)), 4);
        let PointerEvent::Up(ua) = &up_a[0] else {
            panic!("expected up");
        };
        let PointerEvent::Up(ub) = &up_b[0] else {
            panic!("expected up");
        };
        assert_eq!(ua.state.count, 1);
        assert_eq!(ub.state.count, 1);
    }

    #[test]
    fn modifiers_propagate_into_touch_state() {
        let mut reducer = TouchEventReducer::default();
        let events = reducer.reduce(
            1.0,
            &Event::XinputTouchBegin(touch_event_mods(1, 0, 0, 0x1)),
            1,
        );
        let PointerEvent::Down(down) = &events[0] else {
            panic!("expected down");
        };
        assert!(down.state.modifiers.shift());
    }

    #[test]
    fn update_for_unknown_contact_is_ignored() {
        let mut reducer = TouchEventReducer::default();
        assert!(
            reducer
                .reduce(1.0, &Event::XinputTouchUpdate(touch_event(3, 1, 1)), 1)
                .is_empty()
        );
    }

    #[test]
    fn duplicate_touch_begin_is_ignored() {
        let mut reducer = TouchEventReducer::default();
        let first = reducer.reduce(1.0, &Event::XinputTouchBegin(touch_event(7, 0, 0)), 1);
        assert_eq!(first.len(), 1);
        assert!(
            reducer
                .reduce(1.0, &Event::XinputTouchBegin(touch_event(7, 1, 1)), 2)
                .is_empty()
        );
        let moved = reducer.reduce(1.0, &Event::XinputTouchUpdate(touch_event(7, 2, 2)), 3);
        assert!(moved[0].is_primary_pointer());
    }

    #[test]
    fn touch_carries_sourceid_as_persistent_device() {
        let mut reducer = TouchEventReducer::default();
        let event = TouchBeginEvent {
            detail: 3,
            sourceid: 42,
            event_x: px(0),
            event_y: px(0),
            ..Default::default()
        };
        let events = reducer.reduce(1.0, &Event::XinputTouchBegin(event), 1);
        let PointerEvent::Down(down) = &events[0] else {
            panic!("expected down");
        };
        assert_eq!(
            down.pointer.persistent_device_id,
            PersistentDeviceId::new(42)
        );
    }

    #[test]
    #[should_panic(expected = "monotonic")]
    fn non_monotonic_time_panics_in_debug() {
        let mut reducer = TouchEventReducer::default();
        let _ = reducer.reduce(1.0, &Event::XinputTouchBegin(touch_event(1, 0, 0)), 10);
        let _ = reducer.reduce(1.0, &Event::XinputTouchBegin(touch_event(2, 0, 0)), 5);
    }
}
