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

use std::cell::RefCell;
use std::ptr::NonNull;
use std::rc::Rc;

use block2::RcBlock;
use glib::clone;
use gtk::glib;
use objc2::AnyThread;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::NSImage;
use objc2_foundation::{NSData, NSDictionary, NSNumber, NSString};
use objc2_media_player::{
    MPMediaItemArtwork, MPMediaItemPropertyArtist, MPMediaItemPropertyArtwork,
    MPMediaItemPropertyTitle, MPNowPlayingInfoCenter, MPNowPlayingInfoPropertyIsLiveStream,
    MPNowPlayingPlaybackState, MPRemoteCommandCenter, MPRemoteCommandEvent,
    MPRemoteCommandHandlerStatus,
};

use crate::api::{StationCover, SwStation};
use crate::audio::media_controls::COVER_SIZE;
use crate::audio::{SwPlayer, SwTrack};
use crate::utils;

// Gets used on Apple platforms, mainly macOS
// See https://developer.apple.com/documentation/mediaplayer/mpnowplayinginfocenter

#[derive(Debug, Clone)]
pub struct MpNowPlaying {
    info_center: Rc<Retained<MPNowPlayingInfoCenter>>,
    player: SwPlayer,
    cover: Rc<RefCell<StationCover>>,
}

impl MpNowPlaying {
    pub fn start(player: &SwPlayer) -> Self {
        let info_center = unsafe { MPNowPlayingInfoCenter::defaultCenter() };

        let controller = Self {
            info_center: Rc::new(info_center),
            player: player.clone(),
            cover: Rc::new(RefCell::new(StationCover::new(None, COVER_SIZE))),
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
            #[strong(rename_to = c)]
            self,
            move |_| {
                c.update_playback_status();
            }
        ));

        self.player.connect_station_notify(clone!(
            #[strong(rename_to = c)]
            self,
            move |_| {
                *c.cover.borrow_mut() = StationCover::new(c.player.station(), COVER_SIZE);
                c.update_metadata();
            }
        ));

        self.player.connect_playing_track_notify(clone!(
            #[strong(rename_to = c)]
            self,
            move |_| {
                c.update_metadata();
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

    fn update_playback_status(&self) {
        let playback_state = if self.player.state().is_active() {
            MPNowPlayingPlaybackState::Playing
        } else {
            MPNowPlayingPlaybackState::Stopped
        };

        unsafe {
            self.info_center.setPlaybackState(playback_state);
        }
    }

    fn update_metadata(&self) {
        let Some(station) = self.player.station() else {
            unsafe { self.info_center.setNowPlayingInfo(None) };
            return;
        };

        let track = self.player.playing_track();
        let artwork = self.load_artwork();

        self.apply_metadata(&station, &track, artwork);
    }

    fn apply_metadata(
        &self,
        station: &SwStation,
        track: &Option<SwTrack>,
        artwork: Retained<AnyObject>,
    ) {
        let (title, artist) = utils::title_and_artist(station, track);
        let mut info: Vec<(&NSString, Retained<AnyObject>)> = Vec::new();

        unsafe {
            // Station name / current title
            info.push((
                MPMediaItemPropertyTitle,
                Retained::cast_unchecked(NSString::from_str(&title)),
            ));
            if let Some(artist) = artist {
                info.push((
                    MPMediaItemPropertyArtist,
                    Retained::cast_unchecked(NSString::from_str(&artist)),
                ));
            }

            // Station cover
            info.push((MPMediaItemPropertyArtwork, artwork));

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

    fn load_artwork(&self) -> Retained<AnyObject> {
        let bytes = self.cover.borrow().load_bytes(clone!(
            #[strong(rename_to = controller)]
            self,
            move |bytes| {
                let Some(station) = controller.player.station() else {
                    return;
                };

                let track = controller.player.playing_track();
                let artwork = Self::bytes_to_artwork(bytes);

                controller.apply_metadata(&station, &track, artwork);
            }
        ));

        Self::bytes_to_artwork(bytes)
    }

    fn bytes_to_artwork(bytes: Vec<u8>) -> Retained<AnyObject> {
        let data = NSData::with_bytes(&bytes);
        let image = NSImage::initWithData(NSImage::alloc(), &data).unwrap();
        let size = image.size();

        unsafe {
            let artwork = MPMediaItemArtwork::initWithBoundsSize_requestHandler(
                MPMediaItemArtwork::alloc(),
                size,
                &RcBlock::new(move |_size| -> NonNull<NSImage> { NonNull::from(&*image) }),
            );

            Retained::cast_unchecked(artwork)
        }
    }
}
