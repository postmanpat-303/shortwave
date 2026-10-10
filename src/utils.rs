// Shortwave - utils.rs
// Copyright (C) 2021-2026  Felix Häcker <haeckerfelix@gnome.org>
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

use formatx::formatx;
use gettextrs::{gettext, ngettext};
use glycin::Loader;
use gtk::graphene::Rect;
use gtk::prelude::*;
use gtk::{gdk, glib, gsk};

use crate::api::SwStation;
use crate::audio::SwTrack;

pub fn send<T: 'static>(sender: &async_channel::Sender<T>, message: T) {
    let fut = glib::clone!(
        #[strong]
        sender,
        async move {
            if let Err(err) = sender.send(message).await {
                error!(
                    "Failed to send \"{}\" action due to {err}",
                    stringify!(message),
                );
            }
        }
    );
    glib::spawn_future_local(fut);
}

pub fn format_duration(d: u64, short: bool) -> String {
    if short {
        let dt = glib::DateTime::from_unix_utc(d.try_into().unwrap_or_default()).unwrap();
        dt.format("%M:%S").unwrap_or_default().to_string()
    } else {
        let time: u32 = d.try_into().unwrap();
        let sec = time % 60;
        let time = time - sec;
        let min = (time % (60 * 60)) / 60;
        let time = time - (min * 60);
        let hour = time / (60 * 60);

        let hours = if hour != 0 {
            formatx!(ngettext("{} hour", "{} hours", hour), hour).unwrap()
        } else {
            String::new()
        };

        let mins = if min != 0 {
            formatx!(ngettext("{} minute", "{} minutes", min), min).unwrap()
        } else {
            String::new()
        };

        let secs_str = formatx!(ngettext("{} second", "{} seconds", sec), sec).unwrap();
        let secs = if sec != 0 {
            secs_str.clone()
        } else {
            String::new()
        };

        if hour > 0 {
            // Translators: Do NOT translate the content between '{' and '}', this is a variable name.
            formatx!(
                gettext("{hours} {mins} {secs}"),
                hours = hours,
                mins = mins,
                secs = secs,
            )
            .unwrap()
        } else if min > 0 {
            // Translators: Do NOT translate the content between '{' and '}', this is a variable name.
            formatx!(gettext("{mins} {secs}"), mins = mins, secs = secs).unwrap()
        } else if sec > 0 {
            secs
        } else {
            secs_str
        }
    }
}

// Falls back to the station name when there's no track playing
pub fn title_and_artist(station: &SwStation, track: &Option<SwTrack>) -> (String, Option<String>) {
    match track.as_ref().map(|t| t.title()) {
        Some(title) => (title, Some(station.title())),
        None => (station.title(), None),
    }
}

pub async fn decode_untrusted_image(bytes: glib::Bytes) -> anyhow::Result<gdk::Texture> {
    let loader = Loader::for_bytes(&bytes);
    let image = loader.load_future().await?;
    let frame = image.next_frame_future().await?;

    Ok(glycin_gtk4::frame_get_texture(&frame))
}

pub fn node_to_texture(node: &gsk::RenderNode, size: i32) -> gdk::Texture {
    let renderer = gsk::CairoRenderer::new();
    let display = gdk::Display::default().expect("No default display available");
    renderer
        .realize_for_display(&display)
        .expect("Unable to realize renderer for default display");

    let rect = Rect::new(0.0, 0.0, size as f32, size as f32);
    let texture = renderer.render_texture(node, Some(&rect));
    renderer.unrealize();

    texture
}

/// Ellipsizes a string at the end so that it is `max_len` characters long
/// Source: https://gitlab.gnome.org/World/pika-backup/-/blob/6bd7d0df56479ee769a249b466d5ac226f88056b/src/ui/utils.rs#L344
#[allow(dead_code)]
pub fn ellipsize_end<S: std::fmt::Display>(x: S, max_len: usize) -> String {
    let mut text = x.to_string();

    if text.len() <= max_len {
        text
    } else {
        text.truncate(max_len.max(1) - 1);
        text.push('…');
        text
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
pub async fn background_portal_permissions() -> bool {
    if !ashpd::is_sandboxed().await {
        debug!("App is not sandboxed, background playback is allowed.");
        return true;
    }

    if let Ok(res) = ashpd::desktop::background::Background::request()
        .reason("Play radio station in the background")
        .send()
        .await
    {
        match res.response() {
            Ok(response) => response.run_in_background(),
            Err(err) => {
                warn!("{err}");
                false
            }
        }
    } else {
        warn!("Unable to check background permissions, falling back to true.");
        true
    }
}

#[cfg(target_os = "macos")]
pub fn register_coretext_fonts() {
    let entries = std::fs::read_dir(crate::config::PREFIX.join("fonts"))
        .into_iter()
        .flatten()
        .flatten();

    for file in entries
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "ttf"))
    {
        debug!("Register font: {:?}", file);
        unsafe {
            objc2_core_text::CTFontManagerRegisterFontsForURL(
                &objc2_core_foundation::CFURL::from_file_path(&file).unwrap(),
                objc2_core_text::CTFontManagerScope::Process,
                std::ptr::null_mut(),
            );
        }
    }
}
