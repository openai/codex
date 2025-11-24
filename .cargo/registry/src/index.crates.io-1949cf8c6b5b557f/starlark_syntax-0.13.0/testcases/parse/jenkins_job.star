# @generated
# Copyright 2017 The Bazel Authors. All rights reserved.
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

# Jenkins job creation

load(":templates.bzl", "expand_template")
load(":vars.bzl", "MAIL_SUBSTITUTIONS")

def _to_groovy_list(lst):
  return "[%s]" % (",".join(['"%s"' % e for e in lst]))

def jenkins_job(name, config, substitutions = {}, deps = [], deps_aliases = {},
                project='bazel', org='bazelbuild', git_url=None, project_url=None,
                folder=None, test_platforms=["linux-x86_64"],
                create_filegroups=True):
  """Create a job configuration on Jenkins.

  Args:
     name: the name of the job to create
     config: the configuration file for the job
     substitutions: additional substitutions to pass to the template generation
     deps: list of dependencies (templates included by the config file)
     project: the project name on github
     org: the project organization on github, default 'bazelbuild'
     git_url: the URL to the git project, defaulted to the Github URL
     project_url: the project url, defaulted to the Git URL
     test_platforms: platforms on which to run that job when inside of a
       dockerized test, by default only 'linux-x86_64'
     create_filegroups: create filegroups named <name>/all, <name>/staging
       and <name>/test that contains the files needed to be included
       to include that job respectively for the production service, the
       staging service and the docker test version. This is to be set
       to false is the calling macros already creates those filegroups.
  """
  github_project =  "%s/%s" % (org, project)
  github_url = "https://github.com/" + github_project
  if not git_url:
    git_url = github_url
  if not project_url:
    project_url = git_url
  deps = deps + [deps_aliases[k] for k in deps_aliases]
  substitutions = substitutions + {
      "GITHUB_URL": github_url,
      "GIT_URL": git_url,
      "GITHUB_PROJECT": github_project,
      "PROJECT_URL": project_url,
      "production": "true",
      } + MAIL_SUBSTITUTIONS
  substitutions["SEND_EMAIL"] = "1"
  # RESTRICT_CONFIGURATION can be use to restrict configuration of the groovy jobs
  if (not "RESTRICT_CONFIGURATION" in substitutions) or (
      not substitutions["RESTRICT_CONFIGURATION"]):
    substitutions["RESTRICT_CONFIGURATION"] = "[:]"
  expand_template(
      name = name,
      template = config,
      out = "%s.xml" % name,
      deps = deps,
      deps_aliases = deps_aliases,
      substitutions = substitutions,
    )
  if create_filegroups:
    native.filegroup(name = name + "/all", srcs = [name])
  substitutions["SEND_EMAIL"] = "0"
  substitutions["BAZEL_BUILD_RECIPIENT"] = ""
  substitutions["production"] = "false"
  expand_template(
      name = name + "-staging",
      template = config,
      out = "%s-staging.xml" % name,
      deps = deps,
      deps_aliases = deps_aliases,
      substitutions = substitutions,
    )
  if create_filegroups:
    native.filegroup(name = name + "/staging", srcs = [name + "-staging"])

  if test_platforms:
    substitutions["RESTRICT_CONFIGURATION"] += " + [node:%s]" % _to_groovy_list(test_platforms)
    expand_template(
      name = name + "-test",
      template = config,
      out = "%s-test.xml" % name,
      deps = deps,
      deps_aliases = deps_aliases,
      substitutions = substitutions,
    )
    if create_filegroups:
      native.filegroup(name = name + "/test", srcs = [name + "-test"])

def bazel_git_job(**kwargs):
  """Override bazel_github_job to test a project that is not on GitHub."""
  kwargs["github_enabled"] = False
  if not "git_url" in kwargs:
    if not "project_url" in kwargs:
      fail("Neither project_url nor git_url was specified")
    kwargs["git_url"] = kwargs
  bazel_github_job(**kwargs)

def bazel_github_job(name, branch="master", project=None, org="bazelbuild",
                     project_url=None, workspace=".", git_url=None,
                     config="//jenkins/build_defs:default.json",
                     test_platforms=["linux-x86_64"],
                     enable_trigger=True,
                     poll=None,
                     gerrit_project=None,
                     enabled=True,
                     pr_enabled=True,
                     github_enabled=True,
                     run_sequential=False,
                     sauce_enabled=False,
                     use_upstream_branch=False):
  """Create a generic github job configuration to build against Bazel head."""
  if poll == None:
    poll = org != "bazelbuild"
  if not project:
    project = name

  substitutions = {
    "WORKSPACE": workspace,
    "PROJECT_NAME": project,
    "BRANCH": branch,
    "NAME": name,
    "disabled": str(not enabled).lower(),
    "enable_trigger": str(enable_trigger and github_enabled).lower(),
    "poll": str(poll).lower(),
    "github": str(github_enabled),
    "GERRIT_PROJECT": str(gerrit_project) if gerrit_project else "",
    "RUN_SEQUENTIAL": str(run_sequential).lower(),
    "SAUCE_ENABLED": str(sauce_enabled).lower(),
    "GLOBAL_USE_UPSTREAM_BRANCH": str(use_upstream_branch)
  }

  all_files = [name + ".xml"]
  test_files = [name + "-test.xml"]
  staging_files = [name + "-staging.xml"]

  kwargs = {}
  if not github_enabled:
    kwargs["git_url"] = git_url

  jenkins_job(
      name = name,
      config = "//jenkins/build_defs:bazel-job.xml.tpl",
      deps_aliases = {
        "JSON_CONFIGURATION": config,
      },
      substitutions=substitutions,
      project=project,
      org=org,
      project_url=project_url,
      test_platforms=test_platforms,
      create_filegroups=False,
      **kwargs)

  if enabled and config:
    jenkins_job(
        name = "Global/" + name,
        config = "//jenkins/build_defs:bazel-job-Global.xml.tpl",
        deps_aliases = {
          "JSON_CONFIGURATION": config,
        },
        substitutions=substitutions,
        git_url=git_url,
        project=project,
        org=org,
        project_url=project_url,
        test_platforms=test_platforms,
        create_filegroups=False)
    all_files.append("Global/%s.xml" % name)
    test_files.append("Global/%s-test.xml" % name)
    staging_files.append("Global/%s-staging.xml" % name)

  if pr_enabled and config:
    jenkins_job(
        name = "PR/" + name,
        config = "//jenkins/build_defs:bazel-job-PR.xml.tpl",
        deps_aliases = {
          "JSON_CONFIGURATION": config,
        },
        substitutions=substitutions,
        project=project,
        org=org,
        project_url=project_url,
        test_platforms=test_platforms,
        create_filegroups=False)
    all_files.append("PR/%s.xml" % name)
    test_files.append("PR/%s-test.xml" % name)
    staging_files.append("PR/%s-staging.xml" % name)

  if gerrit_project:
    jenkins_job(
        name = "CR/" + name,
        config = "//jenkins/build_defs:bazel-job-Gerrit.xml.tpl",
        deps_aliases = {
          "JSON_CONFIGURATION": config,
        },
        substitutions=substitutions,
        project=project,
        org=org,
        project_url=project_url,
        test_platforms=test_platforms)
    all_files.append("CR/%s.xml" % name)
    test_files.append("CR/%s-test.xml" % name)
    staging_files.append("CR/%s-staging.xml" % name)

  native.filegroup(name = "%s/all" % name, srcs = all_files)
  if test_platforms:
    native.filegroup(name = "%s/test" % name, srcs = test_files)
  native.filegroup(name = "%s/staging" % name, srcs = staging_files)
