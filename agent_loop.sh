#!/bin/bash
set -euo pipefail

mkdir -p ./workspace/logs

while true; do
  step_ts="$(date -Is)"
  log_file="./workspace/logs/agent-$step_ts.log"

  # Detect the most recently active in-progress task
  current_task_file="$(ls -t ./workspace/tasks/in-progress 2>/dev/null | head -n1 || true)"

  task_branch=""
  if [ -n "$current_task_file" ]; then
    task_slug="${current_task_file%.md}"
    task_branch="task/$task_slug"

    # create or switch to the branch for this task
    if git rev-parse --verify "$task_branch" >/dev/null 2>&1; then
      git switch "$task_branch"
    else
      git switch -c "$task_branch"
    fi
  fi

  codex exec \
    --dangerously-bypass-approvals-and-sandbox \
    -C . \
    "$(cat <<AGENT_PROMPT
You are working in the monorepo root. Plans and tasks live under ./workspace.

Branching rules:

- Each in-progress task has a dedicated branch:
  task/<task-file-basename>
- All edits for that task happen on its branch.
- When a task is finished and moved to ./workspace/tasks/done,
  append a final History entry marking completion.

Important: When a task is marked done, do not continue editing it afterward.
Completion triggers a squash merge handled outside the agent.

Parallelization policy:

- When the current task contains multiple independent parts,
  you may break the work into subtasks and run them in parallel.
- You are allowed to spawn additional Codex workers yourself by starting
  background processes such as:

  codex exec --dangerously-bypass-approvals-and-sandbox -C . "subtask prompt here" &

- Never run more than 8 Codex workers at the same time.
- Treat the current process as the coordinator. Sub-workers perform
  bounded concrete work; the coordinator supervises and integrates results.

How to parallelize:

1) Inspect the current task. If it can be decomposed into parts that do not
   conflict on the same files, define clear subtasks.

2) For each independent subtask:
   - spawn one Codex exec worker (up to 8 total)
   - each worker performs one bounded change
   - each worker appends a timestamped History entry in the parent task file
   - each worker commits its change on the same task branch

3) The coordinator:
   - tracks spawned workers
   - waits when appropriate
   - reconciles results and resolves conflicts if they appear
   - writes a short coordination History entry

4) If parallelism is unsafe or unnecessary,
   fall back to a single bounded change in this process.

Behavior step ($step_ts):

1) If exactly one task exists in ./workspace/tasks/in-progress:
   - continue that task on its branch
   - either run parallel subtasks (if appropriate) OR
     make a single bounded incremental edit with a History entry

2) If multiple tasks are in progress:
   - pick the most recently edited one and continue only that one

3) If no tasks are in progress:
   - If there are uncommitted changes, reconcile them with task History
   - Otherwise, pick the oldest task in ./workspace/tasks/todo,
     move it to in-progress, create/switch branch, append "work started"

Constraints:
- Only modify Markdown inside ./workspace
- Append timestamped History; never delete earlier entries
- Keep filenames stable; record renames in History
- Keep changes small and traceable

State what you chose to do, perform the concrete action, then stop.
AGENT_PROMPT
)"
  # stage and commit changes produced by the step
  git add -A
  if ! git diff --cached --quiet; then
    git commit -m "task step: $step_ts — incremental workspace update" \
      -m "log: $(basename "$log_file")" || true
  fi

  # --- Detect task completion and squash-merge to main ---
  # If this task file is no longer in in-progress, assume it was moved to done
  if [ -n "$task_branch" ] && \
     [ ! -f "./workspace/tasks/in-progress/$current_task_file" ] && \
     [ -f "./workspace/tasks/done/$current_task_file" ]; then

    echo "[$step_ts] detected completion of $task_branch — squash-merging to main" \
      | tee -a "$log_file"

    # ensure we’re on the task branch for merge source
    git switch "$task_branch" || true

    # switch to main and update it
    git switch main || git switch -c main
    git pull --ff-only || true

    # squash-merge the task branch
    git merge --squash "$task_branch" || true
    git commit -m "complete $task_branch — squash merge of task work" \
      -m "task: $task_slug" || true

    # optional cleanup:
    # git branch -d "$task_branch" || true
  fi

  echo "[$step_ts] agent step complete on branch: $(git branch --show-current)" \
    | tee -a "$log_file"

  sleep 5
done
