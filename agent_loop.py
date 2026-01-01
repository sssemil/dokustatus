#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""
Agent loop that manages codex tasks through todo -> in-progress -> outbound -> done workflow.
"""

import subprocess
import sys
import time
from datetime import datetime
from pathlib import Path

WORKSPACE = Path("./workspace")
TASKS_TODO = WORKSPACE / "tasks" / "todo"
TASKS_IN_PROGRESS = WORKSPACE / "tasks" / "in-progress"
TASKS_OUTBOUND = WORKSPACE / "tasks" / "outbound"
TASKS_DONE = WORKSPACE / "tasks" / "done"
SESSIONS_DIR = WORKSPACE / "sessions"
LOGS_DIR = WORKSPACE / "logs"

# Task directory structure:
# workspace/tasks/todo/<slug>/ticket.md - the task description
# workspace/tasks/in-progress/<slug>/ticket.md - task being worked on
# workspace/tasks/in-progress/<slug>/plan.md - detailed plan written by agent


def setup_directories():
    """Create required directories if they don't exist."""
    for d in [TASKS_TODO, TASKS_IN_PROGRESS, TASKS_OUTBOUND, TASKS_DONE, SESSIONS_DIR, LOGS_DIR]:
        d.mkdir(parents=True, exist_ok=True)


def session_file_for(slug: str) -> Path:
    return SESSIONS_DIR / f"{slug}.session"


def pick_next_task() -> Path | None:
    """Get the next task directory from todo, sorted by name."""
    # Look for directories containing ticket.md
    task_dirs = sorted([
        d for d in TASKS_TODO.iterdir()
        if d.is_dir() and (d / "ticket.md").exists()
    ])
    return task_dirs[0] if task_dirs else None


def current_in_progress_count() -> int:
    """Count task directories in in-progress."""
    return len([
        d for d in TASKS_IN_PROGRESS.iterdir()
        if d.is_dir() and (d / "ticket.md").exists()
    ])


def the_single_in_progress_task() -> Path | None:
    """Get the single in-progress task directory."""
    task_dirs = [
        d for d in TASKS_IN_PROGRESS.iterdir()
        if d.is_dir() and (d / "ticket.md").exists()
    ]
    return task_dirs[0] if task_dirs else None


def run(cmd: list[str], check: bool = True) -> subprocess.CompletedProcess:
    """Run a command and return the result."""
    print(f"$ {' '.join(cmd)}")
    return subprocess.run(cmd, check=check)


def run_capture(cmd: list[str], check: bool = True) -> str:
    """Run a command and return stdout."""
    result = subprocess.run(cmd, capture_output=True, text=True, check=check)
    return result.stdout.strip()


def git_current_branch() -> str:
    try:
        return run_capture(["git", "branch", "--show-current"], check=False)
    except Exception:
        return ""


def git_branch_exists(branch: str) -> bool:
    result = subprocess.run(
        ["git", "rev-parse", "--verify", branch],
        capture_output=True,
        check=False
    )
    return result.returncode == 0


def git_switch(branch: str, create: bool = False) -> bool:
    """Switch to a git branch. Returns True if successful."""
    try:
        if create:
            run(["git", "switch", "-c", branch])
        else:
            run(["git", "switch", branch])
        return True
    except subprocess.CalledProcessError:
        return False


def git_stash_if_dirty() -> bool:
    """Stash changes if working directory is dirty. Returns True if stashed."""
    result = subprocess.run(
        ["git", "status", "--porcelain"],
        capture_output=True, text=True, check=False
    )
    if result.stdout.strip():
        print("Working directory dirty, stashing changes...")
        run(["git", "stash", "push", "-m", "agent_loop auto-stash"])
        return True
    return False


def git_stash_pop():
    """Pop the last stash."""
    run(["git", "stash", "pop"], check=False)


def start_agent_for_task(slug: str, task_dir: Path):
    """Start a codex agent for the given task directory."""
    import shutil

    branch = f"task/{slug}"
    session_file = session_file_for(slug)
    target_dir = TASKS_IN_PROGRESS / slug

    print(f"Starting agent for {slug} on branch {branch}")

    # Move task directory to in-progress if not already there
    if task_dir.parent != TASKS_IN_PROGRESS:
        if target_dir.exists():
            shutil.rmtree(target_dir)
        shutil.move(str(task_dir), str(target_dir))

    # Stash any dirty changes before switching
    stashed = git_stash_if_dirty()

    # Ensure branch exists and switch
    if git_branch_exists(branch):
        if not git_switch(branch):
            print(f"Failed to switch to branch {branch}")
            if stashed:
                git_stash_pop()
            return
    else:
        if not git_switch(branch, create=True):
            print(f"Failed to create branch {branch}")
            if stashed:
                git_stash_pop()
            return

    prompt = f"""You are working on task: {slug}

Task directory: ./workspace/tasks/in-progress/{slug}/
- ticket.md: The task description (read this first)
- plan.md: Your detailed implementation plan (create this)

This branch is dedicated to this task: task/{slug}

FIRST STEPS:
1. Read the ticket.md to understand the task
2. Create a detailed plan.md with your implementation approach
3. Append a History entry to ticket.md noting you've started

You are responsible for:
- Writing a detailed plan.md before making code changes
- Making bounded edits to the codebase
- Appending timestamped History entries to ticket.md
- Committing your own changes
- Optionally parallelizing subtasks (max 8 workers)

Completion protocol:

When the task is fully complete:
1) Append a final History entry to ticket.md describing completion
2) Move the entire task directory to:
   ./workspace/tasks/outbound/{slug}/
3) EXIT the session

Do NOT merge to main yourself.
The script will handle squash-merge once the directory
appears in outbound.

Stay focused on this task unless you explicitly
decide otherwise and justify it in History."""

    # Mark that a session exists
    session_file.touch()

    # Run codex
    run([
        "codex", "exec",
        "--dangerously-bypass-approvals-and-sandbox",
        "-C", ".",
        prompt
    ], check=False)


def merge_outbound_task(slug: str) -> bool:
    """Merge a completed task from outbound to done."""
    import shutil

    branch = f"task/{slug}"
    session_file = session_file_for(slug)
    outbound_dir = TASKS_OUTBOUND / slug
    done_dir = TASKS_DONE / slug

    print(f"Merging completed task: {slug} (branch {branch})")

    # Stash any dirty changes
    stashed = git_stash_if_dirty()

    # Ensure we are on the task branch
    if not git_switch(branch):
        print(f"Failed to switch to branch {branch}")
        if stashed:
            git_stash_pop()
        return False

    # Verify state - check for directory with ticket.md
    if not outbound_dir.exists() or not (outbound_dir / "ticket.md").exists():
        print(f"ERROR: outbound directory missing for {slug} — refusing merge")
        if stashed:
            git_stash_pop()
        return False

    # Merge to main
    if not git_switch("main"):
        git_switch("main", create=True)

    run(["git", "pull", "--ff-only"], check=False)

    result = subprocess.run(["git", "merge", "--squash", branch], check=False)
    if result.returncode != 0:
        print("Merge failed — leaving branch unmerged")
        if stashed:
            git_stash_pop()
        return False

    run(["git", "commit", "-m", f"complete task {slug} — squash merge"], check=False)

    # Move directory to done
    if done_dir.exists():
        shutil.rmtree(done_dir)
    shutil.move(str(outbound_dir), str(done_dir))

    # Cleanup session file
    if session_file.exists():
        session_file.unlink()

    # Pop stash if we stashed
    if stashed:
        git_stash_pop()

    print(f"Merged and archived: {slug}")
    return True


def resume_task_session(slug: str):
    """Resume or restart a task session."""
    session_file = session_file_for(slug)
    branch = f"task/{slug}"
    task_dir = TASKS_IN_PROGRESS / slug

    # Stash if dirty
    stashed = git_stash_if_dirty()

    # Ensure we're on the correct branch
    if git_branch_exists(branch):
        git_switch(branch)

    if not session_file.exists():
        print(f"No session file for {slug} — starting new agent")
        start_agent_for_task(slug, task_dir)
        return

    print(f"Resuming last session for task {slug}")

    # Resume the most recent session
    result = subprocess.run([
        "codex", "exec", "resume", "--last",
        f"Continue working on the task. Check ticket.md at ./workspace/tasks/in-progress/{slug}/ticket.md for current status."
    ], check=False)

    if result.returncode != 0:
        print("Resume failed, removing stale session file and starting fresh")
        session_file.unlink()
        start_agent_for_task(slug, task_dir)


def validate_in_progress_state_or_die():
    """Validate that the git branch and in-progress state are consistent."""
    count = current_in_progress_count()
    current_branch = git_current_branch()

    if current_branch.startswith("task/"):
        slug = current_branch[5:]  # Remove "task/" prefix

        if count != 1:
            print(f"ERROR: on task branch {current_branch} but {count} tasks in-progress")
            print("This is an inconsistent state — stop and fix manually.")
            sys.exit(1)

        expected_dir = TASKS_IN_PROGRESS / slug
        if not expected_dir.exists() or not (expected_dir / "ticket.md").exists():
            print(f"ERROR: branch {current_branch} does not match in-progress directory")
            sys.exit(1)


def main():
    setup_directories()

    while True:
        step_ts = datetime.now().isoformat()
        print(f"[{step_ts}] loop tick")

        validate_in_progress_state_or_die()

        # CASE 1: outbound task → merge
        outbound_dirs = [
            d for d in TASKS_OUTBOUND.iterdir()
            if d.is_dir() and (d / "ticket.md").exists()
        ]
        if outbound_dirs:
            slug = outbound_dirs[0].name
            merge_outbound_task(slug)
            time.sleep(2)
            continue

        # CASE 2: task in progress → resume or start
        if current_in_progress_count() == 1:
            task_dir = the_single_in_progress_task()
            if task_dir:
                slug = task_dir.name
                resume_task_session(slug)
                time.sleep(3)
                continue

        # CASE 3: pick next task
        next_task = pick_next_task()
        if next_task:
            slug = next_task.name  # Directory name is the slug
            start_agent_for_task(slug, next_task)
            time.sleep(2)
            continue

        print("Idle — no tasks")
        time.sleep(10)


if __name__ == "__main__":
    main()
