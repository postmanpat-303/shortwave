project('shortwave', 'rust',
          version: '5.1.0',
    meson_version: '>= 0.59.0',
          license: 'GPL-3.0-or-later',
)

gnome = import('gnome')
i18n = import('i18n')

# Dependencies
dependency('sqlite3', version: '>= 3.20')
dependency('openssl', version: '>= 1.0')

dependency('glib-2.0', version: '>= 2.66')
dependency('gio-2.0', version: '>= 2.66')
dependency('gtk4', version: '>= 4.16')
dependency('libadwaita-1', version: '>= 1.8.alpha')
dependency('glycin-gtk4-2', version: '>= 2.1.0')

dependency('gstreamer-1.0', version: '>= 1.16')
dependency('gstreamer-base-1.0', version: '>= 1.16')
dependency('gstreamer-audio-1.0', version: '>= 1.16')
dependency('gstreamer-plugins-base-1.0', version: '>= 1.16')
dependency('gstreamer-plugins-bad-1.0', version: '>= 1.16')
dependency('gstreamer-bad-audio-1.0', version: '>= 1.16')

if host_machine.system() != 'windows'
  dependency('shumate-1.0', version: '>= 1.3')
endif

# Initialize project variables
name       = 'Shortwave'
base_id    = 'de.haeckerfelix.Shortwave'
path_id    = '/de/haeckerfelix/Shortwave'
app_id     = base_id
profile    = get_option('profile')
datadir    = join_paths(get_option('prefix'), get_option('datadir'))
bindir     = join_paths(get_option('prefix'), get_option('bindir'))

# Change app id for development builds
if profile == 'development'
  app_id = '@0@.Devel'.format(app_id)
endif

subdir('data')
subdir('src')
subdir('po')
