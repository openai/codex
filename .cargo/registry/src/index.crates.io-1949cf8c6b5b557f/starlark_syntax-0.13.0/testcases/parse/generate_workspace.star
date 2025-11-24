# @generated
# The following dependencies were calculated from:
#
# generate_workspace -a org.apache.maven:maven-artifact:3.5.0 -a org.apache.maven:maven-aether-provider:3.3.9 -a org.eclipse.aether:aether-util:1.1.0 -a com.google.guava:guava:20.0 -a org.powermock:powermock-module-junit4:1.6.6 -a com.google.code.findbugs:jsr305:3.0.1 -a org.codehaus.plexus:plexus-utils:jar:3.0.24 -a org.codehaus.plexus:plexus-component-annotations:1.7.1 -a org.codehaus.plexus:plexus-interpolation:1.24 -a org.mockito:mockito-all:1.9.5 -a junit:junit:4.4 -a com.google.truth:truth:0.30 -a org.apache.httpcomponents:httpcore:4.4.6 -a org.apache.httpcomponents:httpclient:4.5.3


def generated_maven_jars():
  native.maven_jar(
      name = "com_google_code_findbugs_jsr305",
      artifact = "com.google.code.findbugs:jsr305:3.0.1",
  )


  # org.apache.httpcomponents:httpclient:jar:4.5.3 got requested version
  native.maven_jar(
      name = "org_apache_httpcomponents_httpcore",
      artifact = "org.apache.httpcomponents:httpcore:4.4.6",
  )


  # org.powermock:powermock-core:jar:1.6.6
  # org.powermock:powermock-module-junit4-common:jar:1.6.6 got requested version
  native.maven_jar(
      name = "org_powermock_powermock_reflect",
      artifact = "org.powermock:powermock-reflect:1.6.6",
      sha1 = "3fa5d0acee85c5662102ab2ef7a49bbb5a56bae5",
  )


  # org.apache.httpcomponents:httpclient:jar:4.5.3
  native.maven_jar(
      name = "commons_codec_commons_codec",
      artifact = "commons-codec:commons-codec:1.9",
      sha1 = "9ce04e34240f674bc72680f8b843b1457383161a",
  )


  native.maven_jar(
      name = "org_apache_maven_maven_aether_provider",
      artifact = "org.apache.maven:maven-aether-provider:3.3.9",
  )


  native.maven_jar(
      name = "org_powermock_powermock_module_junit4",
      artifact = "org.powermock:powermock-module-junit4:1.6.6",
  )


  native.maven_jar(
      name = "org_apache_maven_maven_artifact",
      artifact = "org.apache.maven:maven-artifact:3.5.0",
  )


  native.maven_jar(
      name = "com_google_truth_truth",
      artifact = "com.google.truth:truth:0.30",
  )


  # com.google.truth:truth:jar:0.30
  native.maven_jar(
      name = "com_google_errorprone_error_prone_annotations",
      artifact = "com.google.errorprone:error_prone_annotations:2.0.8",
      sha1 = "54e2d56cb157df08cbf183149bcf50c9f5151ed4",
  )


  native.maven_jar(
      name = "org_codehaus_plexus_plexus_interpolation",
      artifact = "org.codehaus.plexus:plexus-interpolation:1.24",
  )


  # org.powermock:powermock-reflect:jar:1.6.6
  native.maven_jar(
      name = "org_objenesis_objenesis",
      artifact = "org.objenesis:objenesis:2.4",
      sha1 = "2916b6c96b50c5b3ec4452ed99401db745aabb27",
  )


  # com.google.truth:truth:jar:0.30 wanted version 4.10
  native.maven_jar(
      name = "junit_junit",
      artifact = "junit:junit:4.4",
  )


  # org.powermock:powermock-module-junit4-common:jar:1.6.6
  native.maven_jar(
      name = "org_powermock_powermock_core",
      artifact = "org.powermock:powermock-core:1.6.6",
      sha1 = "8085fae46f60d7ff960f1cc711359c00b35c5887",
  )


  native.maven_jar(
      name = "org_codehaus_plexus_plexus_component_annotations",
      artifact = "org.codehaus.plexus:plexus-component-annotations:1.7.1",
  )


  native.maven_jar(
      name = "org_codehaus_plexus_plexus_utils",
      artifact = "org.codehaus.plexus:plexus-utils:3.0.24",
  )


  # org.apache.httpcomponents:httpclient:jar:4.5.3
  native.maven_jar(
      name = "commons_logging_commons_logging",
      artifact = "commons-logging:commons-logging:1.2",
      sha1 = "4bfc12adfe4842bf07b657f0369c4cb522955686",
  )


  # junit:junit:jar:4.12
  # junit:junit:jar:4.12 got requested version
  native.maven_jar(
      name = "org_hamcrest_hamcrest_core",
      artifact = "org.hamcrest:hamcrest-core:1.3",
      sha1 = "42a25dc3219429f0e5d060061f71acb49bf010a0",
  )


  native.maven_jar(
      name = "org_eclipse_aether_aether_util",
      artifact = "org.eclipse.aether:aether-util:1.1.0",
  )


  # org.powermock:powermock-module-junit4:jar:1.6.6
  native.maven_jar(
      name = "org_powermock_powermock_module_junit4_common",
      artifact = "org.powermock:powermock-module-junit4-common:1.6.6",
      sha1 = "6302c934d03f76fa348ec91c603e11ce05b61f44",
  )


  native.maven_jar(
      name = "org_apache_httpcomponents_httpclient",
      artifact = "org.apache.httpcomponents:httpclient:4.5.3",
  )


  native.maven_jar(
      name = "org_mockito_mockito_all",
      artifact = "org.mockito:mockito-all:1.9.5",
  )


  # org.apache.maven:maven-artifact:jar:3.5.0
  native.maven_jar(
      name = "org_apache_commons_commons_lang3",
      artifact = "org.apache.commons:commons-lang3:3.5",
      sha1 = "6c6c702c89bfff3cd9e80b04d668c5e190d588c6",
  )


  # org.powermock:powermock-core:jar:1.6.6
  native.maven_jar(
      name = "org_javassist_javassist",
      artifact = "org.javassist:javassist:3.22.0-CR2",
      sha1 = "44eaf0990dea92f4bca4b9931b2239c0e8756ee7",
  )


  # org.eclipse.aether:aether-util:jar:1.1.0
  native.maven_jar(
      name = "org_eclipse_aether_aether_api",
      artifact = "org.eclipse.aether:aether-api:1.1.0",
      sha1 = "05dd291e788f50dfb48822dab29defc16ad70860",
  )


  # com.google.truth:truth:jar:0.30 wanted version 19.0
  native.maven_jar(
      name = "com_google_guava_guava",
      artifact = "com.google.guava:guava:20.0",
  )




def generated_java_libraries():
  native.java_library(
      name = "com_google_code_findbugs_jsr305",
      visibility = ["//visibility:public"],
      exports = ["@com_google_code_findbugs_jsr305//jar"],
  )


  native.java_library(
      name = "org_apache_httpcomponents_httpcore",
      visibility = ["//visibility:public"],
      exports = ["@org_apache_httpcomponents_httpcore//jar"],
  )


  native.java_library(
      name = "org_powermock_powermock_reflect",
      visibility = ["//visibility:public"],
      exports = ["@org_powermock_powermock_reflect//jar"],
      runtime_deps = [
          ":org_objenesis_objenesis",
      ],
  )


  native.java_library(
      name = "commons_codec_commons_codec",
      visibility = ["//visibility:public"],
      exports = ["@commons_codec_commons_codec//jar"],
  )


  native.java_library(
      name = "org_apache_maven_maven_aether_provider",
      visibility = ["//visibility:public"],
      exports = ["@org_apache_maven_maven_aether_provider//jar"],
  )


  native.java_library(
      name = "org_powermock_powermock_module_junit4",
      visibility = ["//visibility:public"],
      exports = ["@org_powermock_powermock_module_junit4//jar"],
      runtime_deps = [
          ":junit_junit",
          ":org_hamcrest_hamcrest_core",
          ":org_javassist_javassist",
          ":org_objenesis_objenesis",
          ":org_powermock_powermock_core",
          ":org_powermock_powermock_module_junit4_common",
          ":org_powermock_powermock_reflect",
      ],
  )


  native.java_library(
      name = "org_apache_maven_maven_artifact",
      visibility = ["//visibility:public"],
      exports = ["@org_apache_maven_maven_artifact//jar"],
      runtime_deps = [
          ":org_apache_commons_commons_lang3",
          ":org_codehaus_plexus_plexus_utils",
      ],
  )


  native.java_library(
      name = "com_google_truth_truth",
      visibility = ["//visibility:public"],
      exports = ["@com_google_truth_truth//jar"],
      runtime_deps = [
          ":com_google_errorprone_error_prone_annotations",
          ":com_google_guava_guava",
          ":junit_junit",
      ],
  )


  native.java_library(
      name = "com_google_errorprone_error_prone_annotations",
      visibility = ["//visibility:public"],
      exports = ["@com_google_errorprone_error_prone_annotations//jar"],
  )


  native.java_library(
      name = "org_codehaus_plexus_plexus_interpolation",
      visibility = ["//visibility:public"],
      exports = ["@org_codehaus_plexus_plexus_interpolation//jar"],
  )


  native.java_library(
      name = "org_objenesis_objenesis",
      visibility = ["//visibility:public"],
      exports = ["@org_objenesis_objenesis//jar"],
  )


  native.java_library(
      name = "junit_junit",
      visibility = ["//visibility:public"],
      exports = ["@junit_junit//jar"],
      runtime_deps = [
          ":org_hamcrest_hamcrest_core",
      ],
  )


  native.java_library(
      name = "org_powermock_powermock_core",
      visibility = ["//visibility:public"],
      exports = ["@org_powermock_powermock_core//jar"],
      runtime_deps = [
          ":org_javassist_javassist",
          ":org_objenesis_objenesis",
          ":org_powermock_powermock_reflect",
      ],
  )


  native.java_library(
      name = "org_codehaus_plexus_plexus_component_annotations",
      visibility = ["//visibility:public"],
      exports = ["@org_codehaus_plexus_plexus_component_annotations//jar"],
  )


  native.java_library(
      name = "org_codehaus_plexus_plexus_utils",
      visibility = ["//visibility:public"],
      exports = ["@org_codehaus_plexus_plexus_utils//jar"],
  )


  native.java_library(
      name = "commons_logging_commons_logging",
      visibility = ["//visibility:public"],
      exports = ["@commons_logging_commons_logging//jar"],
  )


  native.java_library(
      name = "org_hamcrest_hamcrest_core",
      visibility = ["//visibility:public"],
      exports = ["@org_hamcrest_hamcrest_core//jar"],
  )


  native.java_library(
      name = "org_eclipse_aether_aether_util",
      visibility = ["//visibility:public"],
      exports = ["@org_eclipse_aether_aether_util//jar"],
      runtime_deps = [
          ":org_eclipse_aether_aether_api",
      ],
  )


  native.java_library(
      name = "org_powermock_powermock_module_junit4_common",
      visibility = ["//visibility:public"],
      exports = ["@org_powermock_powermock_module_junit4_common//jar"],
      runtime_deps = [
          ":junit_junit",
          ":org_javassist_javassist",
          ":org_objenesis_objenesis",
          ":org_powermock_powermock_core",
          ":org_powermock_powermock_reflect",
      ],
  )


  native.java_library(
      name = "org_apache_httpcomponents_httpclient",
      visibility = ["//visibility:public"],
      exports = ["@org_apache_httpcomponents_httpclient//jar"],
      runtime_deps = [
          ":commons_codec_commons_codec",
          ":commons_logging_commons_logging",
          ":org_apache_httpcomponents_httpcore",
      ],
  )


  native.java_library(
      name = "org_mockito_mockito_all",
      visibility = ["//visibility:public"],
      exports = ["@org_mockito_mockito_all//jar"],
  )


  native.java_library(
      name = "org_apache_commons_commons_lang3",
      visibility = ["//visibility:public"],
      exports = ["@org_apache_commons_commons_lang3//jar"],
  )


  native.java_library(
      name = "org_javassist_javassist",
      visibility = ["//visibility:public"],
      exports = ["@org_javassist_javassist//jar"],
  )


  native.java_library(
      name = "org_eclipse_aether_aether_api",
      visibility = ["//visibility:public"],
      exports = ["@org_eclipse_aether_aether_api//jar"],
  )


  native.java_library(
      name = "com_google_guava_guava",
      visibility = ["//visibility:public"],
      exports = ["@com_google_guava_guava//jar"],
  )
