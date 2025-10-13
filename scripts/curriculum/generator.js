const { randomUUID } = require("node:crypto");

const difficultyProfiles = {
  exploratory: {
    label: "exploratory",
    analysisFocus:
      "Introduce the idea gently and prioritize concrete, observable phenomena before moving to symbolic notation.",
    collaborationFocus:
      "Provide heavily scaffolded collaboration with teacher check-ins after each mini-task.",
    productFocus:
      "Learners build short movement studies that demonstrate a single part of the concept.",
  },
  standard: {
    label: "standard",
    analysisFocus:
      "Blend conceptual, numerical, and embodied reasoning so students can move between representations of the idea.",
    collaborationFocus:
      "Balance teacher guidance with peer feedback loops that help students refine their thinking.",
    productFocus:
      "Learners design a polished movement study that illustrates how the concept operates over time.",
  },
  advanced: {
    label: "advanced",
    analysisFocus:
      "Push students to justify models, reference real data sets, and critique limitations of each representation.",
    collaborationFocus:
      "Expect self-directed collaboration with roles for analysis, choreography, and critique.",
    productFocus:
      "Learners create an extended performance piece that communicates the concept and its constraints to an audience.",
  },
};

const defaultDifficultyKey = "standard";

function normalizeDifficulty(difficulty) {
  if (!difficulty || typeof difficulty !== "string") {
    return difficultyProfiles[defaultDifficultyKey];
  }

  const key = difficulty.toLowerCase();
  return difficultyProfiles[key] ?? difficultyProfiles[defaultDifficultyKey];
}

function parseGradeBand(gradeLevel) {
  if (!gradeLevel || typeof gradeLevel !== "string") {
    return {
      label: "Unspecified grade band",
      start: null,
      end: null,
      spanDescription: "Multi-grade grouping",
    };
  }

  const numericMatches = gradeLevel.match(/\d+/g);

  if (!numericMatches) {
    return {
      label: gradeLevel.trim(),
      start: null,
      end: null,
      spanDescription: gradeLevel.trim(),
    };
  }

  const start = Number.parseInt(numericMatches[0], 10);
  const end = Number.parseInt(numericMatches[numericMatches.length - 1], 10);

  return {
    label: gradeLevel.trim(),
    start: Number.isFinite(start) ? start : null,
    end: Number.isFinite(end) ? end : Number.isFinite(start) ? start : null,
    spanDescription:
      Number.isFinite(start) && Number.isFinite(end) && start !== end
        ? `Grades ${start}-${end}`
        : Number.isFinite(start)
          ? `Grade ${start}`
          : gradeLevel.trim(),
  };
}

function sanitizeObjective(objective) {
  if (typeof objective !== "string") {
    return null;
  }

  const cleaned = objective.replace(/\s+/g, " ").trim();
  return cleaned.length > 0 ? cleaned : null;
}

function buildLessonSequence(context) {
  const { concept, lowerConcept, gradeBand, objectives, difficultyProfile } =
    context;

  const dayFocus = [
    {
      id: randomUUID(),
      day: 1,
      title: `Launch: Experiencing ${concept}`,
      focus:
        "Surface prior knowledge and anchor the big idea in movement-rich phenomena.",
      activities: [
        "Gallery walk with sports, robotics, and dance visuals that highlight motion.",
        `Quickwrite: "Where do you see ${lowerConcept} affecting everyday movement?"`,
        "Whole-class discussion to connect student examples with the formal definition of momentum.",
      ],
      learningModality: ["discussion", "writing", "movement"],
      formativeAssessment: [
        "Collect quickwrite responses to check how students describe the relationship between mass and velocity.",
        "Use exit tickets asking students to sketch a collision scenario and label the momentum transfers.",
      ],
    },
    {
      id: randomUUID(),
      day: 2,
      title: "Quantifying motion",
      focus: "Use data to calculate momentum and notice conservation patterns.",
      activities: [
        "Mini-lesson on calculating momentum (p = m × v) with unit analysis reminders.",
        `Stations where teams measure mass and velocity of rolling carts to compute ${lowerConcept}.`,
        "Share-out comparing how changing mass or velocity altered momentum totals.",
      ],
      learningModality: ["lab", "calculation", "discussion"],
      formativeAssessment: [
        "Check station data tables for accurate calculations and units.",
        "Listen for students using additive reasoning when discussing total system momentum.",
      ],
    },
    {
      id: randomUUID(),
      day: 3,
      title: "Embodied models through dance",
      focus: objectives.danceObjective
        ? objectives.danceObjective
        : "Translate mathematical understanding into choreographed movement.",
      activities: [
        "Warm-up exploring levels, directions, and tempo to represent changes in velocity.",
        "Choreography workshop where trios map momentum calculations onto short dance phrases.",
        "Peer feedback protocol: observers describe how mass and velocity were communicated in movement.",
      ],
      learningModality: ["movement", "creative expression", "peer feedback"],
      formativeAssessment: [
        "Use a checklist to note whether each choreography shows clear mass/velocity contrasts.",
        "Capture student explanations (audio or notes) that connect dance choices to conservation of momentum.",
      ],
    },
    {
      id: randomUUID(),
      day: 4,
      title: "Designing the showcase",
      focus: difficultyProfile.productFocus,
      activities: [
        "Storyboard session to plan the beginning, middle, and end of the group performance.",
        "Integrate music or percussion to emphasize collisions or transfers of energy.",
        "Teacher conferences with each group to coach precision in both science vocabulary and dance execution.",
      ],
      learningModality: ["planning", "movement", "coaching"],
      formativeAssessment: [
        "Collect storyboards with annotated science explanations.",
        "Use a mid-rehearsal checklist for clarity of the scientific narrative.",
      ],
    },
    {
      id: randomUUID(),
      day: 5,
      title: "Performance and reflection",
      focus:
        "Synthesize learning through performance, critique, and written analysis.",
      activities: [
        "Group performances with peers using a feedback protocol tied to the rubric.",
        "Audience reflection circle discussing where conservation of momentum was visible.",
        "Individual written explanation summarizing how their choreography models momentum.",
      ],
      learningModality: ["performance", "discussion", "writing"],
      formativeAssessment: [
        "Collect written explanations to evaluate conceptual transfer.",
        "Facilitate self-assessment aligned to the performance rubric.",
      ],
    },
  ];

  return dayFocus;
}

function createPerformanceTask(context) {
  const { concept, lowerConcept, difficultyProfile, language } = context;

  return {
    title: `${concept} Movement Showcase`,
    description: `Student ensembles choreograph a 60-90 second piece that demonstrates how ${lowerConcept} is conserved within a system. The work should connect numerical reasoning, scientific vocabulary, and artistic choices for the intended audience.`,
    deliverables: [
      "Annotated choreography map highlighting where mass, velocity, and total momentum are represented.",
      "Live or recorded performance.",
      "Artist statement that explains creative choices and the science behind them.",
    ],
    assessmentCriteria: [
      "Accuracy: Scientific explanations and vocabulary are precise and aligned to classroom learning.",
      "Modeling: Movement clearly illustrates momentum transfer or conservation across moments in the piece.",
      "Communication: Artist statement and presentation connect effectively with the audience.",
    ],
    languageSupports:
      language && language.toLowerCase().startsWith("en")
        ? [
            'Provide sentence frames for the artist statement (e.g., "Our choreography shows momentum because...").',
            "Offer a word wall with visuals for key vocabulary.",
          ]
        : [
            "Supply bilingual vocabulary cards and allow the artist statement to be written in students' preferred language.",
          ],
    difficultyNotes: difficultyProfile.collaborationFocus,
  };
}

function buildDifferentiation(context) {
  const { difficultyProfile } = context;

  return {
    supportStrategies: [
      "Offer partially completed data tables or choreography outlines for learners who benefit from extra structure.",
      "Provide manipulatives (e.g., weighted objects, scarves) to make mass and velocity changes tangible.",
      "Use small-group workshops to reteach foundational math skills when needed.",
    ],
    extensionIdeas: [
      "Invite students to analyze a professional dance clip or sports highlight and calculate approximate momentum values.",
      "Challenge advanced learners to incorporate two-stage collisions or partner lifts that require precise timing.",
      "Ask students to design a digital animation that pairs with their choreography to visualize vector quantities.",
    ],
    communityConnections: [
      "Collaborate with the dance department or local artists to co-facilitate feedback sessions.",
      "Invite a physics professional to share real-world applications of momentum conservation.",
    ],
    alignmentNotes: difficultyProfile.analysisFocus,
  };
}

function generateCurriculum(requestBody) {
  if (
    !requestBody ||
    typeof requestBody !== "object" ||
    Array.isArray(requestBody)
  ) {
    throw new Error("Request body must be an object.");
  }

  const concept =
    typeof requestBody.concept === "string" ? requestBody.concept.trim() : "";
  const gradeLevel =
    typeof requestBody.grade_level === "string"
      ? requestBody.grade_level.trim()
      : "";
  const learningObjectivesRaw = Array.isArray(requestBody.learning_objectives)
    ? requestBody.learning_objectives
    : [];
  const language =
    typeof requestBody.language === "string"
      ? requestBody.language.trim()
      : "en";
  const difficultyProfile = normalizeDifficulty(requestBody.difficulty);

  if (!concept) {
    throw new Error('"concept" is required.');
  }

  if (!gradeLevel) {
    throw new Error('"grade_level" is required.');
  }

  const sanitizedObjectives = learningObjectivesRaw
    .map(sanitizeObjective)
    .filter((objective) => objective !== null);

  if (sanitizedObjectives.length === 0) {
    throw new Error("At least one learning objective is required.");
  }

  const danceObjective =
    sanitizedObjectives.find((objective) => /dance/i.test(objective)) ??
    "Use dance and movement vocabulary to model how the concept operates in different scenarios.";

  const gradeBand = parseGradeBand(gradeLevel);
  const lowerConcept = concept.toLowerCase();

  const context = {
    concept,
    lowerConcept,
    gradeBand,
    objectives: {
      list: sanitizedObjectives,
      danceObjective,
    },
    difficultyProfile,
    language,
  };

  const lessonSequence = buildLessonSequence(context);
  const performanceTask = createPerformanceTask(context);
  const differentiation = buildDifferentiation(context);

  return {
    concept,
    gradeLevel,
    gradeBand,
    language,
    difficulty: difficultyProfile.label,
    duration: "5 class periods (~45 minutes each)",
    summary: `Students explore ${lowerConcept} through data analysis and creative movement, progressively connecting calculations to embodied models.`,
    learningObjectives: sanitizedObjectives,
    essentialQuestions: [
      `How does changing mass or velocity influence ${lowerConcept} in a system?`,
      `In what ways can movement or dance help us visualize ${lowerConcept}?`,
      `Where do we rely on ${lowerConcept} outside the classroom?`,
    ],
    keyVocabulary: [
      "momentum",
      "mass",
      "velocity",
      "conservation",
      "transfer",
      "system",
    ],
    lessonSequence,
    performanceTask,
    differentiation,
    teacherNotes: [
      "Capture rehearsal photos or short clips for reflection journals and evidence of learning.",
      "Coordinate with physical education or dance staff to ensure safe movement practices.",
      "Leverage the DTG project resources for documenting student artifacts and reflections.",
      "Reference the Sentient Cents data pipeline to discuss real-world logging and analysis workflows.",
    ],
  };
}

module.exports = {
  generateCurriculum,
  parseGradeBand,
  normalizeDifficulty,
};
