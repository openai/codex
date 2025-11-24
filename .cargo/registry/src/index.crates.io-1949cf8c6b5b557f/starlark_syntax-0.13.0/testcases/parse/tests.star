# @generated
load("//tools/bzl:junit.bzl", "junit_tests")

def acceptance_tests(
    group,
    deps = [],
    labels = [],
    vm_args = ['-Xmx256m'],
    **kwargs):
  junit_tests(
    name = group,
    deps = deps + [
      '//gerrit-acceptance-tests:lib',
    ],
    tags = labels + [
      'acceptance',
      'slow',
    ],
    size = "large",
    jvm_flags = vm_args,
    **kwargs
  )
