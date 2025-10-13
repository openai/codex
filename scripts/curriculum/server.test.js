const test = require("node:test");
const assert = require("node:assert/strict");
const { createServer } = require("./server");

function createTestServer() {
  const app = createServer();
  return new Promise((resolve) => {
    const server = app.listen(0, () => {
      const { port } = server.address();
      resolve({ server, port });
    });
  });
}

test("POST /generate-curriculum returns curriculum response", async (t) => {
  const { server, port } = await createTestServer();
  t.after(() => server.close());

  const response = await fetch(`http://127.0.0.1:${port}/generate-curriculum`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      concept: "Momentum",
      grade_level: "6-8",
      learning_objectives: [
        "Define momentum as mass × velocity",
        "Use dance phrases to model conservation of momentum",
      ],
      language: "en",
      difficulty: "standard",
    }),
  });

  assert.equal(response.status, 200);
  const payload = await response.json();
  assert.equal(payload.concept, "Momentum");
  assert.equal(payload.gradeLevel, "6-8");
  assert.equal(payload.difficulty, "standard");
  assert.ok(Array.isArray(payload.lessonSequence));
});

test("POST /generate-curriculum validates payload", async (t) => {
  const { server, port } = await createTestServer();
  t.after(() => server.close());

  const response = await fetch(`http://127.0.0.1:${port}/generate-curriculum`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({}),
  });

  assert.equal(response.status, 400);
  const payload = await response.json();
  assert.ok(Array.isArray(payload.errors));
  assert.ok(payload.errors.length > 0);
});
