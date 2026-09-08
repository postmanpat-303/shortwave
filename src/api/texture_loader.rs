// Shortwave - cover_downloader.rs
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

use anyhow::{Error, Result};
use async_channel::Sender;
use async_compat::CompatExt;
use futures_util::StreamExt;
use gtk::gio::CancellableFuture;
use gtk::{gdk, gio, glib};
use url::Url;

use crate::api::cache;
use crate::utils;

#[derive(Debug, Clone)]
pub struct TextureLoader {
    job_sender: Sender<DownloadJob>,
}

impl TextureLoader {
    pub fn new() -> Self {
        let (job_sender, job_receiver) = async_channel::unbounded::<DownloadJob>();
        let job_stream = job_receiver
            .map(|r| r.execute())
            .buffer_unordered(usize::max(glib::num_processors() as usize / 2, 2));

        glib::spawn_future_local(async move {
            job_stream.collect::<Vec<_>>().await;
        });

        Self { job_sender }
    }

    pub async fn load_texture(
        &self,
        url: &Url,
        cancellable: gio::Cancellable,
    ) -> Result<gdk::Texture> {
        let (sender, receiver) = async_channel::bounded(1);

        let request = DownloadJob {
            url: url.clone(),
            sender,
            cancellable: cancellable.clone(),
        };

        self.job_sender
            .send(request)
            .await
            .map_err(|_| Error::msg("Unable to send job"))?;

        receiver.recv().await?
    }
}

#[derive(Debug, Clone)]
struct DownloadJob {
    url: Url,
    sender: Sender<Result<gdk::Texture>>,
    cancellable: gio::Cancellable,
}

impl DownloadJob {
    pub async fn execute(self) {
        let result = CancellableFuture::new(
            async {
                match self.cached_texture().await {
                    Ok(cached) => Ok(cached),
                    Err(_) => self.download_texture().compat().await,
                }
            },
            self.cancellable.clone(),
        )
        .await;

        let result = result.unwrap_or_else(|cancelled| Err(cancelled.into()));
        self.sender.send(result).await.unwrap();
    }

    async fn cached_texture(&self) -> Result<gdk::Texture> {
        let data = cache::read(self.url.as_str()).await?;
        utils::decode_untrusted_image(glib::Bytes::from_owned(data)).await
    }

    async fn download_texture(&self) -> Result<gdk::Texture> {
        // Download
        let response = crate::api::http::get(self.url.clone()).await?;
        let body_bytes = response.bytes().await?.to_vec();

        // Cache raw bytes
        cache::write(self.url.as_str(), &body_bytes).await?;

        utils::decode_untrusted_image(glib::Bytes::from_owned(body_bytes)).await
    }
}
