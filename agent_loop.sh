#!/bin/bash
set -euo pipefail

mkdir -p ./workspace/logs
mkdir -p ./workspace/tasks/{todo,in-progress,outbound,done}
mkdir -p ./workspace/sessions

session_file_for() {
  echo "./workspace/sessions/$1.session"
}

pick_next_task() {
  ls ./workspace/tasks/todo/*.md 2>/dev/null | sort | head -n1 || true
}

current_in_progress_count() {
  ls ./workspace/tasks/in-progress/*.md 2>/dev/null | wc -l
}

the_single_in_progress_task() {
  ls ./workspace/tasks/in-progress/*.md 2>/dev/null | head -n1 || true
}

start_agent_for_task() {
  local slug="$1"
  local file_path="$2"
  local branch="task/$slug"
  local session_file
  session_file="$(session_file_for "$slug")"

  echo "Starting agent for $slug on branch $branch"

  # move task to in-progress (only if not already there)
  if [ "$file_path" != "./workspace/tasks/in-progress/$slug.md" ]; then
    mv "$file_path" "./workspace/tasks/in-progress/$slug.md"
  fi

  # ensure branch exists and switch
  if git rev-parse --verify "$branch" >/dev/null 2>&1; then
    git switch "$branch"
  else
    git switch -c "$branch"
  fi

  codex exec \
    --dangerously-bypass-approvals-and-sandbox \
    -C . \
    --save-session-id "$session_file" \
    """
You are working on task: $slug

Task file:
./workspace/tasks/in-progress/$slug.md

This branch is dedicated to this task:
task/$slug

You are responsible for:
- making bounded edits
- appending timestamped History entries
- committing your own changes
- optionally parallelizing subtasks (max 8 workers)

Completion protocol:

When the task is fully complete:
1) Append a final History entry describing completion
2) Move the task file to:
   ./workspace/tasks/outbound/$slug.md
3) EXIT the session

Do NOT merge to main yourself.
The script will handle squash-merge once the file
appears in outbound.

Stay focused on this task unless you explicitly
decide otherwise and justify it in History.
"""
}

merge_outbound_task() {
  local slug="$1"
  local branch="task/$slug"
  local session_file
  session_file="$(session_file_for "$slug")"

  echo "Merging completed task: $slug (branch $branch)"

  # ensure we are ON the task branch
  git switch "$branch"

  # verify state
  if [ ! -f "./workspace/tasks/outbound/$slug.md" ]; then
    echo "ERROR: outbound file missing for $slug — refusing merge"
    return 1
  fi

  # merge to main
  git switch main || git switch -c main
  git pull --ff-only || true

  if git merge --squash "$branch"; then
    git commit -m "complete task $slug — squash merge"
  else
    echo "Merge failed — leaving branch unmerged"
    return 1
  fi

  mv "./workspace/tasks/outbound/$slug.md" "./workspace/tasks/done/$slug.md"

  # optional cleanup:
  # git branch -d "$branch" || true

  rm -f "$session_file"

  echo "Merged and archived: $slug"
}

resume_task_session() {
  local slug="$1"
  local session_file
  session_file="$(session_file_for "$slug")"

  if [ ! -f "$session_file" ]; then
    echo "No session file for $slug — starting new agent"
    start_agent_for_task "$slug" "./workspace/tasks/in-progress/$slug.md"
    return
  fi

  echo "Resuming session for task $slug ($(cat "$session_file"))"
  codex resume "$(cat "$session_file")" || rm -f "$session_file"
}

validate_in_progress_state_or_die() {
  local count
  count="$(current_in_progress_count)"

  current_branch="$(git branch --show-current || true)"

  if [[ "$current_branch" =~ ^task/ ]]; then
    slug="${current_branch#task/}"

    if [ "$count" -ne 1 ]; then
      echo "ERROR: on task branch $current_branch but $count tasks in-progress"
      echo "This is an inconsistent state — stop and fix manually."
      exit 1
    fi

    file="./workspace/tasks/in-progress/$slug.md"
    if [ ! -f "$file" ]; then
      echo "ERROR: branch $current_branch does not match in-progress file"
      exit 1
    fi
  fi
}

while true; do
  step_ts="$(date -Is)"
  echo "[$step_ts] loop tick"

  validate_in_progress_state_or_die

  # ---- CASE 1: outbound task → merge ----
  outbound_task="$(ls ./workspace/tasks/outbound/*.md 2>/dev/null | head -n1 || true)"
  if [ -n "$outbound_task" ]; then
    slug="$(basename "${outbound_task%.md}")"
    merge_outbound_task "$slug"
    sleep 2
    continue
  fi

  # ---- CASE 2: task in progress → resume or start ----
  if [ "$(current_in_progress_count)" -eq 1 ]; then
    task_file="$(the_single_in_progress_task)"
    slug="$(basename "${task_file%.md}")"
    resume_task_session "$slug"
    sleep 3
    continue
  fi

  # ---- CASE 3: pick next task ----
  next_task="$(pick_next_task)"
  if [ -n "$next_task" ]; then
    slug="$(basename "${next_task%.md}")"
    start_agent_for_task "$slug" "$next_task"
    sleep 2
    continue
  fi

  echo "Idle — no tasks"
  sleep 10
done
