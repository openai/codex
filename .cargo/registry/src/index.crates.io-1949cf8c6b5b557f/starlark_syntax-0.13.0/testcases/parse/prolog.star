# @generated
# Copyright (C) 2016 The Android Open Source Project
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
# http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

load("//tools/bzl:genrule2.bzl", "genrule2")

def prolog_cafe_library(
    name,
    srcs,
    deps = [],
    **kwargs):
  genrule2(
    name = name + '__pl2j',
    cmd = '$(location //lib/prolog:compiler_bin) ' +
      '$$(dirname $@) $@ ' +
      '$(SRCS)',
    srcs = srcs,
    tools = ['//lib/prolog:compiler_bin'],
    outs = [ name + '.srcjar' ],
  )
  native.java_library(
    name = name,
    srcs = [':' + name + '__pl2j'],
    deps = ['//lib/prolog:runtime'] + deps,
    **kwargs
  )
