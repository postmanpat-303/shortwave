// Shortwave - about_dialog.rs
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

use adw::prelude::*;

use crate::config;
use crate::i18n::i18n;
use crate::ui::SwApplicationWindow;

pub fn show(parent: &SwApplicationWindow) {
    #[cfg(not(target_os = "windows"))]
    let dialog = adw::AboutDialog::from_appdata(
        &format!("{}/metainfo.xml", *config::PATH_ID),
        Some(*config::VERSION),
    );
    #[cfg(target_os = "windows")]
    let dialog = adw::AboutDialog::builder()
        .application_name(*config::NAME)
        .application_icon(*config::APP_ID)
        .developer_name("Felix Häcker")
        .license_type(gtk::License::Gpl30)
        .website("https://apps.gnome.org/Shortwave/")
        .issue_url("https://gitlab.gnome.org/World/Shortwave/issues")
        .build();

    if *config::PROFILE == "development" {
        dialog.set_version(&format!(
            "{}-{} (devel)",
            *config::VERSION,
            *config::VCS_TAG
        ));
    } else {
        dialog.set_version(*config::VERSION);
    }

    dialog.set_developers(&[
        "Felix Häcker <haeckerfelix@gnome.org>",
        "Maximiliano Sandoval <msandova@gnome.org>",
        "Elias Projahn",
    ]);
    dialog.set_designers(&["Tobias Bernard"]);
    dialog.set_translator_credits(&i18n("translator-credits"));
    dialog.add_link(&i18n("Donate"), "https://liberapay.com/haecker-felix");

    dialog.present(Some(parent));
}
