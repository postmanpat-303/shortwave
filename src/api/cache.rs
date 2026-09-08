// Shortwave - cache.rs
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

use std::time::{Duration, SystemTime};

use anyhow::Result;
use cacache::Algorithm;

use crate::path;

pub async fn read(key: &str) -> Result<Vec<u8>> {
    Ok(cacache::read(&*path::CACHE, key).await?)
}

pub fn read_sync(key: &str) -> Result<Vec<u8>> {
    Ok(cacache::read_sync(&*path::CACHE, key)?)
}

pub async fn write(key: &str, data: &[u8]) -> Result<()> {
    cacache::write_with_algo(Algorithm::Xxh3, &*path::CACHE, key, data).await?;
    Ok(())
}

pub fn prune() {
    let ttl = Duration::from_secs(86400 * 30);

    for md in cacache::list_sync(&*path::CACHE).flatten() {
        let now = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();

        let age = Duration::from_millis((now - md.time).try_into().unwrap_or_default());
        if age > ttl {
            let _ = cacache::remove_hash_sync(&*path::CACHE, &md.integrity);
            let _ = cacache::remove_sync(&*path::CACHE, &md.key);
        }
    }
}
