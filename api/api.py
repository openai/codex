from __future__ import annotations

import logging
import os
from typing import List, Literal, Optional

from fastapi import FastAPI, HTTPException
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import JSONResponse
from pydantic import BaseModel, Field, ValidationError

from embodied_learning.components.content_generator import STEMContentGenerator
from embodied_learning.components.movement_mapper import MovementToConceptMapper
from embodied_learning.components.nlp_processor import NeurolinguisticProcessor
from embodied_learning.curriculum import Curriculum
from embodied_learning.generator import EmbodiedLearningGenerator


logger = logging.getLogger("embodied_api")
handler = logging.StreamHandler()
handler.setFormatter(logging.Formatter("%(asctime)s [%(levelname)s] %(message)s"))
logger.setLevel(logging.INFO)
logger.addHandler(handler)

app = FastAPI(
    title="Embodied Learning Curriculum API",
    version="1.0.0",
    description="Generate embodied-learning curricula from movement + concept inputs.",
)

app.add_middleware(
    CORSMiddleware,
    allow_origins=os.environ.get("CORS_ALLOW_ORIGINS", "*").split(","),
    allow_credentials=True,
    allow_methods=["POST", "GET", "OPTIONS"],
    allow_headers=["*"],
)

GradeBand = Literal["K-2", "3-5", "6-8", "9-12", "Undergrad", "Adult"]


class CurriculumRequest(BaseModel):
    concept: str = Field(
        ..., min_length=2, description="Target concept, e.g. 'Momentum', 'Photosynthesis'."
    )
    grade_level: GradeBand | str = Field(..., description="Grade band or descriptor.")
    learning_objectives: List[str] = Field(
        ...,
        min_items=1,
        description="Concrete objectives, e.g. ['Define momentum', 'Relate movement to mass*velocity']",
    )
    language: Optional[str] = Field(default="en", description="ISO language code for generated content.")
    difficulty: Optional[Literal["intro", "standard", "advanced"]] = "standard"


class ErrorPayload(BaseModel):
    detail: str
    code: Optional[str] = None

    def to_dict(self) -> dict:
        model_dump = getattr(self, "model_dump", None)
        if callable(model_dump):
            return model_dump()
        return self.dict()


LLM_API_KEY = os.environ.get("LLM_API_KEY")

try:
    movement_mapper = MovementToConceptMapper()
    content_synthesizer = STEMContentGenerator(api_key=LLM_API_KEY)
    neurolinguistic_processor = NeurolinguisticProcessor()

    curriculum_generator = EmbodiedLearningGenerator(
        movement_analyzer=movement_mapper,
        content_synthesizer=content_synthesizer,
        neurolinguistic_processor=neurolinguistic_processor,
    )
    logger.info("EmbodiedLearning components initialized.")
except Exception:
    logger.exception("Failed to initialize components at import time.")
    curriculum_generator = None  # type: ignore


@app.get("/health")
def health():
    ok = curriculum_generator is not None
    return {"status": "ok" if ok else "degraded"}


@app.get("/version")
def version():
    return {"version": app.version}


@app.exception_handler(ValidationError)
async def pydantic_validation_handler(_, __: ValidationError):
    return JSONResponse(
        status_code=422,
        content=ErrorPayload(detail="Invalid request payload.", code="VALIDATION_ERROR").to_dict(),
    )


@app.exception_handler(Exception)
async def unhandled_handler(_, exc: Exception):
    logger.exception("Unhandled server error: %s", exc)
    return JSONResponse(
        status_code=500,
        content=ErrorPayload(detail="Internal server error.", code="UNHANDLED_ERROR").to_dict(),
    )


@app.post(
    "/generate-curriculum",
    response_model=Curriculum,
    responses={400: {"model": ErrorPayload}, 500: {"model": ErrorPayload}},
)
def generate_embodied_learning_curriculum(request: CurriculumRequest):
    if curriculum_generator is None:
        raise HTTPException(status_code=503, detail="Service not initialized.")

    grade_level = str(request.grade_level)

    try:
        final_curriculum: Curriculum = curriculum_generator.generate_curriculum(
            concept=request.concept.strip(),
            grade_level=grade_level.strip(),
            learning_objectives=[obj.strip() for obj in request.learning_objectives if obj.strip()],
            language=request.language,
            difficulty=request.difficulty,
        )
        return final_curriculum
    except ValueError as ve:
        logger.warning("Domain error: %s", ve)
        raise HTTPException(status_code=400, detail=str(ve))
    except Exception:
        logger.exception("Generation failed.")
        raise HTTPException(status_code=500, detail="Failed to generate curriculum.")


# Optional: run with `uvicorn api:app --reload`
