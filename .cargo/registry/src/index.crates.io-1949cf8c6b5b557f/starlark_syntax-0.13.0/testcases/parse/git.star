# @generated
# Copyright 2015 The Bazel Authors. All rights reserved.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#    http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
"""Rules for cloning external git repositories."""

def _clone_or_update(ctx):
  if (ctx.attr.verbose):
    print('git.bzl: Cloning or updating repository %s' % ctx.name)
  if ((not ctx.attr.tag and not ctx.attr.commit) or
      (ctx.attr.tag and ctx.attr.commit)):
    fail('Exactly one of commit and tag must be provided')
  if ctx.attr.commit:
    ref = ctx.attr.commit
  else:
    ref = 'tags/' + ctx.attr.tag

  st = ctx.execute(['bash', '-c', """
set -ex
( cd {working_dir} &&
    if ! ( cd '{dir}' && git rev-parse --git-dir ) >/dev/null 2>&1; then
      rm -rf '{dir}'
      git clone '{remote}' '{dir}'
    fi
    cd '{dir}'
    git reset --hard {ref} || (git fetch origin {ref}:{ref} && git reset --hard {ref})
    git clean -xdf )
  """.format(
      working_dir=ctx.path('.').dirname,
      dir=ctx.path('.'),
      remote=ctx.attr.remote,
      ref=ref,
  )])
  if st.return_code:
    fail('error cloning %s:\n%s' % (ctx.name, st.stderr))
  if ctx.attr.init_submodules:
    st = ctx.execute(['bash', '-c', """
set -ex
(   cd '{dir}'
    git submodule update --init --checkout --force )
  """.format(
      dir=ctx.path('.'),
  )])
  if st.return_code:
    fail('error updating submodules %s:\n%s' % (ctx.name, st.stderr))


def _new_git_repository_implementation(ctx):
  if ((not ctx.attr.build_file and not ctx.attr.build_file_content) or
      (ctx.attr.build_file and ctx.attr.build_file_content)):
    fail('Exactly one of build_file and build_file_content must be provided.')
  _clone_or_update(ctx)
  ctx.file('WORKSPACE', 'workspace(name = \'{name}\')\n'.format(name=ctx.name))
  if ctx.attr.build_file:
    ctx.symlink(ctx.attr.build_file, 'BUILD.bazel')
  else:
    ctx.file('BUILD.bazel', ctx.attr.build_file_content)

def _git_repository_implementation(ctx):
  _clone_or_update(ctx)


_common_attrs = {
    'remote': attr.string(mandatory=True),
    'commit': attr.string(default=''),
    'tag': attr.string(default=''),
    'init_submodules': attr.bool(default=False),
    'verbose': attr.bool(default=False),
}


new_git_repository = repository_rule(
    implementation=_new_git_repository_implementation,
    attrs=_common_attrs + {
        'build_file': attr.label(allow_single_file=True),
        'build_file_content': attr.string(),
    }
)
"""Clone an external git repository.

Clones a Git repository, checks out the specified tag, or commit, and
makes its targets available for binding.

Args:
  name: A unique name for this rule.

  build_file: The file to use as the BUILD file for this repository.
    Either build_file or build_file_content must be specified.

    This attribute is a label relative to the main workspace. The file
    does not need to be named BUILD, but can be (something like
    BUILD.new-repo-name may work well for distinguishing it from the
    repository's actual BUILD files.

  build_file_content: The content for the BUILD file for this repository.
    Either build_file or build_file_content must be specified.

  init_submodules: Whether to clone submodules in the repository.

  remote: The URI of the remote Git repository.
"""

git_repository = repository_rule(
    implementation=_git_repository_implementation,
    attrs=_common_attrs,
)
"""Clone an external git repository.

Clones a Git repository, checks out the specified tag, or commit, and
makes its targets available for binding.

Args:
  name: A unique name for this rule.

  init_submodules: Whether to clone submodules in the repository.

  remote: The URI of the remote Git repository.
"""
