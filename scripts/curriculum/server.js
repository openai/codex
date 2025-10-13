const express = require("express");
const { generateCurriculum } = require("./generator");

function validateRequest(body) {
  const errors = [];

  if (!body || typeof body !== "object" || Array.isArray(body)) {
    errors.push("Request body must be a JSON object.");
    return { errors };
  }

  if (typeof body.concept !== "string" || body.concept.trim().length === 0) {
    errors.push('"concept" is required and must be a non-empty string.');
  }

  if (
    typeof body.grade_level !== "string" ||
    body.grade_level.trim().length === 0
  ) {
    errors.push('"grade_level" is required and must be a non-empty string.');
  }

  if (
    !Array.isArray(body.learning_objectives) ||
    body.learning_objectives.length === 0
  ) {
    errors.push(
      '"learning_objectives" is required and must be a non-empty array of strings.',
    );
  }

  if (Array.isArray(body.learning_objectives)) {
    const invalidObjectives = body.learning_objectives.filter(
      (item) => typeof item !== "string" || item.trim().length === 0,
    );
    if (invalidObjectives.length > 0) {
      errors.push("Each learning objective must be a non-empty string.");
    }
  }

  if (
    body.language !== undefined &&
    (typeof body.language !== "string" || body.language.trim().length === 0)
  ) {
    errors.push('"language" must be a non-empty string when provided.');
  }

  if (body.difficulty !== undefined && typeof body.difficulty !== "string") {
    errors.push('"difficulty" must be a string when provided.');
  }

  return {
    errors,
    value:
      errors.length === 0
        ? {
            concept: body.concept.trim(),
            grade_level: body.grade_level.trim(),
            learning_objectives: body.learning_objectives.map((objective) =>
              objective.trim(),
            ),
            language: body.language,
            difficulty: body.difficulty,
          }
        : null,
  };
}

function createServer() {
  const app = express();

  app.use(express.json());

  app.get("/healthz", (req, res) => {
    res.json({ status: "ok" });
  });

  app.post("/generate-curriculum", (req, res) => {
    const validation = validateRequest(req.body);

    if (validation.errors.length > 0) {
      res.status(400).json({ errors: validation.errors });
      return;
    }

    try {
      const curriculum = generateCurriculum(validation.value);
      res.json(curriculum);
    } catch (error) {
      res.status(400).json({ errors: [error.message] });
    }
  });

  app.use((error, req, res, next) => {
    if (error instanceof SyntaxError && "body" in error) {
      res.status(400).json({ errors: ["Invalid JSON payload."] });
      return;
    }

    next(error);
  });

  return app;
}

function startServer() {
  const port = Number.parseInt(process.env.PORT ?? "8000", 10);
  const app = createServer();

  app.listen(port, () => {
    console.log(`Curriculum generator running on port ${port}`);
  });
}

if (require.main === module) {
  startServer();
}

module.exports = {
  createServer,
  startServer,
  validateRequest,
};
