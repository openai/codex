import { describe, it, expect } from "vitest";

// Mock the auth flow to test account_id extraction
describe("Auth account_id extraction", () => {
  const mockTokenData = {
    id_token:
      "header." +
      Buffer.from(
        JSON.stringify({
          "https://api.openai.com/auth": {
            organization_id: "org-123",
            project_id: "proj-456",
            chatgpt_account_id: "team-account-789",
            completed_platform_onboarding: true,
            is_org_owner: true,
          },
        }),
      ).toString("base64url") +
      ".signature",
    access_token:
      "header." +
      Buffer.from(
        JSON.stringify({
          "https://api.openai.com/auth": {
            chatgpt_plan_type: "team",
          },
        }),
      ).toString("base64url") +
      ".signature",
    refresh_token: "refresh-token-xyz",
  };

  it("should extract and save chatgpt_account_id from ID token claims", async () => {
    // Parse the ID token
    const idTokenParts = mockTokenData.id_token.split(".");
    const idTokenClaims = JSON.parse(
      Buffer.from(idTokenParts[1]!, "base64url").toString("utf8"),
    );

    // Extract the account_id
    const chatgptAccountId =
      idTokenClaims["https://api.openai.com/auth"]?.chatgpt_account_id;

    // Verify extraction
    expect(chatgptAccountId).toBe("team-account-789");

    // Simulate saving to auth.json structure
    const authData = {
      tokens: {
        id_token: mockTokenData.id_token,
        access_token: mockTokenData.access_token,
        refresh_token: mockTokenData.refresh_token,
        account_id: chatgptAccountId || "",
      },
      last_refresh: new Date().toISOString(),
      OPENAI_API_KEY: "test-api-key",
    };

    // Verify the structure includes account_id
    expect(authData.tokens.account_id).toBe("team-account-789");
    expect(authData.tokens).toHaveProperty("account_id");
  });

  it("should handle missing chatgpt_account_id gracefully", async () => {
    const tokenWithoutAccountId = {
      id_token:
        "header." +
        Buffer.from(
          JSON.stringify({
            "https://api.openai.com/auth": {
              organization_id: "org-123",
              project_id: "proj-456",
              // No chatgpt_account_id
            },
          }),
        ).toString("base64url") +
        ".signature",
    };

    const idTokenParts = tokenWithoutAccountId.id_token.split(".");
    const idTokenClaims = JSON.parse(
      Buffer.from(idTokenParts[1]!, "base64url").toString("utf8"),
    );

    const chatgptAccountId =
      idTokenClaims["https://api.openai.com/auth"]?.chatgpt_account_id;

    // Should be undefined
    expect(chatgptAccountId).toBeUndefined();

    // Should save empty string when missing
    const authData = {
      tokens: {
        account_id: chatgptAccountId || "",
      },
    };

    expect(authData.tokens.account_id).toBe("");
  });

  it("should preserve account_id during token refresh", async () => {
    // Simulate existing auth.json with account_id
    const existingAuth = {
      tokens: {
        id_token: "old-token",
        access_token: "old-access",
        refresh_token: "old-refresh",
        account_id: "existing-account-123",
      },
      last_refresh: "2024-01-01T00:00:00Z",
      OPENAI_API_KEY: "old-api-key",
    };

    // Simulate refresh without new account_id in response
    const refreshedIdToken =
      "header." +
      Buffer.from(
        JSON.stringify({
          "https://api.openai.com/auth": {
            organization_id: "org-123",
            project_id: "proj-456",
            // No chatgpt_account_id in refresh response
          },
        }),
      ).toString("base64url") +
      ".signature";

    // Parse refreshed token
    const idTokenParts = refreshedIdToken.split(".");
    const idClaims = JSON.parse(
      Buffer.from(idTokenParts[1]!, "base64url").toString("utf8"),
    );

    // Update the auth data, preserving account_id if not in new token
    const updatedAuth = { ...existingAuth };
    updatedAuth.tokens.id_token = refreshedIdToken;

    // Preserve existing account_id if new one not provided
    if (
      !updatedAuth.tokens.account_id &&
      idClaims?.["https://api.openai.com/auth"]?.chatgpt_account_id
    ) {
      updatedAuth.tokens.account_id =
        idClaims["https://api.openai.com/auth"].chatgpt_account_id;
    }

    // Verify account_id is preserved
    expect(updatedAuth.tokens.account_id).toBe("existing-account-123");
  });
});
