import { describe, test, expect, vi, beforeEach } from "vitest";
import { GoogleAuthProvider } from "../../../src/providers/auth/google-auth.js";

// Mock state for google-auth-library
const googleAuthState: {
  getClientSpy?: ReturnType<typeof vi.fn>;
  getProjectIdSpy?: ReturnType<typeof vi.fn>;
  getAccessTokenSpy?: ReturnType<typeof vi.fn>;
} = {};

vi.mock("google-auth-library", () => {
  class FakeGoogleAuth {
    async getClient() {
      if (googleAuthState.getClientSpy) {
        return googleAuthState.getClientSpy();
      }
      return {
        getAccessToken:
          googleAuthState.getAccessTokenSpy ||
          (() => ({ token: "fake-token" })),
      };
    }

    async getProjectId() {
      return googleAuthState.getProjectIdSpy?.() ?? null;
    }
  }

  return {
    GoogleAuth: FakeGoogleAuth,
  };
});

describe("GoogleAuthProvider", () => {
  beforeEach(() => {
    // Reset mock state
    googleAuthState.getClientSpy = undefined;
    googleAuthState.getProjectIdSpy = undefined;
    googleAuthState.getAccessTokenSpy = undefined;
    vi.clearAllMocks();
  });

  test("returns bearer token with access token", async () => {
    googleAuthState.getAccessTokenSpy = vi.fn().mockResolvedValue({
      token: "test-access-token",
    });

    const provider = new GoogleAuthProvider();
    const authHeader = await provider.getAuthHeader();

    expect(authHeader).toBe("Bearer test-access-token");
  });

  test("caches access token for subsequent calls", async () => {
    const getAccessTokenSpy = vi.fn().mockResolvedValue({
      token: "cached-token",
    });
    googleAuthState.getAccessTokenSpy = getAccessTokenSpy;

    const provider = new GoogleAuthProvider();

    // First call
    const authHeader1 = await provider.getAuthHeader();
    expect(authHeader1).toBe("Bearer cached-token");
    expect(getAccessTokenSpy).toHaveBeenCalledTimes(1);

    // Second call should use cache
    const authHeader2 = await provider.getAuthHeader();
    expect(authHeader2).toBe("Bearer cached-token");
    expect(getAccessTokenSpy).toHaveBeenCalledTimes(1); // Still only called once
  });

  test("refreshes token when cache expires", async () => {
    const getAccessTokenSpy = vi
      .fn()
      .mockResolvedValueOnce({ token: "token-1" })
      .mockResolvedValueOnce({ token: "token-2" });
    googleAuthState.getAccessTokenSpy = getAccessTokenSpy;

    const provider = new GoogleAuthProvider();

    // First call
    const authHeader1 = await provider.getAuthHeader();
    expect(authHeader1).toBe("Bearer token-1");

    // Manually expire the cache by setting expiry to past
    (provider as any).cachedToken = {
      token: "token-1",
      expiry: Date.now() - 1000, // 1 second ago
    };

    // Second call should fetch new token
    const authHeader2 = await provider.getAuthHeader();
    expect(authHeader2).toBe("Bearer token-2");
    expect(getAccessTokenSpy).toHaveBeenCalledTimes(2);
  });

  test("throws error when getAccessToken returns no token", async () => {
    googleAuthState.getAccessTokenSpy = vi.fn().mockResolvedValue({
      token: null,
    });

    const provider = new GoogleAuthProvider();

    await expect(provider.getAuthHeader()).rejects.toThrow(
      "Failed to get access token from Google Auth",
    );
  });

  test("validation succeeds when auth client can be created", async () => {
    googleAuthState.getClientSpy = vi.fn().mockResolvedValue({
      getAccessToken: () => ({ token: "test-token" }),
    });

    const provider = new GoogleAuthProvider();

    // Should not throw
    await expect(provider.validate()).resolves.toBeUndefined();
  });

  test("validation throws error when auth client creation fails", async () => {
    googleAuthState.getClientSpy = vi
      .fn()
      .mockRejectedValue(new Error("No credentials found"));

    const provider = new GoogleAuthProvider();

    await expect(provider.validate()).rejects.toThrow(
      "Failed to authenticate with Google Cloud",
    );
  });

  test("getProjectId returns project ID from auth client", async () => {
    googleAuthState.getProjectIdSpy = vi
      .fn()
      .mockResolvedValue("test-project-123");

    const provider = new GoogleAuthProvider();
    const projectId = await provider.getProjectId();

    expect(projectId).toBe("test-project-123");
  });

  test("getProjectId returns null when no project ID available", async () => {
    googleAuthState.getProjectIdSpy = vi.fn().mockResolvedValue(null);

    const provider = new GoogleAuthProvider();
    const projectId = await provider.getProjectId();

    expect(projectId).toBeNull();
  });

  test("getProjectId returns null when auth client throws", async () => {
    googleAuthState.getProjectIdSpy = vi
      .fn()
      .mockRejectedValue(new Error("No project ID"));

    const provider = new GoogleAuthProvider();
    const projectId = await provider.getProjectId();

    expect(projectId).toBeNull();
  });
});
