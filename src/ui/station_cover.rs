// Shortwave - station_cover.rs
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

use std::cell::Cell;
use std::cell::RefCell;

use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::{Properties, clone, subclass};
use gtk::{CompositeTemplate, glib};

use crate::api::{StationCover, SwStation};
use crate::ui::SwScalableImage;

mod imp {
    use super::*;

    #[derive(Debug, Default, Properties, CompositeTemplate)]
    #[template(resource = "/de/haeckerfelix/Shortwave/gtk/station_cover.ui")]
    #[properties(wrapper_type = super::SwStationCover)]
    pub struct SwStationCover {
        #[template_child]
        image: TemplateChild<SwScalableImage>,

        #[property(get, set=Self::set_size)]
        size: Cell<i32>,
        #[property(get, set=Self::set_station, nullable)]
        station: RefCell<Option<SwStation>>,

        cover: RefCell<Option<StationCover>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwStationCover {
        const NAME: &'static str = "SwStationCover";
        type ParentType = adw::Bin;
        type Type = super::SwStationCover;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("cover");
            Self::bind_template(klass);
        }

        fn instance_init(obj: &subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for SwStationCover {}

    impl WidgetImpl for SwStationCover {
        fn map(&self) {
            self.parent_map();
            self.update_texture();
        }

        fn unmap(&self) {
            self.parent_unmap();
            self.cancel();
        }
    }

    impl BinImpl for SwStationCover {}

    impl SwStationCover {
        fn set_size(&self, size: i32) {
            self.size.set(size);

            self.image.set_size_request(size, size);
            self.obj().set_size_request(size, size);

            self.update_texture();
        }

        fn set_station(&self, station: Option<&SwStation>) {
            *self.station.borrow_mut() = station.cloned();

            if let Some(station) = self.obj().station() {
                station.connect_metadata_notify(clone!(
                    #[weak(rename_to = imp)]
                    self,
                    move |_| {
                        imp.update_texture();
                    }
                ));
                station.connect_custom_cover_notify(clone!(
                    #[weak(rename_to = imp)]
                    self,
                    move |_| {
                        imp.update_texture();
                    }
                ));
            }

            self.update_texture();
        }

        fn update_texture(&self) {
            let obj = self.obj();

            // Update texture, but only if it's mapped
            if !self.obj().is_mapped() {
                return;
            }

            self.cancel();

            let cover = StationCover::new(self.obj().station(), obj.size() * obj.scale_factor());
            let texture = cover.load_texture(clone!(
                #[weak(rename_to = imp)]
                self,
                move |texture| {
                    imp.image.set_texture(Some(&texture));
                }
            ));

            self.image.set_texture(Some(&texture));
            *self.cover.borrow_mut() = Some(cover);
        }

        fn cancel(&self) {
            self.cover.borrow_mut().take();
        }
    }
}

glib::wrapper! {
    pub struct SwStationCover(ObjectSubclass<imp::SwStationCover>)
        @extends gtk::Widget, adw::Bin,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for SwStationCover {
    fn default() -> Self {
        glib::Object::new()
    }
}

mod imp_animated {
    use super::*;

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::SwStationCoverAnimated)]
    pub struct SwStationCoverAnimated {
        cover1: SwStationCover,
        cover2: SwStationCover,
        stack: adw::ViewStack,

        #[property(get, set)]
        size: Cell<i32>,
        #[property(get, set=Self::set_station, nullable)]
        station: RefCell<Option<SwStation>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwStationCoverAnimated {
        const NAME: &'static str = "SwStationCoverAnimated";
        type ParentType = adw::Bin;
        type Type = super::SwStationCoverAnimated;
    }

    #[glib::derived_properties]
    impl ObjectImpl for SwStationCoverAnimated {
        fn constructed(&self) {
            self.parent_constructed();

            self.obj().set_child(Some(&self.stack));
            self.stack.add_named(&self.cover1, Some("cover1"));
            self.stack.add_named(&self.cover2, Some("cover2"));
            self.stack.set_enable_transitions(true);

            self.obj().set_halign(gtk::Align::Center);
            self.obj().set_valign(gtk::Align::Center);

            self.obj()
                .bind_property("size", &self.cover1, "size")
                .build();
            self.obj()
                .bind_property("size", &self.cover2, "size")
                .build();
        }
    }

    impl WidgetImpl for SwStationCoverAnimated {}

    impl BinImpl for SwStationCoverAnimated {}

    impl SwStationCoverAnimated {
        fn set_station(&self, station: Option<&SwStation>) {
            *self.station.borrow_mut() = station.cloned();

            let new_cover = if self.stack.visible_child_name().unwrap() == "cover1" {
                self.cover2.set_station(station);
                &self.cover2
            } else {
                self.cover1.set_station(station);
                &self.cover1
            };

            self.stack.set_visible_child(new_cover);
        }
    }
}

glib::wrapper! {
    /// Transitions between station covers. Only useful when the underlying [SwStation] can change, for example in player widgets.
    pub struct SwStationCoverAnimated(ObjectSubclass<imp_animated::SwStationCoverAnimated>)
        @extends gtk::Widget, adw::Bin,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}
