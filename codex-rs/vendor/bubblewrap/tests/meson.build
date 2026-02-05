test_programs = [
  ['test-utils', executable(
    'test-utils',
    'test-utils.c',
    '../utils.c',
    '../utils.h',
    dependencies : [selinux_dep],
    include_directories : common_include_directories,
  )],
]

executable(
  'try-syscall',
  'try-syscall.c',
  override_options: ['b_sanitize=none'],
)

test_scripts = [
  'test-run.sh',
  'test-seccomp.py',
  'test-specifying-pidns.sh',
  'test-specifying-userns.sh',
]

test_env = environment()
test_env.set('BWRAP', bwrap.full_path())
test_env.set('G_TEST_BUILDDIR', meson.current_build_dir() / '..')
test_env.set('G_TEST_SRCDIR', meson.current_source_dir() / '..')

foreach pair : test_programs
  name = pair[0]
  test_program = pair[1]
  if meson.version().version_compare('>=0.50.0')
    test(
      name,
      test_program,
      env : test_env,
      protocol : 'tap',
    )
  else
    test(
      name,
      test_program,
      env : test_env,
    )
  endif
endforeach

foreach test_script : test_scripts
  if test_script.endswith('.py')
    interpreter = python
  else
    interpreter = bash
  endif

  if meson.version().version_compare('>=0.50.0')
    test(
      test_script,
      interpreter,
      args : [files(test_script)],
      env : test_env,
      protocol : 'tap',
    )
  else
    test(
      test_script,
      interpreter,
      args : [files(test_script)],
      env : test_env,
    )
  endif
endforeach
