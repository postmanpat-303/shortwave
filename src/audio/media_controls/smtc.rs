// Shortwave - smtc.rs
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

use std::rc::Rc;

use glib::clone;
use gtk::glib;
use windows::Foundation::{TypedEventHandler, Uri};
use windows::Media::{
    MediaPlaybackStatus, MediaPlaybackType, SystemMediaTransportControls,
    SystemMediaTransportControlsButton, SystemMediaTransportControlsButtonPressedEventArgs,
};
use windows::Storage::Streams::RandomAccessStreamReference;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::WinRT::ISystemMediaTransportControlsInterop;
use windows::core::{HSTRING, Ref};

use crate::audio::{SwPlaybackState, SwPlayer};

// Gets used on Windows platforms (System Media Transport Control)
// See https://learn.microsoft.com/en-us/uwp/api/windows.media.control

#[derive(Debug, Clone)]
pub struct SmtcController {
    controls: Rc<SystemMediaTransportControls>,
    player: SwPlayer,
}

impl SmtcController {
    pub fn start(player: &SwPlayer, hwnd: HWND) -> windows::core::Result<Self> {
        let interop: ISystemMediaTransportControlsInterop = windows::core::factory::<
            SystemMediaTransportControls,
            ISystemMediaTransportControlsInterop,
        >()?;

        let controls: SystemMediaTransportControls = unsafe { interop.GetForWindow(hwnd)? };

        controls
            .DisplayUpdater()?
            .SetType(MediaPlaybackType::Music)?;

        controls.SetIsEnabled(true)?;
        controls.SetIsPlayEnabled(true)?;
        controls.SetIsPauseEnabled(true)?;
        controls.SetIsStopEnabled(true)?;

        let controller = Self {
            controls: Rc::new(controls),
            player: player.clone(),
        };

        // Shortwave side callbacks
        controller.setup_player_callbacks();

        // Windows side callbacks
        controller.setup_smtc_callbacks()?;

        controller.update_metadata()?;
        controller.update_playback_status()?;

        Ok(controller)
    }

    fn setup_player_callbacks(&self) {
        self.player.connect_state_notify(clone!(
            #[strong(rename_to = controller)]
            self,
            move |_| {
                if let Err(err) = controller.update_playback_status() {
                    error!("Unable to update SMTC playback status: {err}");
                }
            }
        ));

        self.player.connect_station_notify(clone!(
            #[strong(rename_to = controller)]
            self,
            move |_| {
                if let Err(err) = controller.update_metadata() {
                    error!("Unable to update SMTC metadata: {err}");
                }
            }
        ));

        self.player.connect_playing_track_notify(clone!(
            #[strong(rename_to = controller)]
            self,
            move |_| {
                if let Err(err) = controller.update_metadata() {
                    error!("Unable to update SMTC metadata: {err}");
                }
            }
        ));
    }

    fn setup_smtc_callbacks(&self) -> windows::core::Result<()> {
        let (tx, rx) = async_channel::unbounded::<SystemMediaTransportControlsButton>();

        let button_handler = TypedEventHandler::new(
            move |_, args: Ref<SystemMediaTransportControlsButtonPressedEventArgs>| {
                let _ = tx.try_send(args.ok()?.Button()?);
                Ok(())
            },
        );
        self.controls.ButtonPressed(&button_handler)?;

        glib::spawn_future_local(clone!(
            #[strong(rename_to = player)]
            self.player,
            async move {
                while let Ok(button) = rx.recv().await {
                    match button {
                        SystemMediaTransportControlsButton::Play => player.start_playback().await,
                        SystemMediaTransportControlsButton::Pause
                        | SystemMediaTransportControlsButton::Stop => player.stop_playback().await,
                        _ => {}
                    }
                }
            }
        ));

        Ok(())
    }

    fn update_metadata(&self) -> windows::core::Result<()> {
        let player = &self.player;

        let display_updater = self.controls.DisplayUpdater()?;
        let music_properties = display_updater.MusicProperties()?;

        let Some(station) = player.station() else {
            return self.controls.SetPlaybackStatus(MediaPlaybackStatus::Closed);
        };

        let track_title = player.playing_track().map(|track| track.title());

        // Station name / current title
        if let Some(title) = &track_title {
            music_properties.SetTitle(&HSTRING::from(title))?;
            music_properties.SetArtist(&HSTRING::from(station.title()))?;
        } else {
            music_properties.SetTitle(&HSTRING::from(station.title()))?;
            music_properties.SetArtist(&HSTRING::from(""))?;
        }

        // Station cover
        // TODO: Route this through the cached/rendered cover loader pipeline
        let artwork = station.metadata().favicon.and_then(|favicon_url| {
            let uri = Uri::CreateUri(&HSTRING::from(favicon_url.as_str())).ok()?;
            RandomAccessStreamReference::CreateFromUri(&uri).ok()
        });
        if let Some(artwork) = artwork {
            display_updater.SetThumbnail(&artwork)?;
        }

        display_updater.Update()
    }

    fn update_playback_status(&self) -> windows::core::Result<()> {
        let playback_status = match self.player.state() {
            SwPlaybackState::Playing | SwPlaybackState::Loading => MediaPlaybackStatus::Playing,
            SwPlaybackState::Stopped | SwPlaybackState::Failure => MediaPlaybackStatus::Stopped,
        };

        self.controls.SetPlaybackStatus(playback_status)
    }
}
