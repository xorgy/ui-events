// Copyright 2026 the UI Events Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Multi-click / multi-tap counting for the pointer and touch reducers.
//!
//! [`TapCounter`] sets `count` on Down from time and space proximity of presses.
//!
//! A sequence continues only when:
//! - the previous press has been released (concurrent contacts do not chain);
//! - the new press is within the type-dependent slop and 500 ms of that release;
//! - buttoned pointers use the same button (left-then-right are two singles).

use alloc::vec::Vec;

use ui_events::pointer::{PointerButton, PointerEvent, PointerId, PointerType, PointerUpdate};

#[derive(Clone, Debug)]
struct TapState {
    pointer_id: Option<PointerId>,
    /// `None` for touch / buttonless downs.
    button: Option<PointerButton>,
    down_time: u64,
    /// Equal to `down_time` while held; chainable only after release.
    up_time: u64,
    count: u8,
    x: f64,
    y: f64,
}

impl TapState {
    fn is_released(&self) -> bool {
        self.down_time != self.up_time
    }
}

/// Fills multi-click / multi-tap `count` on events a reducer emits.
#[derive(Debug, Default)]
pub(crate) struct TapCounter {
    taps: Vec<TapState>,
}

impl TapCounter {
    pub(crate) fn attach_count(&mut self, scale_factor: f64, e: PointerEvent) -> PointerEvent {
        match e {
            PointerEvent::Down(mut event) => {
                let pointer_id = event.pointer.pointer_id;
                let position = event.state.position;
                let time = event.state.time;
                let button = event.button;

                let slop = match event.pointer.pointer_type {
                    PointerType::Touch => 12.0,
                    PointerType::Pen => 6.0,
                    // Circle inscribed in a 2px box, times SQRT_2, to match
                    // Windows box-slop roughly while using a radius.
                    _ => 2.0,
                } * core::f64::consts::SQRT_2
                    * scale_factor;

                if let Some(tap) = self.taps.iter_mut().find(|tap| {
                    if !tap.is_released() {
                        return false;
                    }
                    if tap.button != button {
                        return false;
                    }
                    let dx = (tap.x - position.x).abs();
                    let dy = (tap.y - position.y).abs();
                    (dx * dx + dy * dy).sqrt() < slop && (tap.up_time + 500_000_000) > time
                }) {
                    let count = tap.count.saturating_add(1);
                    event.state.count = count;
                    tap.count = count;
                    tap.pointer_id = pointer_id;
                    tap.button = button;
                    tap.down_time = time;
                    // Still held: up_time tracks down_time until release.
                    tap.up_time = time;
                    tap.x = position.x;
                    tap.y = position.y;
                } else {
                    let s = TapState {
                        pointer_id,
                        button,
                        down_time: time,
                        up_time: time,
                        count: 1,
                        x: position.x,
                        y: position.y,
                    };
                    if let Some(t) = self
                        .taps
                        .iter_mut()
                        .find(|state| state.pointer_id == pointer_id)
                    {
                        *t = s;
                    } else {
                        self.taps.push(s);
                    }
                    event.state.count = 1;
                };
                self.clear_expired(time);
                PointerEvent::Down(event)
            }
            PointerEvent::Up(mut event) => {
                let p_id = event.pointer.pointer_id;
                if let Some(tap) = self.taps.iter_mut().find(|state| state.pointer_id == p_id) {
                    tap.up_time = event.state.time;
                    event.state.count = tap.count;
                }
                PointerEvent::Up(event)
            }
            PointerEvent::Move(PointerUpdate {
                pointer,
                mut current,
                mut coalesced,
                mut predicted,
            }) => {
                if let Some(TapState { count, .. }) = self
                    .taps
                    .iter()
                    .find(
                        |TapState {
                             pointer_id,
                             down_time,
                             up_time,
                             ..
                         }| {
                            *pointer_id == pointer.pointer_id && down_time == up_time
                        },
                    )
                    .cloned()
                {
                    current.count = count;
                    for event in coalesced.iter_mut() {
                        event.count = count;
                    }
                    for event in predicted.iter_mut() {
                        event.count = count;
                    }
                    PointerEvent::Move(PointerUpdate {
                        pointer,
                        current,
                        coalesced,
                        predicted,
                    })
                } else {
                    PointerEvent::Move(PointerUpdate {
                        pointer,
                        current,
                        coalesced,
                        predicted,
                    })
                }
            }
            PointerEvent::Cancel(p) => {
                self.taps
                    .retain(|TapState { pointer_id, .. }| *pointer_id != p.pointer_id);
                PointerEvent::Cancel(p)
            }
            PointerEvent::Leave(p) => {
                self.taps
                    .retain(|TapState { pointer_id, .. }| *pointer_id != p.pointer_id);
                PointerEvent::Leave(p)
            }
            e
            @ (PointerEvent::Enter(..) | PointerEvent::Scroll(..) | PointerEvent::Gesture(..)) => e,
        }
    }

    /// Clear expired taps.
    ///
    /// `t` is the timestamp of the last received event, in the caller's
    /// monotonic clock domain shared by every event the reducer sees.
    fn clear_expired(&mut self, t: u64) {
        self.taps.retain(
            |TapState {
                 down_time, up_time, ..
             }| { down_time == up_time || (up_time + 500_000_000) > t },
        );
    }
}
