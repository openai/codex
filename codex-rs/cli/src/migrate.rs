use std::fs;
use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use chrono::Local;
use chrono::Utc;
use clap::Parser;
use clap::Subcommand;
use clap::ValueEnum;
use codex_tui::migration::MigrationWorkspace;
use codex_tui::migration::create_migration_workspace;
use pathdiff::diff_paths;
use serde::Deserialize;
use serde::Serialize;

const STATE_DIR: &str = ".codex/migrate";
const INDEX_FILE: &str = "index.json";
const MIGRATIONS_DIR: &str = "migrations";
const TASKS_FILE: &str = "tasks.json";
const RUNS_DIR: &str = "runs";
const STATE_VERSION: u32 = 1;
const INDEX_VERSION: u32 = 1;

#[derive(Debug, Parser)]
pub struct MigrateCli {
    /// Root of the repository / workspace that owns the migration artifacts.
    #[arg(long = "root", value_name = "DIR", default_value = ".")]
    root: PathBuf,

    #[command(subcommand)]
    command: MigrateCommand,
}

#[derive(Debug, Subcommand)]
enum MigrateCommand {
    /// Create a migration workspace and seed a task graph.
    Plan(PlanCommand),
    /// Execute or update a migration task.
    Execute(ExecuteCommand),
}

#[derive(Debug, Parser)]
struct PlanCommand {
    /// Short description for the migration (used to name the workspace).
    #[arg(value_name = "DESCRIPTION")]
    summary: String,

    /// How many explorer workstreams should be created for parallel agents.
    #[arg(long = "parallel", value_name = "COUNT", default_value_t = 2)]
    parallel_scouts: usize,
}

#[derive(Debug, Parser)]
struct ExecuteCommand {
    /// Specific task id to update. Omit to pick the next runnable task.
    #[arg(value_name = "TASK_ID")]
    task_id: Option<String>,

    /// Name (or path) of the migration workspace to operate on.
    #[arg(long = "workspace", value_name = "PATH")]
    workspace: Option<String>,

    /// Explicitly set a task's status instead of starting it.
    #[arg(long = "status", value_enum, requires = "task_id")]
    status: Option<TaskStatus>,

    /// Append a short note to journal.md after updating the task.
    #[arg(long = "note", value_name = "TEXT")]
    note: Option<String>,
}

impl MigrateCli {
    pub fn run(self) -> Result<()> {
        let root = self
            .root
            .canonicalize()
            .unwrap_or_else(|_| self.root.clone());
        match self.command {
            MigrateCommand::Plan(cmd) => run_plan(&root, cmd),
            MigrateCommand::Execute(cmd) => run_execute(&root, cmd),
        }
    }
}

fn run_plan(root: &Path, cmd: PlanCommand) -> Result<()> {
    fs::create_dir_all(package_dir(root))?;
    let migrations_dir = root.join(MIGRATIONS_DIR);
    let workspace = create_migration_workspace(&migrations_dir, cmd.summary.as_str())
        .with_context(|| {
            format!(
                "failed to create migration workspace inside {}",
                migrations_dir.display()
            )
        })?;
    let parallel = cmd.parallel_scouts.clamp(1, 8);
    let state = MigrationState::new(cmd.summary.clone(), &workspace, parallel);
    state.save()?;
    write_workspace_readme(&workspace, cmd.summary.as_str())?;
    let workspace_rel = diff_paths(&workspace.dir_path, root)
        .unwrap_or_else(|| workspace.dir_path.clone())
        .display()
        .to_string();
    refresh_index(root, &state)?;
    println!(
        "Created migration workspace `{}` in {workspace_rel}",
        workspace.dir_name
    );
    println!("- Plan: {}", rel_to_root(&workspace.plan_path, root));
    println!("- Journal: {}", rel_to_root(&workspace.journal_path, root));
    println!(
        "Next: open this repo in Codex, run /migrate, and let the agent follow up with `migrate-cli execute` to begin running tasks."
    );
    Ok(())
}

fn run_execute(root: &Path, cmd: ExecuteCommand) -> Result<()> {
    let workspace_dir = resolve_workspace(root, cmd.workspace.as_deref())?;
    let mut state = MigrationState::load(workspace_dir)?;
    let task_id = if let Some(id) = cmd.task_id {
        id
    } else if let Some(id) = state.next_runnable_task_id() {
        id
    } else {
        println!("All tasks are complete. Specify --task-id to override.");
        return Ok(());
    };
    if !state.can_start(&task_id) && cmd.status.is_none() {
        anyhow::bail!(
            "Task `{task_id}` is blocked by its dependencies. Complete the prerequisites or pass --status to override."
        );
    }
    let describe_task = cmd.status.is_none();
    let task_snapshot = if describe_task {
        Some(
            state
                .task(&task_id)
                .cloned()
                .with_context(|| format!("unknown task id `{task_id}`"))?,
        )
    } else {
        None
    };
    let new_status = cmd.status.unwrap_or(TaskStatus::Running);
    state.set_status(&task_id, new_status)?;
    let mut run_file = None;
    if new_status == TaskStatus::Running && describe_task {
        run_file = Some(write_run_file(root, &state, &task_id)?);
    }
    state.save()?;
    if let Some(note) = cmd.note {
        append_journal(&state, &task_id, new_status, note.as_str())?;
    }
    refresh_index(root, &state)?;
    if describe_task {
        if let Some(task) = task_snapshot.as_ref() {
            print_task_brief(&state, task, root);
        }
        if let Some(path) = run_file {
            println!("Runbook prepared at {path}");
        }
        println!(
            "When you finish, mark it done with `migrate-cli execute --task-id {task_id} --status done --note \"<summary>\"` and run `migrate-cli execute` again for the next task."
        );
    } else {
        println!("Task `{task_id}` status -> {new_status}");
        if let Some(path) = run_file {
            println!("Runbook prepared at {path}");
        }
    }
    Ok(())
}

fn resolve_workspace(root: &Path, provided: Option<&str>) -> Result<PathBuf> {
    if let Some(input) = provided {
        let direct = PathBuf::from(input);
        let candidate = if direct.is_absolute() {
            direct
        } else {
            let joined = root.join(&direct);
            if joined.join(TASKS_FILE).exists() {
                joined
            } else {
                root.join(MIGRATIONS_DIR).join(&direct)
            }
        };
        if candidate.join(TASKS_FILE).exists() {
            return Ok(candidate);
        }
        anyhow::bail!("No migration workspace found at {}", candidate.display());
    }
    let index = load_index(&index_path(root))?;
    let latest = index
        .migrations
        .iter()
        .max_by_key(|entry| entry.updated_at_epoch)
        .context("No recorded migrations. Run `migrate-cli plan` first.")?;
    let rel = PathBuf::from(&latest.workspace);
    let path = if rel.is_absolute() {
        rel
    } else {
        root.join(rel)
    };
    Ok(path)
}

fn write_workspace_readme(workspace: &MigrationWorkspace, summary: &str) -> Result<()> {
    let contents = format!(
        "# {name}\n\n{summary}\n\n- `plan.md` – canonical blueprint\n- `journal.md` – publish progress + hand-offs\n- `tasks.json` – orchestration metadata\n- `runs/` – generated runbooks per task\n\nUse `migrate-cli execute --workspace {name}` to advance tasks or open this folder in Codex and run `/migrate`.\n",
        name = workspace.dir_name,
        summary = summary
    );
    fs::write(workspace.dir_path.join("README.md"), contents)?;
    Ok(())
}

fn append_journal(
    state: &MigrationState,
    task_id: &str,
    status: TaskStatus,
    note: &str,
) -> Result<()> {
    let mut file = OpenOptions::new()
        .append(true)
        .open(state.journal_path())
        .with_context(|| format!("failed to open {}", state.journal_path().display()))?;
    let timestamp = Local::now().format("%Y-%m-%d %H:%M %Z");
    writeln!(
        file,
        "| {timestamp} | migrate::execute | Task {task_id} -> {status} |  | {note} |"
    )?;
    Ok(())
}

fn write_run_file(root: &Path, state: &MigrationState, task_id: &str) -> Result<String> {
    let task = state
        .task(task_id)
        .with_context(|| format!("unknown task id `{task_id}`"))?;
    let runs_dir = state.workspace_dir().join(RUNS_DIR);
    fs::create_dir_all(&runs_dir)?;
    let timestamp = Utc::now().format("%Y%m%d-%H%M%S");
    let file_name = format!("{task_id}-{timestamp}.md");
    let path = runs_dir.join(&file_name);
    let plan = rel_to_root(&state.plan_path(), root);
    let journal = rel_to_root(&state.journal_path(), root);
    let mut body = format!(
        "# Task {task_id}: {}\n\n{}\n\n## Checkpoints\n",
        task.title, task.description
    );
    for checkpoint in &task.checkpoints {
        body.push_str(&format!("- {checkpoint}\n"));
    }
    body.push_str(&format!(
        "\nPublish updates to `{journal}`. Mirror final scope into `{plan}` when it changes.\n"
    ));
    fs::write(&path, body)?;
    Ok(rel_to_root(&path, root))
}

fn print_task_brief(state: &MigrationState, task: &MigrationTask, root: &Path) {
    println!("--- migrate::execute ---");
    println!("Workspace: {}", state.workspace_dir_string(root));
    println!("Plan: {}", rel_to_root(&state.plan_path(), root));
    println!("Journal: {}", rel_to_root(&state.journal_path(), root));
    println!();
    println!("Task `{}` – {}", task.id, task.title);
    println!("{}", task.description);
    if !task.depends_on.is_empty() {
        println!("Depends on: {}", task.depends_on.join(", "));
    }
    if let Some(group) = &task.parallel_group {
        println!("Parallel track: {group}");
    }
    if let Some(owner) = &task.owner_hint {
        println!("Suggested owner: {owner}");
    }
    if !task.publish_to.is_empty() {
        println!("Publish updates to: {}", task.publish_to.join(", "));
    }
    if !task.checkpoints.is_empty() {
        println!("Checkpoints:");
        for checkpoint in &task.checkpoints {
            println!("  - {checkpoint}");
        }
    }
    println!(
        "Document findings in journal.md, reflect scope changes back into plan.md, and keep runbooks inside runs/."
    );
}

fn package_dir(root: &Path) -> PathBuf {
    root.join(STATE_DIR)
}

fn index_path(root: &Path) -> PathBuf {
    package_dir(root).join(INDEX_FILE)
}

fn rel_to_root(path: &Path, root: &Path) -> String {
    diff_paths(path, root)
        .unwrap_or_else(|| path.to_path_buf())
        .display()
        .to_string()
}

fn write_pretty_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let text = serde_json::to_string_pretty(value)?;
    fs::write(path, text)?;
    Ok(())
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct MigrationIndexEntry {
    slug: String,
    summary: String,
    workspace: String,
    plan: String,
    journal: String,
    tasks_path: String,
    pending_tasks: usize,
    running_tasks: usize,
    blocked_tasks: usize,
    ready_parallel_tasks: Vec<String>,
    status: IndexStatus,
    updated_at: String,
    updated_at_epoch: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum IndexStatus {
    Planning,
    Executing,
    Complete,
}

#[derive(Debug, Serialize, Deserialize)]
struct MigrationIndex {
    version: u32,
    migrations: Vec<MigrationIndexEntry>,
}

impl Default for MigrationIndex {
    fn default() -> Self {
        Self {
            version: INDEX_VERSION,
            migrations: Vec::new(),
        }
    }
}

fn load_index(path: &Path) -> Result<MigrationIndex> {
    if path.exists() {
        let text = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&text)?)
    } else {
        Ok(MigrationIndex::default())
    }
}

fn refresh_index(root: &Path, state: &MigrationState) -> Result<()> {
    fs::create_dir_all(package_dir(root))?;
    let mut index = load_index(&index_path(root))?;
    let entry = state.to_index_entry(root);
    index
        .migrations
        .retain(|existing| existing.slug != entry.slug || existing.workspace != entry.workspace);
    index.migrations.push(entry);
    write_pretty_json(&index_path(root), &index)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
enum TaskStatus {
    #[default]
    Pending,
    Running,
    Blocked,
    Done,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            TaskStatus::Pending => "pending",
            TaskStatus::Running => "running",
            TaskStatus::Blocked => "blocked",
            TaskStatus::Done => "done",
        };
        write!(f, "{label}")
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct MigrationTask {
    id: String,
    title: String,
    description: String,
    #[serde(default)]
    status: TaskStatus,
    #[serde(default)]
    depends_on: Vec<String>,
    #[serde(default)]
    parallel_group: Option<String>,
    #[serde(default)]
    owner_hint: Option<String>,
    #[serde(default)]
    publish_to: Vec<String>,
    #[serde(default)]
    checkpoints: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct MigrationStateFile {
    version: u32,
    summary: String,
    slug: String,
    plan_path: String,
    journal_path: String,
    tasks: Vec<MigrationTask>,
}

struct MigrationState {
    file: MigrationStateFile,
    workspace_dir: PathBuf,
}

impl MigrationState {
    fn new(summary: String, workspace: &MigrationWorkspace, parallel: usize) -> Self {
        let tasks = seed_tasks(&summary, parallel);
        Self {
            file: MigrationStateFile {
                version: STATE_VERSION,
                summary,
                slug: workspace.dir_name.clone(),
                plan_path: "plan.md".to_string(),
                journal_path: "journal.md".to_string(),
                tasks,
            },
            workspace_dir: workspace.dir_path.clone(),
        }
    }

    fn load(workspace_dir: PathBuf) -> Result<Self> {
        let data_path = workspace_dir.join(TASKS_FILE);
        let text = fs::read_to_string(&data_path)
            .with_context(|| format!("missing tasks file at {}", data_path.display()))?;
        let file: MigrationStateFile = serde_json::from_str(&text)?;
        Ok(Self {
            file,
            workspace_dir,
        })
    }

    fn save(&self) -> Result<()> {
        write_pretty_json(&self.workspace_dir.join(TASKS_FILE), &self.file)
    }

    fn workspace_dir(&self) -> &Path {
        &self.workspace_dir
    }

    fn plan_path(&self) -> PathBuf {
        self.workspace_dir.join(&self.file.plan_path)
    }

    fn journal_path(&self) -> PathBuf {
        self.workspace_dir.join(&self.file.journal_path)
    }

    fn task(&self, id: &str) -> Option<&MigrationTask> {
        self.file.tasks.iter().find(|task| task.id == id)
    }

    fn task_mut(&mut self, id: &str) -> Option<&mut MigrationTask> {
        self.file.tasks.iter_mut().find(|task| task.id == id)
    }

    fn set_status(&mut self, id: &str, status: TaskStatus) -> Result<()> {
        let task = self
            .task_mut(id)
            .with_context(|| format!("unknown task id `{id}`"))?;
        task.status = status;
        Ok(())
    }

    fn next_runnable_task_id(&self) -> Option<String> {
        self.file
            .tasks
            .iter()
            .find(|task| task.status == TaskStatus::Pending && self.dependencies_met(task))
            .map(|task| task.id.clone())
    }

    fn dependencies_met(&self, task: &MigrationTask) -> bool {
        task.depends_on.iter().all(|dep| {
            self.file
                .tasks
                .iter()
                .find(|t| &t.id == dep)
                .map(|t| t.status == TaskStatus::Done)
                .unwrap_or(false)
        })
    }

    fn can_start(&self, id: &str) -> bool {
        self.task(id)
            .map(|task| self.dependencies_met(task))
            .unwrap_or(false)
    }

    fn workspace_dir_string(&self, root: &Path) -> String {
        rel_to_root(&self.workspace_dir, root)
    }

    fn ready_parallel_tasks(&self) -> Vec<String> {
        self.file
            .tasks
            .iter()
            .filter(|task| task.parallel_group.is_some())
            .filter(|task| task.status == TaskStatus::Pending)
            .filter(|task| self.dependencies_met(task))
            .map(|task| task.id.clone())
            .collect()
    }

    fn status_counts(&self) -> (usize, usize, usize, usize) {
        let mut pending = 0;
        let mut running = 0;
        let mut blocked = 0;
        let mut done = 0;
        for task in &self.file.tasks {
            match task.status {
                TaskStatus::Pending => pending += 1,
                TaskStatus::Running => running += 1,
                TaskStatus::Blocked => blocked += 1,
                TaskStatus::Done => done += 1,
            }
        }
        (pending, running, blocked, done)
    }

    fn to_index_entry(&self, root: &Path) -> MigrationIndexEntry {
        let (pending, running, blocked, _done) = self.status_counts();
        let ready_parallel_tasks = self.ready_parallel_tasks();
        let status = if pending == 0 && running == 0 && blocked == 0 {
            IndexStatus::Complete
        } else if running > 0 {
            IndexStatus::Executing
        } else {
            IndexStatus::Planning
        };
        let now = Utc::now();
        MigrationIndexEntry {
            slug: self.file.slug.clone(),
            summary: self.file.summary.clone(),
            workspace: self.workspace_dir_string(root),
            plan: rel_to_root(&self.plan_path(), root),
            journal: rel_to_root(&self.journal_path(), root),
            tasks_path: rel_to_root(&self.workspace_dir.join(TASKS_FILE), root),
            pending_tasks: pending,
            running_tasks: running,
            blocked_tasks: blocked,
            ready_parallel_tasks,
            status,
            updated_at: now.to_rfc3339(),
            updated_at_epoch: now.timestamp(),
        }
    }
}

fn seed_tasks(summary: &str, parallel: usize) -> Vec<MigrationTask> {
    let mut tasks = Vec::new();
    let plan_targets = vec!["plan.md".to_string(), "journal.md".to_string()];
    tasks.push(MigrationTask {
        id: "plan-baseline".to_string(),
        title: "Map current + target states".to_string(),
        description: format!(
            "Capture why `{summary}` is needed, current system contracts, and the desired end state in `plan.md`."
        ),
        publish_to: plan_targets.clone(),
        checkpoints: vec![
            "Document repositories, services, and owners".to_string(),
            "List non-negotiable constraints".to_string(),
        ],
        ..Default::default()
    });
    tasks.push(MigrationTask {
        id: "plan-guardrails".to_string(),
        title: "Design guardrails + approvals".to_string(),
        description: "Spell out kill-switches, approvals, and telemetry gating.".to_string(),
        depends_on: vec!["plan-baseline".to_string()],
        publish_to: plan_targets.clone(),
        checkpoints: vec![
            "Define approval owners".to_string(),
            "List telemetry + alerting hooks".to_string(),
        ],
        ..Default::default()
    });
    tasks.push(MigrationTask {
        id: "plan-blueprint".to_string(),
        title: "Lock incremental rollout plan".to_string(),
        description: "Lay out the numbered steps and decision records for the migration."
            .to_string(),
        depends_on: vec!["plan-guardrails".to_string()],
        publish_to: plan_targets.clone(),
        checkpoints: vec![
            "Identify sequencing + dependencies".to_string(),
            "Assign owners to each increment".to_string(),
        ],
        ..Default::default()
    });

    let mut sources: Vec<String> = (1..=parallel.max(1))
        .map(|i| format!("workstream #{i}"))
        .collect();
    if sources.is_empty() {
        sources.push("workstream #1".to_string());
    }

    for (idx, source) in sources.iter().enumerate() {
        tasks.push(MigrationTask {
            id: format!("parallel-scout-{}", idx + 1),
            title: format!("Deep-dive: {source}"),
            description: format!(
                "Inventory blockers, data contracts, and automation opportunities for `{source}`. Feed findings into journal.md and update plan.md if scope shifts."
            ),
            depends_on: vec!["plan-blueprint".to_string()],
            parallel_group: Some("exploration".to_string()),
            owner_hint: Some("domain expert".to_string()),
            publish_to: plan_targets.clone(),
            checkpoints: vec![
                "Publish progress + artifacts to journal.md".to_string(),
                "Flag shared learnings for other workstreams".to_string(),
            ],
            ..Default::default()
        });
    }

    tasks.push(MigrationTask {
        id: "parallel-telemetry".to_string(),
        title: "Build shared telemetry + rehearsal harness".to_string(),
        description:
            "Codify validation scripts, load tests, and dashboards each workstream will reuse."
                .to_string(),
        depends_on: vec!["plan-blueprint".to_string()],
        parallel_group: Some("stabilization".to_string()),
        publish_to: plan_targets.clone(),
        checkpoints: vec![
            "Link dashboards in journal.md".to_string(),
            "Tag required signals per task".to_string(),
        ],
        ..Default::default()
    });
    tasks.push(MigrationTask {
        id: "parallel-backfill".to_string(),
        title: "Design data backfill + rollback story".to_string(),
        description: "Document backfill tooling, rehearsal cadence, and rollback triggers."
            .to_string(),
        depends_on: vec!["plan-blueprint".to_string()],
        parallel_group: Some("stabilization".to_string()),
        publish_to: plan_targets.clone(),
        checkpoints: vec![
            "Note dry-run schedule in journal.md".to_string(),
            "List reversibility safeguards".to_string(),
        ],
        ..Default::default()
    });

    let mut cutover_dependencies = vec![
        "plan-baseline".to_string(),
        "plan-guardrails".to_string(),
        "plan-blueprint".to_string(),
        "parallel-telemetry".to_string(),
        "parallel-backfill".to_string(),
    ];
    cutover_dependencies.extend(
        sources
            .iter()
            .enumerate()
            .map(|(idx, _)| format!("parallel-scout-{}", idx + 1)),
    );

    tasks.push(MigrationTask {
        id: "plan-cutover".to_string(),
        title: "Execute rollout + capture learnings".to_string(),
        description: "Drive the migration, capture deviations, and publish the final hand-off."
            .to_string(),
        depends_on: cutover_dependencies,
        publish_to: plan_targets,
        checkpoints: vec![
            "Attach final verification evidence".to_string(),
            "Document kill-switch + rollback state".to_string(),
        ],
        ..Default::default()
    });

    tasks
}

impl Default for MigrationTask {
    fn default() -> Self {
        Self {
            id: String::new(),
            title: String::new(),
            description: String::new(),
            status: TaskStatus::Pending,
            depends_on: Vec::new(),
            parallel_group: None,
            owner_hint: None,
            publish_to: Vec::new(),
            checkpoints: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn next_task_unlocked_after_dependencies_complete() {
        let tmp = TempDir::new().unwrap();
        let workspace = MigrationWorkspace {
            dir_path: tmp.path().to_path_buf(),
            dir_name: "migration_demo".to_string(),
            plan_path: tmp.path().join("plan.md"),
            journal_path: tmp.path().join("journal.md"),
        };
        fs::write(&workspace.plan_path, "plan").unwrap();
        fs::write(&workspace.journal_path, "journal").unwrap();
        let mut state = MigrationState::new("Demo".to_string(), &workspace, 1);
        assert_eq!(
            state.next_runnable_task_id().as_deref(),
            Some("plan-baseline")
        );
        state.set_status("plan-baseline", TaskStatus::Done).unwrap();
        state
            .set_status("plan-guardrails", TaskStatus::Done)
            .unwrap();
        assert_eq!(
            state.next_runnable_task_id().as_deref(),
            Some("plan-blueprint")
        );
    }

    #[test]
    fn ready_parallel_tasks_wait_for_blueprint() {
        let tmp = TempDir::new().unwrap();
        let workspace = MigrationWorkspace {
            dir_path: tmp.path().to_path_buf(),
            dir_name: "migration_demo".to_string(),
            plan_path: tmp.path().join("plan.md"),
            journal_path: tmp.path().join("journal.md"),
        };
        fs::write(&workspace.plan_path, "plan").unwrap();
        fs::write(&workspace.journal_path, "journal").unwrap();
        let mut state = MigrationState::new("Demo".to_string(), &workspace, 2);
        assert!(state.ready_parallel_tasks().is_empty());
        state.set_status("plan-baseline", TaskStatus::Done).unwrap();
        state
            .set_status("plan-guardrails", TaskStatus::Done)
            .unwrap();
        state
            .set_status("plan-blueprint", TaskStatus::Done)
            .unwrap();
        let ready = state.ready_parallel_tasks();
        let ready_set: std::collections::HashSet<_> = ready.into_iter().collect();
        let expected = std::collections::HashSet::from([
            "parallel-scout-1".to_string(),
            "parallel-scout-2".to_string(),
            "parallel-telemetry".to_string(),
            "parallel-backfill".to_string(),
        ]);
        assert_eq!(ready_set, expected);
    }
}
