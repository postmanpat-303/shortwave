// Shortwave - station_dialog.rs
// Copyright (C) 2021-2024  Felix Häcker <haeckerfelix@gnome.org>
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

use std::cell::OnceCell;

use adw::prelude::*;
use adw::subclass::prelude::*;
use cruet::Inflector;
use formatx::formatx;
use gettextrs::gettext;
use glib::{Properties, subclass};
use gtk::{CompositeTemplate, gdk, glib};

#[cfg(not(target_os = "windows"))]
use shumate::prelude::*;

use crate::api::SwStation;
use crate::app::SwApplication;
use crate::ui::{SwStationCover, ToastWindow};

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate, Properties)]
    #[template(resource = "/de/haeckerfelix/Shortwave/gtk/station_dialog.ui")]
    #[properties(wrapper_type = super::SwStationDialog)]
    pub struct SwStationDialog {
        #[template_child]
        pub toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        station_cover: TemplateChild<SwStationCover>,
        #[template_child]
        local_station_group: TemplateChild<adw::PreferencesGroup>,
        #[template_child]
        orphaned_station_group: TemplateChild<adw::PreferencesGroup>,
        #[template_child]
        title_label: TemplateChild<gtk::Label>,
        #[template_child]
        homepage_label: TemplateChild<gtk::Label>,
        #[template_child]
        library_add_button: TemplateChild<gtk::Button>,
        #[template_child]
        library_remove_button: TemplateChild<gtk::Button>,
        #[template_child]
        information_group: TemplateChild<adw::PreferencesGroup>,
        #[template_child]
        language_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        tags_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        codec_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        bitrate_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        stream_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        location_group: TemplateChild<adw::PreferencesGroup>,
        #[template_child]
        country_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        state_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        map_box: TemplateChild<gtk::Box>,

        #[property(get, set, construct_only)]
        station: OnceCell<SwStation>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwStationDialog {
        const NAME: &'static str = "SwStationDialog";
        type ParentType = adw::Dialog;
        type Type = super::SwStationDialog;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for SwStationDialog {
        fn constructed(&self) {
            self.parent_constructed();

            self.setup_widgets();
        }
    }

    impl WidgetImpl for SwStationDialog {}

    impl AdwDialogImpl for SwStationDialog {}

    #[gtk::template_callbacks]
    impl SwStationDialog {
        fn setup_widgets(&self) {
            let station = self.obj().station();
            let metadata = station.metadata();

            // Station cover
            self.obj()
                .bind_property("station", &*self.station_cover, "station")
                .sync_create()
                .build();

            // Title
            self.obj().set_title(&metadata.name);
            self.title_label.set_text(&metadata.name);

            // Homepage
            if let Some(ref homepage) = metadata.homepage {
                let url = homepage.to_string().replace('&', "&amp;");
                let domain = homepage.domain().unwrap_or_default();
                let markup = format!("<a href=\"{}\">{}</a>", url, domain);

                self.homepage_label.set_visible(true);
                self.homepage_label.set_markup(&markup);
                self.homepage_label.set_tooltip_text(Some(&url));
            }

            // Action pill buttons
            if SwApplication::default()
                .library()
                .contains_station(&station)
            {
                self.library_remove_button.set_visible(true);
            } else {
                self.library_add_button.set_visible(true);
            }

            // Local station info row
            if self.station.get().unwrap().is_local() {
                self.local_station_group.set_visible(true);
                self.information_group.set_visible(false);
            }

            // Orphaned station info row
            if self.station.get().unwrap().is_orphaned() {
                self.orphaned_station_group.set_visible(true);
            }

            // Language
            if !metadata.language.is_empty() {
                self.information_group.set_visible(true);
                self.language_row.set_visible(true);
                self.language_row
                    .set_subtitle(&metadata.language.to_title_case());
            }

            // Tags
            if !metadata.tags.is_empty() {
                self.information_group.set_visible(true);
                self.tags_row.set_visible(true);
                self.tags_row.set_subtitle(&metadata.formatted_tags());
            }

            // Location
            if !metadata.country.is_empty() {
                self.location_group.set_visible(true);
                self.country_row.set_visible(true);
                self.country_row.set_subtitle(&metadata.country);
            }
            if !metadata.state.is_empty() {
                self.location_group.set_visible(true);
                self.state_row.set_visible(true);
                self.state_row.set_subtitle(&metadata.state);
            }

            // Map
            #[cfg(not(target_os = "windows"))]
            {
                let long: f64 = metadata.geo_long.unwrap_or(0.0).into();
                let lat: f64 = metadata.geo_lat.unwrap_or(0.0).into();
                if long != 0.0 || lat != 0.0 {
                    self.setup_map_widget(lat, long);
                    self.map_box.set_visible(true);
                }
            }

            // Codec
            if !metadata.codec.is_empty() {
                self.codec_row.set_visible(true);
                self.codec_row.set_subtitle(&metadata.codec);
            }

            // Bitrate
            if metadata.bitrate != 0 {
                self.bitrate_row.set_visible(true);
                let bitrate = formatx!(gettext("{} kbit/s"), metadata.bitrate).unwrap();
                self.bitrate_row.set_subtitle(&bitrate);
            }

            // Stream url
            let url = if let Some(url_resolved) = metadata.url_resolved {
                url_resolved.to_string()
            } else {
                metadata.url.map(|x| x.to_string()).unwrap_or_default()
            };
            let url = url.replace('&', "&amp;");
            let subtitle = format!("<a href=\"{}\">{}</a>", url, url);

            self.stream_row.set_subtitle(&subtitle);
            self.stream_row.set_tooltip_text(Some(&url));
        }

        #[cfg(not(target_os = "windows"))]
        fn setup_map_widget(&self, lat: f64, long: f64) {
            let registry = shumate::MapSourceRegistry::with_defaults();
            let source = registry.by_id(shumate::MAP_SOURCE_OSM_MAPNIK).unwrap();

            let map = shumate::Map::new();
            map.set_sensitive(false);
            map.set_height_request(294);
            map.set_map_source(&source);

            let viewport = map.viewport().unwrap();
            viewport.set_reference_map_source(Some(&source));
            viewport.set_zoom_level(6.0);

            let layer = shumate::MapLayer::new(&source, &viewport);
            map.add_layer(&layer);

            let marker = shumate::Marker::default();
            let marker_layer = shumate::MarkerLayer::new(&viewport);
            marker_layer.add_marker(&marker);
            map.add_layer(&marker_layer);

            let marker_img = gtk::Image::from_icon_name("mark-location-symbolic");
            marker_img.add_css_class("map-pin");
            marker_img.set_icon_size(gtk::IconSize::Large);
            marker.set_child(Some(&marker_img));

            marker.set_location(lat, long);
            map.center_on(lat, long);

            let frame = gtk::Frame::new(None);
            frame.set_halign(gtk::Align::Fill);
            frame.set_child(Some(&map));

            let license = shumate::License::new();
            license.set_xalign(0.0);
            license.append_map_source(&source);

            self.map_box.append(&frame);
            self.map_box.append(&license);
        }

        #[template_callback]
        fn add_station(&self) {
            let obj = self.obj();

            let station = obj.station();
            SwApplication::default().library().add_station(station);

            obj.close();
        }

        #[template_callback]
        fn remove_station(&self) {
            let obj = self.obj();

            let station = obj.station();
            SwApplication::default()
                .library()
                .remove_stations(vec![station]);

            obj.close();
        }

        #[template_callback]
        async fn start_playback(&self) {
            let obj = self.obj();
            let station = obj.station();

            let player = SwApplication::default().player();
            player.set_station(station).await;
            player.start_playback().await;

            obj.close();
        }

        #[template_callback]
        fn copy_stream_clipboard(&self) {
            let metadata = self.obj().station().metadata();

            if let Some(url_resolved) = metadata.url_resolved {
                let display = gdk::Display::default().unwrap();
                let clipboard = display.clipboard();
                clipboard.set_text(url_resolved.as_ref());

                let toast = adw::Toast::new(&gettext("Copied"));
                self.toast_overlay.add_toast(toast);
            }
        }
    }
}

glib::wrapper! {
    pub struct SwStationDialog(ObjectSubclass<imp::SwStationDialog>)
        @extends gtk::Widget, adw::Dialog,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl SwStationDialog {
    pub fn new(station: &SwStation) -> Self {
        glib::Object::builder().property("station", station).build()
    }
}

impl ToastWindow for SwStationDialog {
    fn toast_overlay(&self) -> adw::ToastOverlay {
        self.imp().toast_overlay.clone()
    }
}
