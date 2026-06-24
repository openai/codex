-- Guardian prompts are synthetic review context, not user-authored messages.
-- Keep the SQLite projection small; rollout JSONL remains canonical.
-- Legacy automatic titles were empty or copied first_user_message; preserve a
-- different non-empty title because it may have been set explicitly.
UPDATE threads
SET title = CASE
        WHEN trim(title) = '' OR trim(title) = trim(first_user_message)
            THEN 'Guardian review'
        ELSE title
    END,
    preview = 'Approval review',
    first_user_message = ''
WHERE source = '{"subagent":{"other":"guardian"}}';
