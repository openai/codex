CREATE TABLE thread_goal_continuation_deferrals (
    thread_id TEXT PRIMARY KEY NOT NULL REFERENCES thread_goals(thread_id) ON DELETE CASCADE
);
