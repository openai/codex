project(
  'use-bubblewrap-as-subproject',
  'c',
  version : '0',
  meson_version : '>=0.49.0',
)

configure_file(
  output : 'config.h',
  input : 'dummy-config.h.in',
  configuration : configuration_data(),
)

subproject(
  'bubblewrap',
  default_options : [
    'install_rpath=${ORIGIN}/../lib',
    'program_prefix=not-flatpak-',
  ],
)
