package com.openai.codex.agent

import android.content.Context
import android.util.Log
import com.openai.codex.bridge.SessionExecutionSettings
import java.io.IOException
import org.json.JSONArray
import org.json.JSONObject
import org.json.JSONTokener

data class AgentDelegationTarget(
    val packageName: String,
    val objective: String,
    val finalPresentationPolicy: SessionFinalPresentationPolicy,
)

data class AgentDelegationPlan(
    val originalObjective: String,
    val targets: List<AgentDelegationTarget>,
    val rationale: String?,
    val usedOverride: Boolean,
) {
    val primaryTargetPackage: String
        get() = targets.first().packageName
}

object AgentTaskPlanner {
    private const val TAG = "AgentTaskPlanner"
    private const val PLANNER_ATTEMPTS = 2
    private const val PLANNER_REQUEST_TIMEOUT_MS = 90_000L

    private val PLANNER_INSTRUCTIONS =
        """
        You are Codex acting as the Android Agent orchestrator.
        The user interacts only with the Agent. Decide which installed Android packages should receive delegated Genie sessions.
        Use the standard Android shell tools already available in this runtime, such as `cmd package`, `pm`, and `am`, to inspect installed packages and resolve the correct targets.
        Return exactly one JSON object and nothing else. Do not wrap it in markdown fences.
        JSON schema:
        {
          "targets": [
            {
              "packageName": "installed.package",
              "objective": "free-form delegated objective for the child Genie",
              "finalPresentationPolicy": "ATTACHED | DETACHED_HIDDEN | DETACHED_SHOWN | AGENT_CHOICE"
            }
          ],
          "reason": "short rationale",
          "allowDetachedMode": true
        }
        Rules:
        - Choose the fewest packages needed to complete the request.
        - `targets` must be non-empty.
        - Each delegated `objective` should be written for the child Genie, not the user.
        - Each target must include `finalPresentationPolicy`.
        - Prefer `DETACHED_HIDDEN` by default so the target app stays in the background while the Agent reports the outcome.
        - Use `ATTACHED` only when the user explicitly asks to leave the app open, bring it to the front, show them the resulting screen, or the request clearly implies that the final UI must be visible.
        - Use `DETACHED_SHOWN` only when the user asks for the app to remain visibly shown in detached mode rather than attached.
        - Use `AGENT_CHOICE` only when there is a strong reason to let the Genie choose the final presentation dynamically; otherwise pick `DETACHED_HIDDEN`.
        - Stop after at most 6 shell commands.
        - Start from the installed package list, then narrow to the most likely candidates.
        - Prefer direct package-manager commands over broad shell pipelines.
        - Verify each chosen package by inspecting focused query-activities or resolve-activity output before returning it.
        - Only choose packages that directly own the requested app behavior. Never choose helper packages such as `com.android.shell`, `com.android.systemui`, or the Codex Agent/Genie packages unless the user explicitly asked for them.
        - If the user objective already names a specific installed package, use it directly after verification.
        - If the user objective is too ambiguous to choose target packages or delegated objectives safely, call request_user_input before returning JSON. Do not guess a task for vague prompts like "do something".
        - `pm list packages PACKAGE_NAME` alone is not sufficient verification.
        - Prefer focused verification commands such as `pm list packages clock`, `cmd package query-activities --brief -p PACKAGE -a android.intent.action.MAIN`, and `cmd package resolve-activity --brief -a RELEVANT_ACTION PACKAGE`.
        - Do not enumerate every launcher activity on the device. Query specific candidate packages instead.
        """.trimIndent()
    private val PLANNER_OUTPUT_SCHEMA =
        JSONObject()
            .put("type", "object")
            .put(
                "properties",
                JSONObject()
                    .put(
                        "targets",
                        JSONObject()
                            .put("type", "array")
                            .put("minItems", 1)
                            .put(
                                "items",
                                JSONObject()
                                    .put("type", "object")
                                    .put(
                                        "properties",
                                        JSONObject()
                                            .put("packageName", JSONObject().put("type", "string"))
                                            .put("objective", JSONObject().put("type", "string"))
                                            .put(
                                                "finalPresentationPolicy",
                                                JSONObject()
                                                    .put("type", "string")
                                                    .put(
                                                        "enum",
                                                        JSONArray()
                                                            .put(SessionFinalPresentationPolicy.ATTACHED.wireValue)
                                                            .put(SessionFinalPresentationPolicy.DETACHED_HIDDEN.wireValue)
                                                            .put(SessionFinalPresentationPolicy.DETACHED_SHOWN.wireValue)
                                                            .put(SessionFinalPresentationPolicy.AGENT_CHOICE.wireValue),
                                                    ),
                                            ),
                                    )
                                    .put(
                                        "required",
                                        JSONArray()
                                            .put("packageName")
                                            .put("objective")
                                            .put("finalPresentationPolicy"),
                                    )
                                    .put("additionalProperties", false),
                            ),
                    )
                    .put("reason", JSONObject().put("type", "string"))
                    .put("allowDetachedMode", JSONObject().put("type", "boolean")),
            )
            .put("required", JSONArray().put("targets").put("reason").put("allowDetachedMode"))
            .put("additionalProperties", false)

    internal fun plannerInstructions(): String = PLANNER_INSTRUCTIONS

    fun startSession(
        context: Context,
        userObjective: String,
        targetPackageOverride: String?,
        allowDetachedMode: Boolean,
        finalPresentationPolicyOverride: SessionFinalPresentationPolicy? = null,
        executionSettings: SessionExecutionSettings = SessionExecutionSettings.default,
        sessionController: AgentSessionController,
        requestUserInputHandler: ((JSONArray) -> JSONObject)? = null,
    ): SessionStartResult {
        if (!targetPackageOverride.isNullOrBlank()) {
            Log.i(TAG, "Using explicit target override $targetPackageOverride")
            return sessionController.startDirectSession(
                plan = AgentDelegationPlan(
                    originalObjective = userObjective,
                    targets = listOf(
                        AgentDelegationTarget(
                            packageName = targetPackageOverride,
                            objective = userObjective,
                            finalPresentationPolicy =
                                finalPresentationPolicyOverride ?: SessionFinalPresentationPolicy.AGENT_CHOICE,
                        ),
                    ),
                    rationale = "Using explicit target package override.",
                    usedOverride = true,
                ),
                allowDetachedMode = allowDetachedMode,
            )
        }
        val pendingSession = sessionController.createPendingDirectSession(
            objective = userObjective,
            executionSettings = executionSettings,
        )
        val sessionStartResult = try {
            val effectiveRequestUserInputHandler = requestUserInputHandler ?: { questions: JSONArray ->
                AgentPlannerQuestionRegistry.requestUserInput(
                    context = context,
                    sessionController = sessionController,
                    sessionId = pendingSession.parentSessionId,
                    questions = questions,
                )
            }
            val request = planSession(
                context = context,
                userObjective = userObjective,
                executionSettings = executionSettings,
                sessionController = sessionController,
                requestUserInputHandler = effectiveRequestUserInputHandler,
                frameworkSessionId = pendingSession.parentSessionId,
            )
            sessionController.startDirectSessionChildren(
                parentSessionId = pendingSession.parentSessionId,
                geniePackage = pendingSession.geniePackage,
                plan = request.plan,
                allowDetachedMode = allowDetachedMode && request.allowDetachedMode,
                executionSettings = executionSettings,
                cancelParentOnFailure = true,
            )
        } catch (err: IOException) {
            runCatching { sessionController.cancelSession(pendingSession.parentSessionId) }
            throw err
        } catch (err: RuntimeException) {
            runCatching { sessionController.cancelSession(pendingSession.parentSessionId) }
            throw err
        }
        Log.i(TAG, "Planner sessionStartResult=$sessionStartResult")
        return sessionStartResult
    }

    fun planSession(
        context: Context,
        userObjective: String,
        executionSettings: SessionExecutionSettings = SessionExecutionSettings.default,
        sessionController: AgentSessionController,
        requestUserInputHandler: ((JSONArray) -> JSONObject)? = null,
        frameworkSessionId: String? = null,
    ): AgentFrameworkToolBridge.StartDirectSessionRequest {
        Log.i(TAG, "Planning Agent session for objective=${userObjective.take(160)}")
        val isEligibleTargetPackage = { packageName: String ->
            sessionController.canStartSessionForTarget(packageName) &&
                packageName !in setOf(
                    "com.android.shell",
                    "com.android.systemui",
                    "com.openai.codex.agent",
                    "com.openai.codex.genie",
                )
        }
        var previousPlannerResponse: String? = null
        var plannerRequest: AgentFrameworkToolBridge.StartDirectSessionRequest? = null
        var lastPlannerError: IOException? = null
        for (attemptIndex in 0 until PLANNER_ATTEMPTS) {
            val plannerResponse = AgentPlannerRuntimeManager.requestText(
                context = context,
                instructions = PLANNER_INSTRUCTIONS,
                prompt = buildPlannerPrompt(
                    userObjective = userObjective,
                    previousPlannerResponse = previousPlannerResponse,
                    previousPlannerError = lastPlannerError?.message,
                ),
                outputSchema = PLANNER_OUTPUT_SCHEMA,
                requestUserInputHandler = requestUserInputHandler,
                executionSettings = executionSettings,
                requestTimeoutMs = PLANNER_REQUEST_TIMEOUT_MS,
                frameworkSessionId = frameworkSessionId,
            )
            Log.i(TAG, "Planner response=${plannerResponse.take(400)}")
            previousPlannerResponse = plannerResponse
            val parsedRequest = runCatching {
                parsePlannerResponse(
                    responseText = plannerResponse,
                    userObjective = userObjective,
                    isEligibleTargetPackage = isEligibleTargetPackage,
                )
            }.getOrElse { err ->
                if (err is IOException && attemptIndex < PLANNER_ATTEMPTS - 1) {
                    Log.w(TAG, "Planner response rejected: ${err.message}")
                    lastPlannerError = err
                    continue
                }
                throw err
            }
            plannerRequest = parsedRequest
            break
        }
        return plannerRequest ?: throw (lastPlannerError
            ?: IOException("Planner did not return a valid session plan"))
    }

    private fun buildPlannerPrompt(
        userObjective: String,
        previousPlannerResponse: String?,
        previousPlannerError: String?,
    ): String {
        return buildString {
            appendLine("User objective:")
            appendLine(userObjective)
            if (!previousPlannerError.isNullOrBlank()) {
                appendLine()
                appendLine("Previous candidate plan was rejected by host validation:")
                appendLine(previousPlannerError)
                appendLine("Choose a different installed target package and verify it with focused package commands.")
            }
            if (!previousPlannerResponse.isNullOrBlank()) {
                appendLine()
                appendLine("Previous invalid planner response:")
                appendLine(previousPlannerResponse)
            }
        }.trim()
    }

    internal fun parsePlannerResponse(
        responseText: String,
        userObjective: String,
        isEligibleTargetPackage: (String) -> Boolean,
    ): AgentFrameworkToolBridge.StartDirectSessionRequest {
        val plannerJson = extractPlannerJson(responseText)
        return AgentFrameworkToolBridge.parseStartDirectSessionArguments(
            arguments = plannerJson,
            userObjective = userObjective,
            isEligibleTargetPackage = isEligibleTargetPackage,
        )
    }

    private fun extractPlannerJson(responseText: String): JSONObject {
        val trimmed = responseText.trim()
        parseJsonObject(trimmed)?.let { return it }
        val unfenced = trimmed
            .removePrefix("```json")
            .removePrefix("```")
            .removeSuffix("```")
            .trim()
        parseJsonObject(unfenced)?.let { return it }
        val firstBrace = trimmed.indexOf('{')
        val lastBrace = trimmed.lastIndexOf('}')
        if (firstBrace >= 0 && lastBrace > firstBrace) {
            parseJsonObject(trimmed.substring(firstBrace, lastBrace + 1))?.let { return it }
        }
        throw IOException("Planner did not return a valid JSON object")
    }

    private fun parseJsonObject(text: String): JSONObject? {
        return runCatching {
            val tokener = JSONTokener(text)
            val value = tokener.nextValue()
            value as? JSONObject
        }.getOrNull()
    }
}
