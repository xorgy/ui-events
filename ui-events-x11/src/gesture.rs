// Copyright 2026 the UI Events Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Reduces XInput 2 gesture events into [`PointerEvent::Gesture`]s.
//!
//! `GesturePinchBegin` / `Update` / `End` become incremental
//! [`PointerGesture::Pinch`] and [`PointerGesture::Rotate`] (same model as the
//! Wayland adapter).
//!
//! Scale is relative to gesture start (`1.0` at begin); each update emits
//! `current / previous - 1` via [`mapping::pinch_scale_fraction`].
//! `delta_angle` is a clockwise degree step, converted to radians for Rotate.
//!
//! Begin and end only reset the scale baseline.
//! Cancelled ends (`GESTURE_PINCH_CANCELLED`) are treated like normal ends.
//!
//! `GestureSwipe*` is ignored (use the pointer reducer for scroll).
//! Events use primary mouse identity, `sourceid` as
//! [`PointerInfo::persistent_device_id`], and the gesture center.
//!
//! [`PointerEvent::Gesture`]: ui_events::pointer::PointerEvent::Gesture
//! [`PointerGesture::Pinch`]: ui_events::pointer::PointerGesture::Pinch
//! [`PointerGesture::Rotate`]: ui_events::pointer::PointerGesture::Rotate
//! [`PointerInfo::persistent_device_id`]: ui_events::pointer::PointerInfo::persistent_device_id

use alloc::vec::Vec;

use ui_events::pointer::{
    PersistentDeviceId, PointerEvent, PointerGesture, PointerGestureEvent, PointerId, PointerInfo,
    PointerState, PointerType,
};
use x11rb::protocol::Event;
use x11rb::protocol::xinput::{DeviceId, GesturePinchBeginEvent};

use crate::mapping;

/// Reduces XInput 2 gesture events into [`PointerEvent::Gesture`]s.
///
/// One reducer per seat (or master pointer).
/// See the [module documentation](self).
#[derive(Debug)]
pub struct GestureEventReducer {
    last_scale: f64,
    state: PointerState,
    sourceid: DeviceId,
    last_seen_time: Option<u64>,
}

impl Default for GestureEventReducer {
    fn default() -> Self {
        Self {
            last_scale: 1.0,
            state: PointerState::default(),
            sourceid: 0,
            last_seen_time: None,
        }
    }
}

impl GestureEventReducer {
    /// Reduce a decoded [`Event`] into zero, one, or two gesture events.
    ///
    /// Only pinch begin/update/end translate.
    /// Begin and end emit nothing; update may emit Pinch then Rotate.
    /// `scale_factor` and host-clock `time` (ns) are stamped; `time` must not regress.
    pub fn reduce(&mut self, scale_factor: f64, event: &Event, time: u64) -> Vec<PointerEvent> {
        self.check_time_monotonic(time);
        self.state.time = time;
        self.state.scale_factor = scale_factor;

        match event {
            Event::XinputGesturePinchBegin(event) => {
                self.apply_common(event);
                self.last_scale = 1.0;
                Vec::new()
            }
            Event::XinputGesturePinchUpdate(event) => self.update(event),
            Event::XinputGesturePinchEnd(event) => {
                self.apply_common(event);
                self.last_scale = 1.0;
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    /// Incremental pinch and/or rotate.
    fn update(&mut self, event: &GesturePinchBeginEvent) -> Vec<PointerEvent> {
        self.apply_common(event);

        let scale = mapping::fp1616_to_f64(event.scale);
        let pinch = mapping::pinch_scale_fraction(self.last_scale, scale);
        if scale.is_finite() && scale > 0.0 {
            self.last_scale = scale;
        }

        let rotate =
            mapping::rotation_radians_from_degrees(mapping::fp1616_to_f64(event.delta_angle));

        let mut events = Vec::new();
        if pinch != 0.0 {
            events.push(self.gesture_event(PointerGesture::Pinch(pinch)));
        }
        if rotate != 0.0 {
            events.push(self.gesture_event(PointerGesture::Rotate(rotate)));
        }
        events
    }

    /// Stamp position, modifiers, and source from a gesture event.
    fn apply_common(&mut self, event: &GesturePinchBeginEvent) {
        self.sourceid = event.sourceid;
        self.state.position = mapping::position_from_fp1616(event.event_x, event.event_y);
        self.state.modifiers = mapping::modifiers_from_xi_mask(event.mods.effective);
    }

    fn gesture_event(&self, gesture: PointerGesture) -> PointerEvent {
        PointerEvent::Gesture(PointerGestureEvent {
            pointer: gesture_pointer(self.sourceid),
            gesture,
            state: self.state.clone(),
        })
    }

    fn check_time_monotonic(&mut self, time: u64) {
        if let Some(previous) = self.last_seen_time {
            debug_assert!(
                time >= previous,
                "GestureEventReducer::reduce timestamps must be monotonic nanoseconds"
            );
        }
        self.last_seen_time = Some(time);
    }
}

fn gesture_pointer(sourceid: DeviceId) -> PointerInfo {
    PointerInfo {
        pointer_id: Some(PointerId::PRIMARY),
        persistent_device_id: PersistentDeviceId::new(u64::from(sourceid)),
        pointer_type: PointerType::Mouse,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[expect(
        clippy::cast_possible_truncation,
        reason = "test helper packs small f64 values into FP1616"
    )]
    fn fp(v: f64) -> i32 {
        (v * 65536.0) as i32
    }

    fn pinch_event(scale: f64, delta_angle_deg: f64) -> GesturePinchBeginEvent {
        GesturePinchBeginEvent {
            scale: fp(scale),
            delta_angle: fp(delta_angle_deg),
            event_x: fp(10.0),
            event_y: fp(20.0),
            sourceid: 7,
            ..Default::default()
        }
    }

    fn pinch_fraction(event: &PointerEvent) -> f32 {
        match event {
            PointerEvent::Gesture(PointerGestureEvent {
                gesture: PointerGesture::Pinch(f),
                ..
            }) => *f,
            other => panic!("expected pinch, got {other:?}"),
        }
    }

    fn rotation_radians(event: &PointerEvent) -> f32 {
        match event {
            PointerEvent::Gesture(PointerGestureEvent {
                gesture: PointerGesture::Rotate(r),
                ..
            }) => *r,
            other => panic!("expected rotate, got {other:?}"),
        }
    }

    #[test]
    fn scale_change_emits_pinch_against_begin_baseline() {
        let mut reducer = GestureEventReducer::default();
        let _ = reducer.reduce(
            1.0,
            &Event::XinputGesturePinchBegin(pinch_event(1.0, 0.0)),
            1,
        );
        let events = reducer.reduce(
            1.0,
            &Event::XinputGesturePinchUpdate(pinch_event(1.1, 0.0)),
            2,
        );
        assert_eq!(events.len(), 1);
        assert!((pinch_fraction(&events[0]) - 0.1).abs() < 1e-4);
        assert!(events[0].is_primary_pointer());
    }

    #[test]
    fn successive_updates_are_incremental() {
        let mut reducer = GestureEventReducer::default();
        let _ = reducer.reduce(
            1.0,
            &Event::XinputGesturePinchBegin(pinch_event(1.0, 0.0)),
            1,
        );
        let first = reducer.reduce(
            1.0,
            &Event::XinputGesturePinchUpdate(pinch_event(1.1, 0.0)),
            2,
        );
        let second = reducer.reduce(
            1.0,
            &Event::XinputGesturePinchUpdate(pinch_event(1.21, 0.0)),
            3,
        );
        assert!((pinch_fraction(&first[0]) - 0.1).abs() < 1e-4);
        assert!((pinch_fraction(&second[0]) - 0.1).abs() < 1e-4);
    }

    #[test]
    fn rotation_is_clockwise_radians() {
        let mut reducer = GestureEventReducer::default();
        let events = reducer.reduce(
            1.0,
            &Event::XinputGesturePinchUpdate(pinch_event(1.0, 90.0)),
            1,
        );
        assert_eq!(events.len(), 1);
        assert!((rotation_radians(&events[0]) - core::f32::consts::FRAC_PI_2).abs() < 1e-4);
    }

    #[test]
    fn scale_and_rotation_emit_pinch_then_rotate() {
        let mut reducer = GestureEventReducer::default();
        let events = reducer.reduce(
            1.0,
            &Event::XinputGesturePinchUpdate(pinch_event(1.5, 30.0)),
            1,
        );
        assert_eq!(events.len(), 2);
        assert!((pinch_fraction(&events[0]) - 0.5).abs() < 1e-4);
        assert!((rotation_radians(&events[1]) - 30.0_f32.to_radians()).abs() < 1e-4);
    }

    #[test]
    fn end_resets_scale_baseline() {
        let mut reducer = GestureEventReducer::default();
        let _ = reducer.reduce(
            1.0,
            &Event::XinputGesturePinchUpdate(pinch_event(2.0, 0.0)),
            1,
        );
        let _ = reducer.reduce(1.0, &Event::XinputGesturePinchEnd(pinch_event(2.0, 0.0)), 2);
        let events = reducer.reduce(
            1.0,
            &Event::XinputGesturePinchUpdate(pinch_event(1.1, 0.0)),
            3,
        );
        assert!((pinch_fraction(&events[0]) - 0.1).abs() < 1e-4);
    }

    #[test]
    fn update_stamps_position_modifiers_and_source() {
        let mut reducer = GestureEventReducer::default();
        let mut event = pinch_event(1.1, 0.0);
        event.mods.effective = 0x1; // ShiftMask
        let events = reducer.reduce(2.0, &Event::XinputGesturePinchUpdate(event), 42);
        let PointerEvent::Gesture(g) = &events[0] else {
            panic!("expected gesture");
        };
        assert_eq!(g.state.time, 42);
        assert_eq!(g.state.scale_factor, 2.0);
        assert!((g.state.position.x - 10.0).abs() < 1e-6);
        assert!((g.state.position.y - 20.0).abs() < 1e-6);
        assert!(g.state.modifiers.shift());
        assert_eq!(g.pointer.persistent_device_id, PersistentDeviceId::new(7));
    }

    #[test]
    fn swipe_events_are_ignored() {
        let mut reducer = GestureEventReducer::default();
        assert!(
            reducer
                .reduce(1.0, &Event::XinputGestureSwipeUpdate(Default::default()), 1)
                .is_empty()
        );
    }

    #[test]
    #[should_panic(expected = "monotonic")]
    fn non_monotonic_time_panics_in_debug() {
        let mut reducer = GestureEventReducer::default();
        let _ = reducer.reduce(
            1.0,
            &Event::XinputGesturePinchUpdate(pinch_event(1.0, 0.0)),
            10,
        );
        let _ = reducer.reduce(
            1.0,
            &Event::XinputGesturePinchUpdate(pinch_event(1.0, 0.0)),
            5,
        );
    }
}
