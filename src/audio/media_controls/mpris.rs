// Shortwave - mpris.rs
// Copyright (C) 2024-2026  Felix Häcker <haeckerfelix@gnome.org>
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

// Gets used on UNIX(-like) platforms, excluding macOS (Linux, BSD, ...)
// See: https://specifications.freedesktop.org/mpris/latest/

use std::cell::RefCell;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::rc::Rc;

use gio::prelude::FileExt;
use glib::clone;
use gtk::{gio, glib, prelude::ApplicationExt};
use mpris_server::{Metadata, PlaybackStatus, Player, zbus::Result};

use crate::api::{StationCover, SwStation};
use crate::audio::media_controls::COVER_SIZE;
use crate::audio::{SwPlayer, SwTrack};
use crate::utils;
use crate::{app::SwApplication, config, path};

#[derive(Debug, Clone)]
pub struct Mpris {
    mpris: Rc<Player>,
    player: SwPlayer,
    cover: Rc<RefCell<StationCover>>,
}

impl Mpris {
    pub async fn start(player: &SwPlayer) -> Result<Self> {
        let mpris = Player::builder(*config::APP_ID)
            .desktop_entry(*config::APP_ID)
            .identity(*config::NAME)
            .can_play(true)
            // This is not true, but MPRIS has no concept of play/stop
            // for live streams, so we only can use play/pause here
            .can_pause(true)
            .can_go_next(false)
            .can_go_previous(false)
            .can_seek(false)
            .can_set_fullscreen(false)
            .can_raise(true)
            .can_quit(true)
            .build()
            .await?;

        let server = Self {
            mpris: Rc::new(mpris),
            player: player.clone(),
            cover: Rc::new(RefCell::new(StationCover::new(None, COVER_SIZE))),
        };

        // Shortwave side callbacks
        server.setup_player_callbacks();

        // Mpris side callbacks
        server.setup_mpris_callbacks();

        glib::spawn_future_local(server.mpris.run());

        server.update_playback_status().await;
        server.update_metadata().await;
        server.update_volume().await;

        Ok(server)
    }

    fn setup_player_callbacks(&self) {
        self.player.connect_state_notify(clone!(
            #[strong(rename_to = server)]
            self,
            move |_| {
                glib::spawn_future_local(clone!(
                    #[strong]
                    server,
                    async move {
                        server.update_playback_status().await;
                    }
                ));
            }
        ));

        self.player.connect_station_notify(clone!(
            #[strong(rename_to = server)]
            self,
            move |_| {
                let station = SwApplication::default().player().station();
                *server.cover.borrow_mut() = StationCover::new(station, COVER_SIZE);

                glib::spawn_future_local(clone!(
                    #[strong]
                    server,
                    async move {
                        server.update_metadata().await;
                    }
                ));
            }
        ));

        self.player.connect_playing_track_notify(clone!(
            #[strong(rename_to = server)]
            self,
            move |_| {
                glib::spawn_future_local(clone!(
                    #[strong]
                    server,
                    async move {
                        server.update_metadata().await;
                    }
                ));
            }
        ));

        self.player.connect_volume_notify(clone!(
            #[strong(rename_to = server)]
            self,
            move |_| {
                glib::spawn_future_local(clone!(
                    #[strong]
                    server,
                    async move {
                        server.update_volume().await;
                    }
                ));
            }
        ));
    }

    fn setup_mpris_callbacks(&self) {
        self.mpris.connect_play_pause(|_| {
            glib::spawn_future_local(async move {
                SwApplication::default().player().toggle_playback().await;
            });
        });

        self.mpris.connect_play(|_| {
            glib::spawn_future_local(async move {
                SwApplication::default().player().start_playback().await;
            });
        });

        self.mpris.connect_stop(|_| {
            glib::spawn_future_local(async move {
                SwApplication::default().player().stop_playback().await;
            });
        });

        self.mpris.connect_set_volume(|_, volume| {
            SwApplication::default().player().set_volume(volume);
        });

        self.mpris.connect_raise(|_| {
            SwApplication::default().activate();
        });

        self.mpris.connect_quit(|_| {
            SwApplication::default().quit();
        });
    }

    async fn update_playback_status(&self) {
        let can_play = self.player.has_station();
        if let Err(err) = self.mpris.set_can_play(can_play).await {
            error!("Unable to update mpris can-play: {:?}", err.to_string())
        }

        let playback_status = if self.player.state().is_active() {
            PlaybackStatus::Playing
        } else {
            PlaybackStatus::Stopped
        };

        if let Err(err) = self.mpris.set_playback_status(playback_status).await {
            error!(
                "Unable to update mpris playback status: {:?}",
                err.to_string()
            )
        }
    }

    async fn update_volume(&self) {
        if let Err(err) = self.mpris.set_volume(self.player.volume()).await {
            error!("Unable to update mpris volume: {:?}", err.to_string())
        }
    }

    async fn update_metadata(&self) {
        let Some(station) = self.player.station() else {
            if let Err(err) = self.mpris.set_metadata(Metadata::builder().build()).await {
                error!("Unable to update mpris metadata: {:?}", err.to_string())
            }
            return;
        };

        let track = self.player.playing_track();
        let artwork = self.load_artwork();

        self.apply_metadata(&station, &track, artwork).await;
    }

    async fn apply_metadata(&self, station: &SwStation, track: &Option<SwTrack>, artwork: String) {
        let (title, artist) = utils::title_and_artist(station, track);

        let mut metadata = Metadata::builder().title(title).art_url(artwork);

        if let Some(artist) = artist {
            metadata = metadata.artist(vec![artist]);
        }

        if let Some(url) = station.stream_url() {
            metadata = metadata.url(url);
        }

        if let Err(err) = self.mpris.set_metadata(metadata.build()).await {
            error!("Unable to update mpris metadata: {:?}", err.to_string())
        }
    }

    fn load_artwork(&self) -> String {
        let bytes = self.cover.borrow().load_bytes(clone!(
            #[strong(rename_to = c)]
            self,
            move |bytes| {
                let Some(station) = c.player.station() else {
                    return;
                };
                let track = c.player.playing_track();
                let artwork = Self::bytes_to_artwork(bytes);

                glib::spawn_future_local(clone!(
                    #[strong]
                    c,
                    async move {
                        c.apply_metadata(&station, &track, artwork).await;
                    }
                ));
            }
        ));

        Self::bytes_to_artwork(bytes)
    }

    fn bytes_to_artwork(bytes: Vec<u8>) -> String {
        let mut hasher = DefaultHasher::new();
        bytes.hash(&mut hasher);

        let dir = path::CACHE.join("mpris-cover");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(format!("{:x}.png", hasher.finish()));

        if !path.exists()
            && let Err(err) = std::fs::write(&path, &bytes)
        {
            error!("Unable to write mpris cover to disk: {:?}", err.to_string());
        }

        gio::File::for_path(&path).uri().to_string()
    }
}
