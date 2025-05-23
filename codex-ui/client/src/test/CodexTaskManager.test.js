const assert = require('assert');

// --- Mock fetch ---
let mockFetchResponse = {}; // This will be the 'body' of the response
let mockFetchStatus = 200;
// let mockFetchOk = true; // Derived from mockFetchStatus
let mockFetchError = null;
let lastFetchUrl = null;
let lastFetchOptions = null;
let fetchCallCount = 0;

global.fetch = async (url, options) => {
  fetchCallCount++;
  lastFetchUrl = url;
  lastFetchOptions = options;
  console.log(`Mock fetch called for URL: ${url}, Method: ${options ? options.method : 'GET'}`); // Log the call

  if (mockFetchError) {
    console.log('Mock fetch throwing error:', mockFetchError);
    throw mockFetchError;
  }

  const response = {
    ok: mockFetchStatus >= 200 && mockFetchStatus < 300,
    status: mockFetchStatus,
    json: async () => mockFetchResponse, // The actual body of the response
    text: async () => JSON.stringify(mockFetchResponse)
  };
  console.log('Mock fetch returning response:', response);
  return response;
};

// --- Simulate App.js#handleCodexExecute ---
// This is the function passed as onExecute to CodexTaskManager
async function handleCodexExecute(prompt, options) {
  console.log(`handleCodexExecute called with prompt: "${prompt}", options:`, options);
  try {
    const response = await fetch('/api/codex/execute', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ prompt, options }),
    });

    const responseData = await response.json(); // Always try to parse JSON

    if (!response.ok) {
      // If response.ok is false, throw an error with the parsed data if available,
      // or a generic error.
      // The backend sends { status: "error", message: "...", ... } for CLI errors,
      // which gets wrapped here.
      throw new Error(responseData.message || responseData.error || `HTTP error! status: ${response.status}`);
    }
    // If response.ok is true, the backend might still send { status: "error", ... }
    // or { status: "success", ... }. The caller (CodexTaskManager) will handle this.
    return responseData;
  } catch (error) {
    console.error('handleCodexExecute caught error:', error.message);
    throw error; // Re-throw for the caller (CodexTaskManager) to handle
  }
}

// --- Test Runner ---
const tests = [];
let testsPassed = 0;
let testsFailed = 0;

function test(description, fn) {
  tests.push({ description, fn });
}

async function runAllTests() {
  console.log('\nStarting CodexTaskManager related tests...');

  for (const t of tests) {
    // Reset mocks for each test
    mockFetchResponse = {};
    mockFetchStatus = 200;
    mockFetchError = null;
    lastFetchUrl = null;
    lastFetchOptions = null;
    fetchCallCount = 0;
    
    console.log(`\nRUNNING: ${t.description}`);
    try {
      await t.fn();
      console.log(`PASSED: ${t.description}`);
      testsPassed++;
    } catch (error) {
      console.error(`FAILED: ${t.description}`);
      console.error(error.stack || error);
      testsFailed++;
    }
  }

  console.log('\n--- Test Summary ---');
  console.log(`Total Tests: ${tests.length}`);
  console.log(`Passed: ${testsPassed}`);
  console.log(`Failed: ${testsFailed}`);
  console.log('--------------------');

  if (testsFailed > 0) {
    console.error("\nSOME FRONTEND TESTS FAILED.");
  } else {
    console.log("\nALL FRONTEND TESTS PASSED!");
  }
}


// --- Test Definitions ---

test('handleCodexExecute - Successful API call from codex', async () => {
  mockFetchStatus = 200;
  mockFetchResponse = { status: "success", stdout: "Test stdout", stderr: "Test stderr" };

  const result = await handleCodexExecute("test prompt", { model: "o4-mini", mode: "interactive" });
  
  assert.strictEqual(fetchCallCount, 1, 'Fetch should be called once');
  assert.strictEqual(lastFetchUrl, '/api/codex/execute', 'Fetch URL should be correct');
  assert.deepStrictEqual(JSON.parse(lastFetchOptions.body), { prompt: "test prompt", options: { model: "o4-mini", mode: "interactive" } }, 'Fetch body should be correct');
  assert.deepStrictEqual(result, mockFetchResponse, 'Result should match mock success response');
});

test('handleCodexExecute - API call returns an error from codex execution', async () => {
  mockFetchStatus = 200; // HTTP call is ok
  mockFetchResponse = { status: "error", message: "Codex CLI error detail", stdout: "partial stdout", stderr: "actual stderr from CLI" };

  const result = await handleCodexExecute("error prompt", { model: "o4", mode: "auto-edit" });

  assert.strictEqual(fetchCallCount, 1, 'Fetch should be called once');
  assert.deepStrictEqual(result, mockFetchResponse, 'Result should match mock codex error response');
});


test('handleCodexExecute - fetch itself fails (e.g. HTTP 500 from server)', async () => {
  mockFetchStatus = 500; // Simulate server error
  mockFetchResponse = { error: "Internal Server Error", message: "Server is down" }; // Backend might send this
  mockFetchError = null;

  try {
    await handleCodexExecute("server fail prompt", {});
    assert.fail("Should have thrown an HTTP error");
  } catch (error) {
    assert(error.message.includes(mockFetchResponse.message) || error.message.includes("HTTP error! status: 500"), `Error message should indicate HTTP failure. Got: "${error.message}"`);
  }
  assert.strictEqual(fetchCallCount, 1, 'Fetch should be called once');
});

test('handleCodexExecute - fetch itself fails (network error)', async () => {
  mockFetchError = new Error("Network failure");
  mockFetchResponse = {}; // Not used
  mockFetchStatus = 0; // Not used

  try {
    await handleCodexExecute("network fail prompt", {});
    assert.fail("Should have thrown a network error");
  } catch (error) {
    assert.strictEqual(error.message, "Network failure", "Error message should be 'Network failure'");
  }
  assert.strictEqual(fetchCallCount, 1, 'Fetch should be called once');
});


function describeCodexTaskManagerBehavior() {
  console.log("\n--- Describing CodexTaskManager state updates (conceptual) ---");

  console.log("\nScenario 1: Successful execution from onExecute");
  const mockSuccessData = { status: "success", stdout: "CMD Output", stderr: "CMD Warning" };
  console.log("  Given onExecute resolves with:", JSON.stringify(mockSuccessData));
  console.log("  CodexTaskManager should ideally:");
  console.log("    - Set isLoading = false");
  console.log("    - Set stdoutContent = 'CMD Output'");
  console.log("    - Set stderrContent = 'CMD Warning'");
  console.log("    - Clear its general 'error' state");
  console.log("    - Clear userActionMessage");


  console.log("\nScenario 2: Codex execution error reported by backend (via onExecute)");
  const mockCodexErrorData = { status: "error", message: "CLI Error Occurred", stdout: "Output before error", stderr: "The actual CLI error" };
  console.log("  Given onExecute resolves with:", JSON.stringify(mockCodexErrorData));
  console.log("  CodexTaskManager should ideally:");
  console.log("    - Set isLoading = false");
  console.log("    - Set its general 'error' state = 'CLI Error Occurred'");
  console.log("    - Set stdoutContent = 'Output before error'");
  console.log("    - Set stderrContent = 'The actual CLI error'");
  console.log("    - Clear userActionMessage");

  console.log("\nScenario 3: Network error or direct fetch failure (onExecute promise rejects)");
  const mockNetworkError = new Error("Network request failed");
  console.log("  Given onExecute promise rejects with error:", mockNetworkError.message);
  console.log("  CodexTaskManager should ideally:");
  console.log("    - Set isLoading = false");
  console.log("    - Set its general 'error' state = 'Network request failed'");
  console.log("    - Clear stdoutContent");
  console.log("    - Clear stderrContent");
  console.log("    - Clear userActionMessage");
  
  console.log("--- End conceptual description ---");
}

// Run all defined tests and descriptions
async function main() {
    await runAllTests();
    describeCodexTaskManagerBehavior();
}

main().catch(e => console.error("Critical error in test execution:", e));

// End of codex-ui/client/src/test/CodexTaskManager.test.js
