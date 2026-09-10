use std::path::{Path, PathBuf};
use std::sync::LazyLock;

/// Reads the compile-time environment variable `MESON_<name>`.
///
/// Panics on first access if the environment variable was not set during compilation.
macro_rules! meson_env {
    ($name:ident) => {{
        #[expect(clippy::option_env_unwrap)]
        let value = option_env!(concat!("MESON_", stringify!($name))).expect(concat!(
            "MESON_",
            stringify!($name),
            " was not set at compile time"
        ));
        value
    }};
}

static PREFIX: LazyLock<PathBuf> = LazyLock::new(|| {
    std::env::current_exe()
        .unwrap()
        .ancestors()
        .nth(Path::new(meson_env!(BINDIR)).components().count() + 1)
        .expect("executable is not inside the expected <prefix>/<bindir> layout")
        .to_path_buf()
});

/// Resolves a relative path at runtime against [`PREFIX`] so the binary embeds no absolute paths.
macro_rules! config_path {
    ($name:ident) => {
        pub static $name: LazyLock<PathBuf> = LazyLock::new(|| PREFIX.join(meson_env!($name)));
    };
}

config_path!(LOCALEDIR);
config_path!(DATADIR);

/// Declares a lazily-initialized static variable that reads its value from [`meson_env!`].
macro_rules! config_var {
    ($name:ident) => {
        pub static $name: LazyLock<&'static str> = LazyLock::new(|| meson_env!($name));
    };
}

config_var!(NAME);
config_var!(PKGNAME);
config_var!(APP_ID);
config_var!(PATH_ID);
config_var!(VERSION);
config_var!(PROFILE);
config_var!(VCS_TAG);
