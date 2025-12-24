// Shortwave - recording_state.rs
// Copyright (C) 2024-2025  Felix Häcker <haeckerfelix@gnome.org>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use gettextrs::gettext;
use gtk::glib;
use gtk::glib::Enum;

#[derive(Display, Copy, Debug, Clone, EnumString, Eq, PartialEq, Enum)]
#[repr(u32)]
#[enum_type(name = "SwRecordingState")]
#[derive(Default)]
pub enum SwRecordingState {
    // Idle
    #[default]
    IdleDisabled,
    IdleIgnoredTrack,
    IdleIncomplete,

    // Recording
    Recording,
    Recorded,
    RecordedReachedMaxDuration,

    // Discarded
    DiscardedBelowMinDuration,
    DiscardedCancelled,
}

impl SwRecordingState {
    pub fn include_in_past_tracks(&self) -> bool {
        *self != Self::IdleIgnoredTrack && *self != Self::DiscardedBelowMinDuration
    }

    pub fn is_recorded(&self) -> bool {
        *self == Self::Recorded || *self == Self::RecordedReachedMaxDuration
    }

    pub fn title(&self) -> String {
        match self {
            SwRecordingState::IdleDisabled => gettext("Not Recorded"),
            SwRecordingState::IdleIgnoredTrack => gettext("Ignored Track"),
            SwRecordingState::IdleIncomplete => gettext("Not Recorded"),

            SwRecordingState::Recording => gettext("Recording…"),
            SwRecordingState::Recorded => gettext("Recorded"),
            SwRecordingState::RecordedReachedMaxDuration => gettext("Recorded"),

            SwRecordingState::DiscardedBelowMinDuration => gettext("Below Threshold"),
            SwRecordingState::DiscardedCancelled => gettext("Cancelled"),
        }
    }

    pub fn description(&self) -> String {
        match self {
            SwRecordingState::IdleDisabled => gettext("Recording is deactivated in preferences"),
            SwRecordingState::IdleIgnoredTrack => {
                gettext("The track contains a word that is on the ignore list")
            }
            SwRecordingState::IdleIncomplete => {
                gettext("The track wasn't played from the beginning, so it can't be fully recorded")
            }
            SwRecordingState::Recording => {
                gettext("The track will be recorded until a new track gets played")
            }
            SwRecordingState::Recorded => gettext("The track has been temporarily recorded"),
            SwRecordingState::RecordedReachedMaxDuration => {
                gettext("The maximum recording duration has been reached")
            }
            SwRecordingState::DiscardedBelowMinDuration => {
                gettext("The track has been discarded as the duration was below the set threshold")
            }
            SwRecordingState::DiscardedCancelled => gettext("Recording has been cancelled"),
        }
    }
}
