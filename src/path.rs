// Shortwave - path.rs
// Copyright (C) 2021  Felix Häcker <haeckerfelix@gnome.org>
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

use std::fs;
use std::path::PathBuf;
use std::sync::LazyLock;

use gtk::glib;

use crate::config;

pub static DATA: LazyLock<PathBuf> = LazyLock::new(|| {
    let mut path = glib::user_data_dir();
    path.push(*config::PKGNAME);
    path
});

pub static CACHE: LazyLock<PathBuf> = LazyLock::new(|| {
    let mut path = glib::user_cache_dir();
    path.push(*config::PKGNAME);
    path
});

pub fn init() -> std::io::Result<()> {
    migrate_legacy_paths();

    fs::create_dir_all(DATA.to_owned())?;
    fs::create_dir_all(CACHE.to_owned())?;
    Ok(())
}

/// Older versions used `config::NAME` instead of `config::PKGNAME`
/// TODO: Drop migration step again with next major release
fn migrate_legacy_paths() {
    let legacy_data = glib::user_data_dir().join(*config::NAME);
    if legacy_data.exists() && !DATA.exists() {
        info!("Migrate data directory...");
        let _ = fs::rename(&legacy_data, &*DATA);
    }

    let legacy_cache = glib::user_cache_dir().join(*config::NAME);
    if legacy_cache.exists() && !CACHE.exists() {
        info!("Migrate cache directory...");
        let _ = fs::rename(legacy_cache, &*CACHE);
    }

    let legacy_db = DATA.join(format!("{}.db", *config::NAME));
    let db = DATA.join(format!("{}.db", *config::PKGNAME));
    if legacy_db.exists() && !db.exists() {
        info!("Migrate database...");
        let _ = fs::rename(legacy_db, db);
    }
}
