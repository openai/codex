# @generated
def _is_staging(job):
  job_desc = native.existing_rule(job + "-staging")
  job_subs = job_desc["substitutions"]
  is_bazel = "PROJECT_NAME" in job_subs
  is_gerrit = "GERRIT_PROJECT" in job_subs and job_subs["GERRIT_PROJECT"] != ""
  # Take job with Gerrit review, or jobs that are not bazel jovbs
  is_gerrit_or_not_bazel = is_gerrit or not is_bazel
  # Gold jobs are some bazel job that we include for testing
  is_gold = job in ["TensorFlow", "Tutorial", "rules_k8s", "rules_python"]
  return (is_gold or is_gerrit_or_not_bazel)


def _is_testing(job):
  # We include all test but the docker ones (they needs access to the docker server).
  return not "docker" in job and job != "continuous-integration"


def job_lists(name = "jobs", visibility = None):
  jobs = native.existing_rules()

  native.filegroup(
    name = name,
    srcs = [j for j in jobs if j.endswith("/all")],
    visibility = visibility,
  )

  native.filegroup(
    name = "staging-" + name,
    srcs = [j for j in jobs if j.endswith("/staging") and _is_staging(j[:-8])],
    visibility = visibility,
  )

  native.filegroup(
    name = "test-" + name,
    srcs = [j for j in jobs if j.endswith("/test") and _is_testing(j[:-5])],
    visibility = visibility,
  )
