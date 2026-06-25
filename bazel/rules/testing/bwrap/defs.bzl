BWRAP_EXEC_PROPERTIES = {
    # BuildBuddy's isolated network mode disables loopback, which the runner uses to reach exec-server.
    "test.network": "external",
    # A fresh VM isolates the deliberately writable outer namespace and permits nested user namespaces.
    "test.workload-isolation-type": "firecracker",
}

BWRAP_INTEGRATION_TEST_EXEC_PROPERTIES = BWRAP_EXEC_PROPERTIES | {
    # Round the full core/app-server runfiles plus per-test scratch space up to the 10 GB RBE tier.
    "test.EstimatedFreeDiskBytes": "10GB",
    # Round the overlapping Codex, exec-server, and test processes up to the 8 GB RBE tier.
    "test.EstimatedMemory": "8GB",
}
