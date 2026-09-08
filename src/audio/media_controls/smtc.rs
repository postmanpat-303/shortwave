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

use std::cell::RefCell;
use std::rc::Rc;

use glib::clone;
use gtk::glib;
use windows::Foundation::TypedEventHandler;
use windows::Media::{
    MediaPlaybackStatus, MediaPlaybackType, SystemMediaTransportControls,
    SystemMediaTransportControlsButton, SystemMediaTransportControlsButtonPressedEventArgs,
};
use windows::Storage::Streams::{
    DataWriter, InMemoryRandomAccessStream, RandomAccessStreamReference,
};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::WinRT::ISystemMediaTransportControlsInterop;
use windows::core::{HSTRING, Ref, Result};

use crate::api::{StationCover, SwStation};
use crate::audio::media_controls::COVER_SIZE;
use crate::audio::{SwPlayer, SwTrack};
use crate::utils;

// Gets used on Windows platforms (System Media Transport Control)
// See https://learn.microsoft.com/en-us/uwp/api/windows.media.control

#[derive(Debug, Clone)]
pub struct Smtc {
    controls: Rc<SystemMediaTransportControls>,
    player: SwPlayer,
    cover: Rc<RefCell<StationCover>>,
}

impl Smtc {
    pub fn start(player: &SwPlayer, hwnd: HWND) -> Result<Self> {
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
            cover: Rc::new(RefCell::new(StationCover::new(None, COVER_SIZE))),
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
                *controller.cover.borrow_mut() =
                    StationCover::new(controller.player.station(), COVER_SIZE);

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

    fn setup_smtc_callbacks(&self) -> Result<()> {
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

    fn update_playback_status(&self) -> Result<()> {
        let playback_status = if self.player.state().is_active() {
            MediaPlaybackStatus::Playing
        } else {
            MediaPlaybackStatus::Stopped
        };

        self.controls.SetPlaybackStatus(playback_status)
    }

    fn update_metadata(&self) -> Result<()> {
        let Some(station) = self.player.station() else {
            return self.controls.SetPlaybackStatus(MediaPlaybackStatus::Closed);
        };

        let track = self.player.playing_track();
        let artwork = self.load_artwork()?;

        self.apply_metadata(&station, &track, artwork)
    }

    fn apply_metadata(
        &self,
        station: &SwStation,
        track: &Option<SwTrack>,
        artwork: Option<RandomAccessStreamReference>,
    ) -> Result<()> {
        let display_updater = self.controls.DisplayUpdater()?;
        let music_properties = display_updater.MusicProperties()?;

        // Station name / current title
        let (title, artist) = utils::title_and_artist(station, track);
        music_properties.SetTitle(&HSTRING::from(title))?;
        music_properties.SetArtist(&HSTRING::from(artist.unwrap_or_default()))?;

        // Station cover
        if let Some(artwork) = artwork {
            display_updater.SetThumbnail(&artwork)?;
        }

        display_updater.Update()
    }

    fn load_artwork(&self) -> Result<Option<RandomAccessStreamReference>> {
        let bytes = self.cover.borrow().load_bytes(clone!(
            #[strong(rename_to = controller)]
            self,
            move |bytes| {
                let Some(station) = controller.player.station() else {
                    return;
                };
                let track = controller.player.playing_track();

                let result = Self::bytes_to_artwork(&bytes)
                    .and_then(|artwork| controller.apply_metadata(&station, &track, Some(artwork)));

                if let Err(err) = result {
                    error!("Unable to update SMTC artwork: {err}");
                }
            }
        ));

        Self::bytes_to_artwork(&bytes).map(Some)
    }

    fn bytes_to_artwork(bytes: &[u8]) -> Result<RandomAccessStreamReference> {
        let stream = InMemoryRandomAccessStream::new()?;

        let writer = DataWriter::CreateDataWriter(&stream)?;
        writer.WriteBytes(bytes)?;
        writer.StoreAsync()?.join()?;
        writer.DetachStream()?;

        stream.Seek(0)?;
        RandomAccessStreamReference::CreateFromStream(&stream)
    }
}
