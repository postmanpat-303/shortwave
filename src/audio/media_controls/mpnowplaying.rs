// Shortwave - mpnowplaying.rs
// Copyright (C) 2026  Felix Häcker <haeckerfelix@gnome.org>
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

use std::ptr::NonNull;
use std::rc::Rc;

use block2::RcBlock;
use glib::clone;
use gtk::glib;
use objc2::AnyThread;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::NSImage;
use objc2_foundation::{NSDictionary, NSNumber, NSString, NSURL};
use objc2_media_player::{
    MPMediaItemArtwork, MPMediaItemPropertyArtist, MPMediaItemPropertyArtwork,
    MPMediaItemPropertyTitle, MPNowPlayingInfoCenter, MPNowPlayingInfoPropertyIsLiveStream,
    MPNowPlayingPlaybackState, MPRemoteCommandCenter, MPRemoteCommandEvent,
    MPRemoteCommandHandlerStatus,
};

use crate::api::SwStation;
use crate::audio::{SwPlaybackState, SwPlayer};

// Gets used on Apple platforms, mainly macOS
// See https://developer.apple.com/documentation/mediaplayer/mpnowplayinginfocenter

#[derive(Debug, Clone)]
pub struct MpNowPlayingController {
    info_center: Rc<Retained<MPNowPlayingInfoCenter>>,
    player: SwPlayer,
}

impl MpNowPlayingController {
    pub fn start(player: &SwPlayer) -> Self {
        let info_center = unsafe { MPNowPlayingInfoCenter::defaultCenter() };

        let controller = Self {
            info_center: Rc::new(info_center),
            player: player.clone(),
        };

        // Shortwave side callbacks
        controller.setup_player_callbacks();

        // macOS side callbacks
        controller.setup_remote_commands();

        controller.update_metadata();
        controller.update_playback_status();

        controller
    }

    fn setup_player_callbacks(&self) {
        self.player.connect_state_notify(clone!(
            #[strong(rename_to = controller)]
            self,
            move |_| {
                controller.update_playback_status();
            }
        ));

        self.player.connect_station_notify(clone!(
            #[strong(rename_to = controller)]
            self,
            move |_| {
                controller.update_metadata();
            }
        ));

        self.player.connect_playing_track_notify(clone!(
            #[strong(rename_to = controller)]
            self,
            move |_| {
                controller.update_metadata();
            }
        ));
    }

    fn setup_remote_commands(&self) {
        let commands = unsafe { MPRemoteCommandCenter::sharedCommandCenter() };

        // play command
        let block = RcBlock::new(clone!(
            #[strong(rename_to = player)]
            self.player,
            move |_event: NonNull<MPRemoteCommandEvent>| -> MPRemoteCommandHandlerStatus {
                glib::spawn_future_local(clone!(
                    #[strong]
                    player,
                    async move { player.start_playback().await }
                ));
                MPRemoteCommandHandlerStatus::Success
            }
        ));
        unsafe {
            commands.playCommand().addTargetWithHandler(&block);
        }

        // pause/stop command
        let block = RcBlock::new(clone!(
            #[strong(rename_to = player)]
            self.player,
            move |_event: NonNull<MPRemoteCommandEvent>| -> MPRemoteCommandHandlerStatus {
                glib::spawn_future_local(clone!(
                    #[strong]
                    player,
                    async move { player.stop_playback().await }
                ));
                MPRemoteCommandHandlerStatus::Success
            }
        ));
        unsafe {
            commands.pauseCommand().addTargetWithHandler(&block);
            commands.stopCommand().addTargetWithHandler(&block);
        }

        // toggle play/pause command
        let block = RcBlock::new(clone!(
            #[strong(rename_to = player)]
            self.player,
            move |_event: NonNull<MPRemoteCommandEvent>| -> MPRemoteCommandHandlerStatus {
                glib::spawn_future_local(clone!(
                    #[strong]
                    player,
                    async move { player.toggle_playback().await }
                ));
                MPRemoteCommandHandlerStatus::Success
            }
        ));
        unsafe {
            commands
                .togglePlayPauseCommand()
                .addTargetWithHandler(&block);
        }
    }

    fn update_metadata(&self) {
        let player = &self.player;

        let Some(station) = player.station() else {
            unsafe { self.info_center.setNowPlayingInfo(None) };
            return;
        };

        let artwork = Self::load_artwork(&station);
        let track_title = player.playing_track().map(|track| track.title());

        let mut info: Vec<(&NSString, Retained<AnyObject>)> = Vec::new();

        unsafe {
            // Station name / current title
            if let Some(title) = &track_title {
                info.push((
                    MPMediaItemPropertyTitle,
                    Retained::cast_unchecked(NSString::from_str(title)),
                ));
                info.push((
                    MPMediaItemPropertyArtist,
                    Retained::cast_unchecked(NSString::from_str(&station.title())),
                ));
            } else {
                info.push((
                    MPMediaItemPropertyTitle,
                    Retained::cast_unchecked(NSString::from_str(&station.title())),
                ));
            }

            // Station cover
            if let Some(artwork) = artwork {
                info.push((MPMediaItemPropertyArtwork, artwork));
            }

            // We only play live streams
            info.push((
                MPNowPlayingInfoPropertyIsLiveStream,
                Retained::cast_unchecked(NSNumber::new_bool(true)),
            ));

            // Apply new now playing info
            let (keys, values): (Vec<&NSString>, Vec<_>) = info.into_iter().unzip();
            let info = NSDictionary::from_retained_objects(&keys, &values);
            self.info_center.setNowPlayingInfo(Some(&info));
        }
    }

    // TODO: Route this through the cached/rendered cover loader pipeline
    fn load_artwork(station: &SwStation) -> Option<Retained<AnyObject>> {
        let favicon_url = station.metadata().favicon?;

        unsafe {
            let url_string = NSString::from_str(favicon_url.as_str());
            let url = NSURL::URLWithString(&url_string)?;
            let image = NSImage::initWithContentsOfURL(NSImage::alloc(), &url)?;
            let size = image.size();

            let artwork = MPMediaItemArtwork::initWithBoundsSize_requestHandler(
                MPMediaItemArtwork::alloc(),
                size,
                &RcBlock::new(move |_size| -> NonNull<NSImage> { NonNull::from(&*image) }),
            );

            Some(Retained::cast_unchecked(artwork))
        }
    }

    fn update_playback_status(&self) {
        let player = &self.player;

        let playback_state = match player.state() {
            SwPlaybackState::Playing | SwPlaybackState::Loading => {
                MPNowPlayingPlaybackState::Playing
            }
            SwPlaybackState::Stopped | SwPlaybackState::Failure => {
                MPNowPlayingPlaybackState::Stopped
            }
        };

        unsafe {
            self.info_center.setPlaybackState(playback_state);
        }
    }
}
