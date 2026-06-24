BWRAP_EXEC_PROPERTIES = {
    # BuildBuddy's isolated network mode disables loopback, which the runner uses to reach exec-server.
    "test.network": "external",
    # A fresh VM isolates the deliberately writable outer namespace and permits nested user namespaces.
    "test.workload-isolation-type": "firecracker",
}
