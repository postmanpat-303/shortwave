# Setup cargo
cargo = find_program('cargo')
cargo_build_script = find_program(meson.project_source_root() / 'build-aux' / 'cargo_build.py')

cargo_env = environment()
cargo_env.set('CARGO_HOME', meson.project_build_root() / 'cargo-home')
cargo_env.set('MESON_NAME', name)
cargo_env.set('MESON_PKGNAME', meson.project_name())
cargo_env.set('MESON_APP_ID', app_id)
cargo_env.set('MESON_PATH_ID', path_id)
cargo_env.set('MESON_VERSION', meson.project_version())
cargo_env.set('MESON_PROFILE', profile)
cargo_env.set('MESON_VCS_TAG', vcs_tag)
cargo_env.set('MESON_LOCALEDIR', localedir)
cargo_env.set('MESON_DATADIR', datadir)

cargo_options = [ '--manifest-path', meson.project_source_root() / 'Cargo.toml' ]
cargo_options += [ '--target-dir', meson.project_build_root() / 'target' ]

if get_option('profile') == 'default'
  cargo_options += [ '--release' ]
  rust_target = 'release'
  message('Building in release mode')
else
  rust_target = 'debug'
  message('Building in debug mode')
endif

exe_suffix = (host_machine.system() == 'windows') ? '.exe' : ''

cargo_release = custom_target(
  'cargo-build',
  build_by_default: true,
  build_always_stale: true,
  output: meson.project_name() + exe_suffix,
  console: true,
  install: true,
  install_dir: bindir,
  env: cargo_env,
  command: [
    cargo_build_script,
    '--cargo', cargo,
    '--binary', 'target' / rust_target / (meson.project_name() + exe_suffix),
    '--output', '@OUTPUT@',
    '--', cargo_options,
  ]
)