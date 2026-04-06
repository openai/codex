package com.openai.codex.agent

import android.app.Activity
import android.app.agent.AgentManager
import android.app.agent.AgentSessionInfo
import android.content.Context
import android.content.Intent
import android.graphics.drawable.Drawable
import android.os.Bundle
import android.os.Binder
import android.util.Log
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.ArrayAdapter
import android.widget.Button
import android.widget.EditText
import android.widget.ImageView
import android.widget.Spinner
import android.widget.TextView
import android.widget.Toast
import com.openai.codex.bridge.SessionExecutionSettings
import kotlin.concurrent.thread

class CreateSessionActivity : Activity() {
    companion object {
        private const val TAG = "CodexCreateSession"
        const val ACTION_CREATE_SESSION = "com.openai.codex.agent.action.CREATE_SESSION"
        const val EXTRA_INITIAL_PROMPT = "com.openai.codex.agent.extra.INITIAL_PROMPT"
        private const val EXTRA_EXISTING_SESSION_ID = "existingSessionId"
        private const val EXTRA_TARGET_PACKAGE = "targetPackage"
        private const val EXTRA_LOCK_TARGET = "lockTarget"
        private const val EXTRA_INITIAL_MODEL = "initialModel"
        private const val EXTRA_INITIAL_REASONING_EFFORT = "initialReasoningEffort"
        private const val DEFAULT_MODEL = "gpt-5.3-codex-spark"
        private const val DEFAULT_REASONING_EFFORT = "low"

        fun preferredInitialSettings(): SessionExecutionSettings {
            return SessionExecutionSettings(
                model = DEFAULT_MODEL,
                reasoningEffort = DEFAULT_REASONING_EFFORT,
            )
        }

        private fun mergedWithPreferredDefaults(settings: SessionExecutionSettings): SessionExecutionSettings {
            val defaults = preferredInitialSettings()
            return SessionExecutionSettings(
                model = settings.model ?: defaults.model,
                reasoningEffort = settings.reasoningEffort ?: defaults.reasoningEffort,
            )
        }

        fun externalCreateSessionIntent(initialPrompt: String): Intent {
            return Intent(ACTION_CREATE_SESSION).apply {
                addCategory(Intent.CATEGORY_DEFAULT)
                putExtra(EXTRA_INITIAL_PROMPT, initialPrompt)
            }
        }

        fun newSessionIntent(
            context: Context,
            initialSettings: SessionExecutionSettings,
        ): Intent {
            return Intent(context, CreateSessionActivity::class.java).apply {
                putExtra(EXTRA_INITIAL_MODEL, initialSettings.model)
                putExtra(EXTRA_INITIAL_REASONING_EFFORT, initialSettings.reasoningEffort)
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            }
        }

        fun existingHomeSessionIntent(
            context: Context,
            sessionId: String,
            targetPackage: String,
            initialSettings: SessionExecutionSettings,
        ): Intent {
            return newSessionIntent(context, initialSettings).apply {
                putExtra(EXTRA_EXISTING_SESSION_ID, sessionId)
                putExtra(EXTRA_TARGET_PACKAGE, targetPackage)
                putExtra(EXTRA_LOCK_TARGET, true)
            }
        }
    }

    private val sessionController by lazy { AgentSessionController(this) }
    private val sessionUiLeaseToken = Binder()
    private var availableModels: List<AgentModelOption> = emptyList()
    @Volatile
    private var modelsRefreshInFlight = false
    private val pendingModelCallbacks = mutableListOf<() -> Unit>()

    private var existingSessionId: String? = null
    private var leasedSessionId: String? = null
    private var uiActive = false
    private var selectedPackage: InstalledApp? = null
    private var targetLocked = false
    private var agentAnchoredOnly = false

    private lateinit var promptInput: EditText
    private lateinit var packageSummary: TextView
    private lateinit var packageButton: Button
    private lateinit var clearPackageButton: Button
    private lateinit var modelSpinner: Spinner
    private lateinit var effortSpinner: Spinner
    private lateinit var titleView: TextView
    private lateinit var statusView: TextView
    private lateinit var startButton: Button

    private var selectedReasoningOptions = emptyList<AgentReasoningEffortOption>()
    private var pendingEffortOverride: String? = null
    private lateinit var effortLabelAdapter: ArrayAdapter<String>
    private var initialSettings = preferredInitialSettings()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_create_session)
        setFinishOnTouchOutside(true)
        bindViews()
        loadInitialState()
        refreshModelsIfNeeded(force = true)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        loadInitialState()
        if (availableModels.isNotEmpty()) {
            applyModelOptions()
        }
    }

    override fun onResume() {
        super.onResume()
        uiActive = true
        updateSessionUiLease(existingSessionId)
    }

    override fun onPause() {
        uiActive = false
        updateSessionUiLease(null)
        super.onPause()
    }

    private fun bindViews() {
        titleView = findViewById(R.id.create_session_title)
        statusView = findViewById(R.id.create_session_status)
        promptInput = findViewById(R.id.create_session_prompt)
        packageSummary = findViewById(R.id.create_session_target_summary)
        packageButton = findViewById(R.id.create_session_pick_target_button)
        clearPackageButton = findViewById(R.id.create_session_clear_target_button)
        modelSpinner = findViewById(R.id.create_session_model_spinner)
        effortSpinner = findViewById(R.id.create_session_effort_spinner)
        startButton = findViewById(R.id.create_session_start_button)

        effortLabelAdapter = ArrayAdapter(
            this,
            android.R.layout.simple_spinner_item,
            mutableListOf<String>(),
        ).also {
            it.setDropDownViewResource(android.R.layout.simple_spinner_dropdown_item)
            effortSpinner.adapter = it
        }
        modelSpinner.adapter = ArrayAdapter(
            this,
            android.R.layout.simple_spinner_item,
            mutableListOf<String>(),
        ).also { it.setDropDownViewResource(android.R.layout.simple_spinner_dropdown_item) }
        modelSpinner.onItemSelectedListener = SimpleItemSelectedListener {
            updateEffortOptions(pendingEffortOverride)
            pendingEffortOverride = null
        }

        packageButton.setOnClickListener {
            showInstalledAppPicker { app ->
                selectedPackage = app
                updatePackageSummary()
            }
        }
        clearPackageButton.setOnClickListener {
            selectedPackage = null
            updatePackageSummary()
        }
        findViewById<Button>(R.id.create_session_cancel_button).setOnClickListener {
            cancelAndFinish()
        }
        startButton.setOnClickListener {
            startSession()
        }
        updatePackageSummary()
    }

    private fun loadInitialState() {
        updateSessionUiLease(null)
        existingSessionId = null
        selectedPackage = null
        targetLocked = false
        agentAnchoredOnly = isAgentSessionLauncherIntent(intent)
        titleView.text = "New Session"
        statusView.visibility = View.GONE
        statusView.text = "Loading session…"
        startButton.isEnabled = true
        unlockTargetSelection()
        updatePackageSummary()
        if (agentAnchoredOnly) {
            titleView.text = "New Codex Session"
            lockTargetSelection()
        }

        existingSessionId = intent.getStringExtra(EXTRA_EXISTING_SESSION_ID)?.trim()?.ifEmpty { null }
        initialSettings = mergedWithPreferredDefaults(
            SessionExecutionSettings(
            model = intent.getStringExtra(EXTRA_INITIAL_MODEL)?.trim()?.ifEmpty { null } ?: DEFAULT_MODEL,
            reasoningEffort = intent.getStringExtra(EXTRA_INITIAL_REASONING_EFFORT)?.trim()?.ifEmpty { null }
                ?: DEFAULT_REASONING_EFFORT,
            ),
        )
        promptInput.setText(intent.getStringExtra(EXTRA_INITIAL_PROMPT).orEmpty())
        promptInput.setSelection(promptInput.text.length)
        val explicitTarget = intent.getStringExtra(EXTRA_TARGET_PACKAGE)?.trim()?.ifEmpty { null }
        targetLocked = intent.getBooleanExtra(EXTRA_LOCK_TARGET, false)
        if (explicitTarget != null) {
            selectedPackage = InstalledAppCatalog.resolveInstalledApp(this, sessionController, explicitTarget)
            titleView.text = "New Session"
            updatePackageSummary()
            if (targetLocked) {
                lockTargetSelection()
            }
            if (uiActive) {
                updateSessionUiLease(existingSessionId)
            }
            return
        }
        val incomingSessionId = intent.getStringExtra(AgentManager.EXTRA_SESSION_ID)?.trim()?.ifEmpty { null }
        if (incomingSessionId != null) {
            statusView.visibility = View.VISIBLE
            statusView.text = "Loading session…"
            startButton.isEnabled = false
            thread {
                val draftSession = runCatching {
                    findStandaloneHomeDraftSession(incomingSessionId)
                }.getOrElse { err ->
                    Log.w(TAG, "Failed to inspect incoming session $incomingSessionId", err)
                    null
                }
                runOnUiThread {
                    if (draftSession == null) {
                        startActivity(
                            SessionPopupActivity.intent(this, incomingSessionId)
                                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                                .addFlags(Intent.FLAG_ACTIVITY_CLEAR_TOP),
                        )
                        finish()
                        return@runOnUiThread
                    }
                    existingSessionId = draftSession.sessionId
                    selectedPackage = InstalledAppCatalog.resolveInstalledApp(
                        this,
                        sessionController,
                        checkNotNull(draftSession.targetPackage),
                    )
                    initialSettings = mergedWithPreferredDefaults(
                        sessionController.executionSettingsForSession(draftSession.sessionId),
                    )
                    targetLocked = true
                    titleView.text = "New Session"
                    updatePackageSummary()
                    lockTargetSelection()
                    statusView.visibility = View.GONE
                    startButton.isEnabled = true
                    if (uiActive) {
                        updateSessionUiLease(existingSessionId)
                    }
                    if (availableModels.isNotEmpty()) {
                        applyModelOptions()
                    }
                }
            }
        }
    }

    private fun cancelAndFinish() {
        val sessionId = existingSessionId
        if (sessionId == null) {
            finish()
            return
        }
        startButton.isEnabled = false
        thread {
            runCatching {
                sessionController.cancelSession(sessionId)
            }.onFailure { err ->
                runOnUiThread {
                    startButton.isEnabled = true
                    showToast("Failed to cancel session: ${err.message}")
                }
            }.onSuccess {
                runOnUiThread {
                    finish()
                }
            }
        }
    }

    private fun lockTargetSelection() {
        packageButton.visibility = View.GONE
        clearPackageButton.visibility = View.GONE
    }

    private fun unlockTargetSelection() {
        packageButton.visibility = View.VISIBLE
        clearPackageButton.visibility = View.VISIBLE
    }

    private fun startSession() {
        val prompt = promptInput.text.toString().trim()
        if (prompt.isEmpty()) {
            promptInput.error = "Enter a prompt"
            return
        }
        val targetPackage = selectedPackage?.packageName
        if (existingSessionId != null && targetPackage == null) {
            showToast("Missing target app for existing session")
            return
        }
        startButton.isEnabled = false
        thread {
            runCatching {
                AgentSessionLauncher.startSessionAsync(
                    context = this,
                    request = LaunchSessionRequest(
                        prompt = prompt,
                        targetPackage = targetPackage,
                        model = selectedModel().model,
                        reasoningEffort = selectedEffort(),
                        existingSessionId = existingSessionId,
                    ),
                    sessionController = sessionController,
                )
            }.onFailure { err ->
                runOnUiThread {
                    startButton.isEnabled = true
                    showToast("Failed to start session: ${err.message}")
                }
            }.onSuccess { result ->
                runOnUiThread {
                    showToast("Started session")
                    setResult(RESULT_OK, Intent().putExtra(SessionDetailActivity.EXTRA_SESSION_ID, result.parentSessionId))
                    finish()
                }
            }
        }
    }

    private fun refreshModelsIfNeeded(
        force: Boolean,
        onComplete: (() -> Unit)? = null,
    ) {
        if (!force && availableModels.isNotEmpty()) {
            onComplete?.invoke()
            return
        }
        if (onComplete != null) {
            synchronized(pendingModelCallbacks) {
                pendingModelCallbacks += onComplete
            }
        }
        if (modelsRefreshInFlight) {
            return
        }
        modelsRefreshInFlight = true
        thread {
            try {
                runCatching { AgentCodexAppServerClient.listModels(this) }
                    .onFailure { err ->
                        Log.w(TAG, "Failed to load model catalog", err)
                    }
                    .onSuccess { models ->
                        availableModels = models
                    }
            } finally {
                runOnUiThread {
                    if (availableModels.isNotEmpty()) {
                        applyModelOptions()
                    } else {
                        statusView.visibility = View.VISIBLE
                        statusView.text = "Failed to load model catalog."
                    }
                }
                modelsRefreshInFlight = false
                val callbacks = synchronized(pendingModelCallbacks) {
                    pendingModelCallbacks.toList().also { pendingModelCallbacks.clear() }
                }
                callbacks.forEach { callback -> callback.invoke() }
            }
        }
    }

    private fun applyModelOptions() {
        val models = availableModels.ifEmpty(::fallbackModels)
        if (availableModels.isEmpty()) {
            availableModels = models
        }
        val labels = models.map { model ->
            if (model.description.isBlank()) {
                model.displayName
            } else {
                "${model.displayName} (${model.description})"
            }
        }
        val adapter = ArrayAdapter(
            this,
            android.R.layout.simple_spinner_item,
            labels,
        )
        adapter.setDropDownViewResource(android.R.layout.simple_spinner_dropdown_item)
        pendingEffortOverride = initialSettings.reasoningEffort
        modelSpinner.adapter = adapter
        val modelIndex = models.indexOfFirst { it.model == initialSettings.model }
            .takeIf { it >= 0 } ?: models.indexOfFirst(AgentModelOption::isDefault)
            .takeIf { it >= 0 } ?: 0
        modelSpinner.setSelection(modelIndex, false)
        updateEffortOptions(initialSettings.reasoningEffort)
        statusView.visibility = View.GONE
    }

    private fun selectedModel(): AgentModelOption {
        return availableModels[modelSpinner.selectedItemPosition.coerceIn(0, availableModels.lastIndex)]
    }

    private fun selectedEffort(): String? {
        return selectedReasoningOptions.getOrNull(effortSpinner.selectedItemPosition)?.reasoningEffort
    }

    private fun updateEffortOptions(requestedEffort: String?) {
        if (availableModels.isEmpty()) {
            return
        }
        selectedReasoningOptions = selectedModel().supportedReasoningEfforts
        val labels = selectedReasoningOptions.map { option ->
            "${option.reasoningEffort} — ${option.description}"
        }
        effortLabelAdapter.clear()
        effortLabelAdapter.addAll(labels)
        effortLabelAdapter.notifyDataSetChanged()
        val desiredEffort = requestedEffort ?: selectedModel().defaultReasoningEffort
        val selectedIndex = selectedReasoningOptions.indexOfFirst { it.reasoningEffort == desiredEffort }
            .takeIf { it >= 0 } ?: 0
        effortSpinner.setSelection(selectedIndex, false)
    }

    private fun updatePackageSummary() {
        val app = selectedPackage
        if (app == null) {
            packageSummary.text = if (agentAnchoredOnly) {
                "Codex will plan across apps and create child Genie sessions as needed."
            } else {
                "No target app selected. This will start an Agent-anchored session."
            }
            packageSummary.setCompoundDrawablesRelativeWithIntrinsicBounds(null, null, null, null)
            return
        }
        packageSummary.text = "${app.label} (${app.packageName})"
        packageSummary.setCompoundDrawablesRelativeWithIntrinsicBounds(
            resizeIcon(app.icon),
            null,
            null,
            null,
        )
        packageSummary.compoundDrawablePadding =
            resources.getDimensionPixelSize(android.R.dimen.app_icon_size) / 4
    }

    private fun showInstalledAppPicker(onSelected: (InstalledApp) -> Unit) {
        val apps = InstalledAppCatalog.listInstalledApps(this, sessionController)
        if (apps.isEmpty()) {
            android.app.AlertDialog.Builder(this)
                .setMessage("No launchable target apps are available.")
                .setPositiveButton(android.R.string.ok, null)
                .show()
            return
        }
        val adapter = object : ArrayAdapter<InstalledApp>(
            this,
            R.layout.list_item_installed_app,
            apps,
        ) {
            override fun getView(position: Int, convertView: View?, parent: ViewGroup): View {
                return bindAppRow(position, convertView, parent)
            }

            override fun getDropDownView(position: Int, convertView: View?, parent: ViewGroup): View {
                return bindAppRow(position, convertView, parent)
            }

            private fun bindAppRow(position: Int, convertView: View?, parent: ViewGroup): View {
                val row = convertView ?: LayoutInflater.from(context)
                    .inflate(R.layout.list_item_installed_app, parent, false)
                val app = getItem(position) ?: return row
                val iconView = row.findViewById<ImageView>(R.id.installed_app_icon)
                val titleView = row.findViewById<TextView>(R.id.installed_app_title)
                val subtitleView = row.findViewById<TextView>(R.id.installed_app_subtitle)
                iconView.setImageDrawable(app.icon ?: getDrawable(android.R.drawable.sym_def_app_icon))
                titleView.text = app.label
                subtitleView.text = if (app.eligibleTarget) {
                    app.packageName
                } else {
                    "${app.packageName} — unavailable"
                }
                row.isEnabled = app.eligibleTarget
                titleView.isEnabled = app.eligibleTarget
                subtitleView.isEnabled = app.eligibleTarget
                iconView.alpha = if (app.eligibleTarget) 1f else 0.5f
                row.alpha = if (app.eligibleTarget) 1f else 0.6f
                return row
            }
        }
        val dialog = android.app.AlertDialog.Builder(this)
            .setTitle("Choose app")
            .setAdapter(adapter) { _, which ->
                val app = apps[which]
                if (!app.eligibleTarget) {
                    android.app.AlertDialog.Builder(this)
                        .setMessage(
                            "The current framework rejected ${app.packageName} as a target for Genie sessions on this device.",
                        )
                        .setPositiveButton(android.R.string.ok, null)
                        .show()
                    return@setAdapter
                }
                onSelected(app)
            }
            .setNegativeButton(android.R.string.cancel, null)
            .create()
        dialog.setOnShowListener {
            dialog.listView?.isVerticalScrollBarEnabled = true
            dialog.listView?.isScrollbarFadingEnabled = false
            dialog.listView?.isFastScrollEnabled = true
            dialog.listView?.scrollBarStyle = View.SCROLLBARS_INSIDE_INSET
        }
        dialog.show()
    }

    private fun findStandaloneHomeDraftSession(sessionId: String): AgentSessionDetails? {
        val snapshot = sessionController.loadSnapshot(sessionId)
        val session = snapshot.sessions.firstOrNull { it.sessionId == sessionId } ?: return null
        val hasChildren = snapshot.sessions.any { it.parentSessionId == sessionId }
        return session.takeIf {
            it.anchor == AgentSessionInfo.ANCHOR_HOME &&
                it.state == AgentSessionInfo.STATE_CREATED &&
                !hasChildren &&
                !it.targetPackage.isNullOrBlank()
        }
    }

    private fun isAgentSessionLauncherIntent(intent: Intent?): Boolean {
        return intent?.action == Intent.ACTION_MAIN &&
            intent.hasCategory(Intent.CATEGORY_LAUNCHER) &&
            intent.getStringExtra(AgentManager.EXTRA_SESSION_ID).isNullOrBlank()
    }

    private fun updateSessionUiLease(sessionId: String?) {
        if (leasedSessionId == sessionId) {
            return
        }
        leasedSessionId?.let { previous ->
            runCatching {
                sessionController.unregisterSessionUiLease(previous, sessionUiLeaseToken)
            }
            leasedSessionId = null
        }
        sessionId?.let { current ->
            val registered = runCatching {
                sessionController.registerSessionUiLease(current, sessionUiLeaseToken)
            }
            if (registered.isSuccess) {
                leasedSessionId = current
            }
        }
    }

    private fun resizeIcon(icon: Drawable?): Drawable? {
        val sizedIcon = icon?.constantState?.newDrawable()?.mutate() ?: return null
        val iconSize = resources.getDimensionPixelSize(android.R.dimen.app_icon_size)
        sizedIcon.setBounds(0, 0, iconSize, iconSize)
        return sizedIcon
    }

    private fun fallbackModels(): List<AgentModelOption> {
        return listOf(
            AgentModelOption(
                id = initialSettings.model ?: DEFAULT_MODEL,
                model = initialSettings.model ?: DEFAULT_MODEL,
                displayName = initialSettings.model ?: DEFAULT_MODEL,
                description = "Current Agent runtime default",
                supportedReasoningEfforts = listOf(
                    AgentReasoningEffortOption("minimal", "Fastest"),
                    AgentReasoningEffortOption("low", "Low"),
                    AgentReasoningEffortOption("medium", "Balanced"),
                    AgentReasoningEffortOption("high", "Deep"),
                    AgentReasoningEffortOption("xhigh", "Max"),
                ),
                defaultReasoningEffort = initialSettings.reasoningEffort ?: DEFAULT_REASONING_EFFORT,
                isDefault = true,
            ),
        )
    }

    private fun showToast(message: String) {
        runOnUiThread {
            Toast.makeText(this, message, Toast.LENGTH_SHORT).show()
        }
    }
}
