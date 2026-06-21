CREATE INDEX idx_thread_spawn_edges_parent_child
    ON thread_spawn_edges(parent_thread_id, child_thread_id);
