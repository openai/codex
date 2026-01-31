bash_completion_dir = get_option('bash_completion_dir')

if bash_completion_dir == ''
  bash_completion = dependency(
    'bash-completion',
    version : '>=2.0',
    required : false,
  )

  if bash_completion.found()
    if meson.version().version_compare('>=0.51.0')
      bash_completion_dir = bash_completion.get_variable(
        default_value: '',
        pkgconfig: 'completionsdir',
        pkgconfig_define: [
          'datadir', get_option('prefix') / get_option('datadir'),
        ],
      )
    else
      bash_completion_dir = bash_completion.get_pkgconfig_variable(
        'completionsdir',
        default: '',
        define_variable: [
          'datadir', get_option('prefix') / get_option('datadir'),
        ],
      )
    endif
  endif
endif

if bash_completion_dir == ''
  bash_completion_dir = get_option('datadir') / 'bash-completion' / 'completions'
endif

install_data('bwrap', install_dir : bash_completion_dir)
