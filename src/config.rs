// SPDX-License-Identifier: MPL-2.0

use cosmic::cosmic_config::{self, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry};

#[derive(Debug, Clone, CosmicConfigEntry, Eq, PartialEq)]
#[version = 1]
pub struct Config {
    pub work_mins: u32,
    pub short_break_mins: u32,
    pub long_break_mins: u32,
    pub long_break_interval: u32,
    pub notifications: bool,
    pub sound: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            work_mins: 25,
            short_break_mins: 5,
            long_break_mins: 15,
            long_break_interval: 4,
            notifications: true,
            sound: true,
        }
    }
}
