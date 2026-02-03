zsh_completion_dir = get_option('zsh_completion_dir')

if zsh_completion_dir == ''
  zsh_completion_dir = get_option('datadir') / 'zsh' / 'site-functions'
endif

install_data('_bwrap', install_dir : zsh_completion_dir)
