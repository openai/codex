const test = require("node:test");
const assert = require("node:assert/strict");
const {
  generateCurriculum,
  parseGradeBand,
  normalizeDifficulty,
} = require("./generator");

test("parseGradeBand extracts numeric range", () => {
  const result = parseGradeBand("6-8");
  assert.equal(result.start, 6);
  assert.equal(result.end, 8);
  assert.equal(result.spanDescription, "Grades 6-8");
});

test("normalizeDifficulty falls back to standard when unknown", () => {
  const profile = normalizeDifficulty("unknown");
  assert.equal(profile.label, "standard");
});

test("generateCurriculum returns structured plan", () => {
  const curriculum = generateCurriculum({
    concept: "Momentum",
    grade_level: "6-8",
    learning_objectives: [
      "Define momentum as mass × velocity",
      "Use dance phrases to model conservation of momentum",
    ],
    language: "en",
    difficulty: "standard",
  });

  assert.equal(curriculum.concept, "Momentum");
  assert.equal(curriculum.gradeLevel, "6-8");
  assert.ok(Array.isArray(curriculum.lessonSequence));
  assert.equal(curriculum.lessonSequence.length, 5);
  assert.ok(
    curriculum.learningObjectives.includes(
      "Define momentum as mass × velocity",
    ),
  );
  const danceLesson = curriculum.lessonSequence.find(
    (lesson) => /dance/i.test(lesson.title) || /dance/i.test(lesson.focus),
  );
  assert.ok(
    danceLesson,
    "Expected a lesson that references dance integration.",
  );
});
