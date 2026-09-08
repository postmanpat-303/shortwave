// Shortwave - station_cover.rs
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

use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::LazyLock;

use anyhow::Result;
use gdk::RGBA;
use gsk::prelude::IsRenderNode;
use gtk::gdk::Texture;
use gtk::graphene::{Point, Rect};
use gtk::gsk::{
    BlurNode, ClipNode, ColorNode, ContainerNode, ScalingFilter, TextureNode, TextureScaleNode,
};
use gtk::prelude::*;
use gtk::{gdk, gio, glib, gsk};
use url::Url;

use super::SwStation;
use crate::api::{TextureLoader, cache};
use crate::config;
use crate::utils::{self, node_to_texture};

pub static TEXTURE_LOADER: LazyLock<TextureLoader> = LazyLock::new(TextureLoader::new);

enum CoverSource {
    Custom(glib::Bytes),
    Favicon(Url),
}

// Based on libadwaita avatar colors
// https://gitlab.gnome.org/GNOME/libadwaita/-/blob/8c422b36bcc3efca6bbf63a3979bd53aededbdcd/src/stylesheet/widgets/_avatar.scss
// The list has this form: (symbolic-color, bg-color)
const COLORS: [(&str, &str); 14] = [
    ("#cfe1f5", "#337fdc"), // blue
    ("#caeaf2", "#0f9ac8"), // cyan
    ("#cef8d8", "#29ae74"), // green
    ("#e6f9d7", "#6ab85b"), // lime
    ("#f9f4e1", "#d29d09"), // yellow
    ("#ffead1", "#d68400"), // gold
    ("#ffe5c5", "#ed5b00"), // orange
    ("#f8d2ce", "#e62d42"), // raspberry
    ("#fac7de", "#e33b6a"), // magenta
    ("#e7c2e8", "#9945b5"), // purple
    ("#d5d2f5", "#7a59ca"), // violet
    ("#f2eade", "#b08952"), // beige
    ("#e5d6ca", "#785336"), // brown
    ("#d8d7d3", "#6e6d71"), // gray
];

#[derive(Debug)]
pub struct StationCover {
    station: Option<SwStation>,
    size: i32,
    cancellable: gio::Cancellable,
}

impl StationCover {
    pub fn new(station: Option<SwStation>, size: i32) -> Self {
        Self {
            station,
            size,
            cancellable: gio::Cancellable::default(),
        }
    }

    pub fn load_texture(&self, on_loaded: impl Fn(Texture) + 'static) -> Texture {
        let title = self.station.as_ref().map(|s| s.title()).unwrap_or_default();

        let Some((key, source)) = self.resolve_cover_source() else {
            return Self::render_fallback_cover(&title, self.size);
        };

        if let Ok(bytes) = cache::read_sync(&key)
            && let Ok(texture) = Texture::from_bytes(&glib::Bytes::from_owned(bytes))
        {
            return texture;
        }

        self.spawn_render(source, key, move |texture, _bytes| on_loaded(texture));

        Self::render_fallback_cover(&title, self.size)
    }

    pub fn load_bytes(&self, on_loaded: impl Fn(Vec<u8>) + 'static) -> Vec<u8> {
        let title = self.station.as_ref().map(|s| s.title()).unwrap_or_default();

        let Some((key, source)) = self.resolve_cover_source() else {
            return Self::render_fallback_cover(&title, self.size)
                .save_to_png_bytes()
                .to_vec();
        };

        if let Ok(bytes) = cache::read_sync(&key) {
            return bytes;
        }

        self.spawn_render(source, key, move |_texture, bytes| on_loaded(bytes));

        Self::render_fallback_cover(&title, self.size)
            .save_to_png_bytes()
            .to_vec()
    }

    fn resolve_cover_source(&self) -> Option<(String, CoverSource)> {
        let station = self.station.as_ref()?;
        let size = self.size;

        if let Some(custom_cover) = station.custom_cover() {
            let mut hasher = DefaultHasher::new();
            custom_cover.as_ref().hash(&mut hasher);
            let hash = hasher.finish();

            let key = format!("cover:custom-{size}-{hash:x}");
            Some((key, CoverSource::Custom(custom_cover)))
        } else if let Some(favicon_url) = station.metadata().favicon {
            let key = format!("cover:favicon-{size}-{favicon_url}");
            Some((key, CoverSource::Favicon(favicon_url)))
        } else {
            None
        }
    }

    fn spawn_render(
        &self,
        source: CoverSource,
        key: String,
        on_loaded: impl Fn(Texture, Vec<u8>) + 'static,
    ) {
        let size = self.size;
        let cancellable = self.cancellable.clone();

        glib::spawn_future_local(async move {
            let result: Result<(Texture, Vec<u8>)> = async {
                let raw = match source {
                    CoverSource::Custom(bytes) => utils::decode_untrusted_image(bytes).await?,
                    CoverSource::Favicon(url) => {
                        TEXTURE_LOADER
                            .load_texture(&url, cancellable.clone())
                            .await?
                    }
                };
                Self::render_cover(key, raw, size).await
            }
            .await;

            if cancellable.is_cancelled() {
                return;
            }

            match result {
                Ok((texture, bytes)) => on_loaded(texture, bytes),
                Err(e) => trace!("Unable to load cover: {e}"),
            }
        });
    }

    async fn render_cover(key: String, favicon: Texture, size: i32) -> Result<(Texture, Vec<u8>)> {
        let handle = gio::spawn_blocking(move || {
            let node = Self::draw_cover(favicon, size as f32);
            let texture = node_to_texture(&node, size);
            let png_bytes = texture.save_to_png_bytes().to_vec();
            (texture, png_bytes)
        });
        let (texture, bytes) = handle.await.unwrap();

        cache::write(&key, &bytes).await?;
        Ok((texture, bytes))
    }

    // Intial implementation from Highscore (Alice Mikhaylenko), ported to gsk nodes
    // https://gitlab.gnome.org/World/highscore/-/blob/b07460f0c1475269381902c6305e4d91e55b61f5/src/library/cover-loader.vala#L124
    fn draw_cover(favicon: Texture, size: f32) -> gsk::RenderNode {
        // Fit size
        let aspect_ratio = favicon.width() as f32 / favicon.height() as f32;
        let mut width = size;
        let mut height = size;

        if aspect_ratio < 1.0 {
            width = aspect_ratio * size;
        } else {
            height = size / aspect_ratio;
        }

        if width >= size - 2.0 {
            width = size;
        }

        if height >= size - 2.0 {
            height = size;
        }

        let cover_rect = Rect::new((size - width) / 2.0, (size - height) / 2.0, width, height);
        let bounds = Rect::new(0.0, 0.0, size, size);

        // Background
        let color_node = ColorNode::new(&RGBA::new(0.96, 0.96, 0.96, 1.0), &bounds);
        let mut children = vec![color_node.upcast()];

        // Blurred backdrop, only needed if the cover doesn't fill the full square
        if width < size || height < size {
            let blur_radius = size / 4.0;

            let outer_rect_width;
            let outer_rect_height;
            if aspect_ratio < 1.0 {
                outer_rect_width = size + blur_radius * 2.0;
                outer_rect_height = outer_rect_width / aspect_ratio;
            } else {
                outer_rect_height = size + blur_radius * 2.0;
                outer_rect_width = aspect_ratio * outer_rect_height;
            }

            let outer_rect = Rect::new(
                (size - outer_rect_width) / 2.0,
                (size - outer_rect_height) / 2.0,
                outer_rect_width,
                outer_rect_height,
            );

            let blurred_texture = TextureNode::new(&favicon, &outer_rect);
            children.push(BlurNode::new(blurred_texture, blur_radius).upcast());
            children.push(ColorNode::new(&RGBA::new(0.0, 0.0, 0.0, 0.2), &outer_rect).upcast());
        }

        // Cover
        let scale_node = TextureScaleNode::new(&favicon, &cover_rect, ScalingFilter::Trilinear);
        children.push(scale_node.upcast());

        // Render
        let container = ContainerNode::new(&children);
        ClipNode::new(container, &bounds).upcast()
    }

    fn render_fallback_cover(title: &str, size: i32) -> Texture {
        // Colors
        let (icon_color, bg_color) = if !title.is_empty() {
            let mut hasher = DefaultHasher::new();
            title.hash(&mut hasher);

            let (icon, bg) = COLORS[(hasher.finish() as usize) % COLORS.len()];
            let icon_color = RGBA::parse(icon).unwrap();
            let bg_color = RGBA::parse(bg).unwrap();

            (icon_color, bg_color)
        } else {
            // Fallback if there's no text
            let icon_color = RGBA::new(0.0, 0.0, 0.0, 0.0);
            let bg_color = RGBA::new(0.0, 0.0, 0.0, 0.0);
            (icon_color, bg_color)
        };

        let snapshot = gtk::Snapshot::new();
        let display = gdk::Display::default().expect("No default display available");

        // Background
        let bounds = Rect::new(0.0, 0.0, size as f32, size as f32);
        snapshot.append_color(&bg_color, &bounds);

        // Icon
        let icon_size = (size as f64 * 0.5).round() as i32;
        let icon = gtk::IconTheme::for_display(&display).lookup_icon(
            &format!("{}-symbolic", *config::APP_ID),
            &[],
            icon_size,
            1,
            gtk::TextDirection::None,
            gtk::IconLookupFlags::FORCE_SYMBOLIC,
        );

        let offset = ((size - icon_size) / 2) as f32;
        snapshot.translate(&Point::new(offset, offset));
        icon.snapshot_symbolic(&snapshot, icon_size as f64, icon_size as f64, &[icon_color]);

        // Render
        let node = snapshot.to_node().unwrap();
        node_to_texture(&node, size)
    }
}

impl Drop for StationCover {
    fn drop(&mut self) {
        self.cancellable.cancel();
    }
}
