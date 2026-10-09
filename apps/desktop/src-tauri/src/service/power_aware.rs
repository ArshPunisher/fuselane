//! Battery aware (B9.10). On battery and low, Fuselane does what the setting
//! says: keep going, leave out tethered phones (which charge from the
//! computer), or pause downloads until it's plugged in. Downloads paused for
//! the battery carry on by themselves once it's charging.

use std::sync::Arc;

use fuselane_core::{Event, Status};

use super::{Service, lock};
use crate::automation::LowBattery;
use crate::battery::Battery;

impl Service {
    /// Low battery (unplugged) right now, as last read.
    pub fn battery_low(&self) -> bool {
        lock(&self.battery).is_some_and(|b| b.low())
    }

    pub(super) fn low_battery_mode(&self) -> LowBattery {
        lock(&self.automation).low_battery
    }

    /// Phones are left out now (low battery and the setting asks for it).
    pub(super) fn phone_left_out(&self) -> bool {
        self.battery_low() && self.low_battery_mode() == LowBattery::LeaveOutPhone
    }

    /// Downloads may start now as far as the battery goes.
    pub(super) fn battery_allows(&self) -> bool {
        !(self.battery_low() && self.low_battery_mode() == LowBattery::Pause)
    }

    /// Called every few seconds with a fresh reading (None: no battery).
    pub fn tick_battery(self: &Arc<Self>, now: Option<Battery>) {
        let was_low = self.battery_low();
        *lock(&self.battery) = now;
        let low = self.battery_low();
        match (was_low, low) {
            (false, true) => match self.low_battery_mode() {
                LowBattery::KeepGoing => {}
                LowBattery::Pause => {
                    let mut paused = lock(&self.battery_paused);
                    for (id, r) in lock(&self.running).iter() {
                        if paused.insert(*id) {
                            r.cancel.cancel();
                        }
                    }
                }
                // Restart running downloads at once, without the phone.
                LowBattery::LeaveOutPhone => {
                    for r in lock(&self.running).values_mut() {
                        r.replan = true;
                        r.cancel.cancel();
                    }
                }
            },
            (true, false) => {
                let ids: Vec<i64> = lock(&self.battery_paused).drain().collect();
                for id in ids {
                    if self.store.get(id).is_ok_and(|j| j.status == Status::Paused) {
                        let _ = self.store.apply(id, Event::Resume, None);
                    }
                }
                // Phones come back for downloads that start from now on.
                self.publish_jobs();
                self.pump();
            }
            _ => {}
        }
    }

    /// Leaves tethered phones out of `all` while the battery asks for it, unless
    /// nothing else is left.
    pub(super) fn without_phone_if_low(
        &self,
        all: Vec<fuselane_netif::Interface>,
    ) -> Vec<fuselane_netif::Interface> {
        if !self.phone_left_out() {
            return all;
        }
        let rest: Vec<_> = all
            .iter()
            .filter(|i| {
                !matches!(
                    i.kind,
                    fuselane_netif::Kind::Tether | fuselane_netif::Kind::Cellular
                )
            })
            .cloned()
            .collect();
        if rest.is_empty() { all } else { rest }
    }
}
