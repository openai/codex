import { isSafeCommand, canAutoApprove } from "../src/approvals.js";
import { expect, test, describe } from "vitest";

describe("Git & GitHub CLI approval configuration", () => {
  test("Git commands respect requireApprovalByDefault setting", () => {
    // Create a test config
    const testConfig = {
      git: {
        requireApprovalByDefault: true,
        autoApprovedCommands: ["status", "log"],
        requireApprovalCommands: ["commit", "push"]
      }
    };
    
    // When requireApprovalByDefault is true, commands not in autoApprovedCommands should require approval
    expect(isSafeCommand(["git", "status"], testConfig)).not.toBeNull();
    expect(isSafeCommand(["git", "log"], testConfig)).not.toBeNull();
    expect(isSafeCommand(["git", "commit"], testConfig)).toBeNull();
    expect(isSafeCommand(["git", "push"], testConfig)).toBeNull();
    expect(isSafeCommand(["git", "branch"], testConfig)).toBeNull(); // Not explicitly allowed or denied
    
    // Flip the default setting
    testConfig.git.requireApprovalByDefault = false;
    
    // Now commands not explicitly denied should be auto-approved
    expect(isSafeCommand(["git", "status"], testConfig)).not.toBeNull();
    expect(isSafeCommand(["git", "log"], testConfig)).not.toBeNull();
    expect(isSafeCommand(["git", "commit"], testConfig)).toBeNull(); // Still denied
    expect(isSafeCommand(["git", "push"], testConfig)).toBeNull(); // Still denied
    expect(isSafeCommand(["git", "branch"], testConfig)).not.toBeNull(); // Now allowed
  });
  
  test("GitHub CLI commands respect requireApprovalByDefault setting", () => {
    // Create a test config
    const testConfig = {
      githubCli: {
        requireApprovalByDefault: false,
        autoApprovedCommands: ["issue list", "pr list"],
        requireApprovalCommands: ["pr create", "issue close"]
      }
    };
    
    // When requireApprovalByDefault is false, commands not in requireApprovalCommands should be auto-approved
    expect(isSafeCommand(["gh", "issue", "list"], testConfig)).not.toBeNull();
    expect(isSafeCommand(["gh", "pr", "list"], testConfig)).not.toBeNull();
    expect(isSafeCommand(["gh", "pr", "create"], testConfig)).toBeNull();
    expect(isSafeCommand(["gh", "issue", "close"], testConfig)).toBeNull();
    expect(isSafeCommand(["gh", "workflow", "list"], testConfig)).not.toBeNull(); // Not explicitly denied
    
    // Flip the default setting
    testConfig.githubCli.requireApprovalByDefault = true;
    
    // Now commands not explicitly allowed should require approval
    expect(isSafeCommand(["gh", "issue", "list"], testConfig)).not.toBeNull();
    expect(isSafeCommand(["gh", "pr", "list"], testConfig)).not.toBeNull();
    expect(isSafeCommand(["gh", "pr", "create"], testConfig)).toBeNull(); // Still denied
    expect(isSafeCommand(["gh", "issue", "close"], testConfig)).toBeNull(); // Still denied
    expect(isSafeCommand(["gh", "workflow", "list"], testConfig)).toBeNull(); // Now denied
  });
  
  test("canAutoApprove passes config to isSafeCommand", () => {
    // Create a test config
    const testConfig = {
      git: {
        requireApprovalByDefault: true,
        autoApprovedCommands: ["status"],
        requireApprovalCommands: ["commit"]
      }
    };
    
    // Test with config
    const allowedResult = canAutoApprove(["git", "status"], "suggest", [process.cwd()], process.env, testConfig);
    const deniedResult = canAutoApprove(["git", "commit"], "suggest", [process.cwd()], process.env, testConfig);
    
    expect(allowedResult.type).toBe("auto-approve");
    expect(deniedResult.type).toBe("ask-user");
  });
});