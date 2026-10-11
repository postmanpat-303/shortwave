# Setup cargo
cargo = find_program('cargo')
cargo_build_script = find_program(meson.project_source_root() / 'build-aux' / 'cargo_build.py')

cargo_env = environment()
cargo_env.set('CARGO_HOME', meson.project_build_root() / 'cargo-home')
cargo_env.set('CARGO_TARGET_DIR', meson.project_build_root() / 'target')
cargo_env.set('MESON_NAME', name)
cargo_env.set('MESON_PKGNAME', pkgname)
cargo_env.set('MESON_APP_ID', app_id)
cargo_env.set('MESON_PATH_ID', path_id)
cargo_env.set('MESON_VERSION', meson.project_version())
cargo_env.set('MESON_PROFILE', profile)
cargo_env.set('MESON_VCS_TAG', run_command('git', 'rev-parse', '--short', 'HEAD', check: false).stdout().strip())
cargo_env.set('MESON_BINDIR', get_option('bindir'))
cargo_env.set('MESON_LOCALEDIR', get_option('localedir'))
cargo_env.set('MESON_DATADIR', get_option('datadir'))

cargo_options = [ '--manifest-path', meson.project_source_root() / 'Cargo.toml' ]

if profile == 'default'
  cargo_options += [ '--release' ]
  cargo_profile = 'release'
  message('Building in release mode')
else
  cargo_profile = 'debug'
  message('Building in debug mode')
endif

exe_suffix = (host_machine.system() == 'windows') ? '.exe' : ''

custom_target(
  'cargo-build',
  build_by_default: true,
  build_always_stale: true,
  output: pkgname + exe_suffix,
  console: true,
  install: true,
  install_dir: bindir,
  env: cargo_env,
  command: [
    cargo_build_script,
    '--cargo', cargo,
    '--profile', cargo_profile,
    '--binary', meson.project_name() + exe_suffix,
    '--output', '@OUTPUT@',
    '--', cargo_options,
  ]
)