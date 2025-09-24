# Implementation Plan Template

## Metadata
- Input: [FEATURE_SPEC_PATH]
- Output: Implementation artifacts in SPECS_DIR
- Status: INITIALIZING

## Technical Context
[USER_PROVIDED_CONTEXT]

## Progress Tracking
- [ ] Phase 0: Research and Analysis
- [ ] Phase 1: Design and Architecture
- [ ] Phase 2: Implementation Planning

## Execution Flow

### main() {
#### Step 1: Initialize
- Set FEATURE_SPEC from input
- Create output directories
- Load feature specification

#### Step 2: Validate Prerequisites
- Check feature spec exists and is readable
- Verify output directory is writable
- Ensure no blocking dependencies

#### Step 3: Phase 0 - Research
- Analyze existing codebase
- Identify integration points
- Document findings in research.md
- Gate: Research complete?

#### Step 4: Phase 1 - Design
- Create data model specifications
- Define API contracts
- Generate quickstart guide
- Gate: Design approved?

#### Step 5: Phase 2 - Implementation Planning
- Break down into executable tasks
- Estimate effort and complexity
- Define task dependencies
- Generate tasks.md
- Gate: Tasks complete?

#### Step 6: Generate Artifacts
- Create all output files
- Validate artifact completeness
- Update progress tracking

#### Step 7: Error Handling
- Log any errors encountered
- Rollback incomplete operations
- Report error state

#### Step 8: Finalize
- Update status to COMPLETE
- Generate summary report
- Clean up temporary files

#### Step 9: Return
- Report success/failure
- List generated artifacts
- Provide next steps
### }

## Error Handling
- GATE_FAILED: Prerequisite not met
- GENERATION_ERROR: Artifact creation failed
- VALIDATION_ERROR: Invalid input or output

## Generated Artifacts
- Phase 0: research.md
- Phase 1: data-model.md, contracts/, quickstart.md
- Phase 2: tasks.md