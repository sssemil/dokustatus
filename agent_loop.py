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


# =============================================================================
# Planning Phase Functions
# =============================================================================

def run_claude_planning(slug: str, version: int, feedback_content: str = ""):
    """Have Claude CLI create a versioned plan."""
    task_dir = TASKS_IN_PROGRESS / slug
    plan_file = task_dir / f"plan-v{version}.md"

    ticket_content = (task_dir / "ticket.md").read_text()

    if version == 1:
        prompt = f"""Create a detailed implementation plan for this task.

=== TICKET ===
{ticket_content}

=== YOUR TASK ===
Create a plan with:
- Summary of what needs to be done
- Step-by-step implementation approach
- Files to modify
- Testing approach
- Edge cases to handle

Output ONLY the plan content in markdown, no preamble."""
    else:
        prev_plan = (task_dir / f"plan-v{version - 1}.md").read_text() if (task_dir / f"plan-v{version - 1}.md").exists() else ""
        prompt = f"""Revise this implementation plan based on the feedback. This is revision {version}/3.

=== TICKET ===
{ticket_content}

=== PREVIOUS PLAN (v{version - 1}) ===
{prev_plan}

=== FEEDBACK ===
{feedback_content}

=== YOUR TASK ===
Create an improved plan addressing the feedback while keeping what works well.
Output ONLY the revised plan content in markdown, no preamble."""

    print(f"[PLANNING] Claude creating plan-v{version}.md for {slug}")

    result = subprocess.run(
        ["claude", "-p", prompt],
        capture_output=True,
        text=True,
        check=False
    )

    # Claude CLI may output to stdout or stderr depending on mode
    output = result.stdout.strip() or result.stderr.strip()

    if output:
        plan_file.write_text(output)
        print(f"[PLANNING] Plan written to {plan_file}")
    else:
        print(f"[PLANNING] ERROR: No output from Claude CLI")
        print(f"[PLANNING] stdout: {result.stdout[:200] if result.stdout else '(empty)'}")
        print(f"[PLANNING] stderr: {result.stderr[:200] if result.stderr else '(empty)'}")
        print(f"[PLANNING] returncode: {result.returncode}")
        plan_file.write_text(f"# Plan v{version}\n\nNo plan generated - check logs.")
        print(f"[PLANNING] Using placeholder")

    # Commit the plan
    run(["git", "add", str(task_dir)], check=False)
    run(["git", "commit", "-m", f"plan {slug}: create plan-v{version}.md"], check=False)


def run_codex_review(slug: str, iteration: int):
    """Have Codex review the plan and write versioned feedback."""
    task_dir = TASKS_IN_PROGRESS / slug
    plan_file = f"plan-v{iteration}.md"
    feedback_file = f"feedback-{iteration}.md"

    prompt = f"""Review the implementation plan for task: {slug}

Read ./workspace/tasks/in-progress/{slug}/ticket.md (the task)
Read ./workspace/tasks/in-progress/{slug}/{plan_file} (the plan)

Write feedback to ./workspace/tasks/in-progress/{slug}/{feedback_file} with:
- What's good about the plan
- What's missing or unclear
- Suggested improvements
- Any risks or concerns

Be specific and actionable. Focus on catching issues before implementation.
Then EXIT."""

    print(f"[PLANNING] Codex reviewing plan-v{iteration}.md for {slug}")

    run([
        "codex", "exec",
        "--dangerously-bypass-approvals-and-sandbox",
        "-C", ".",
        prompt
    ], check=False)

    # Commit the feedback
    run(["git", "add", str(task_dir)], check=False)
    run(["git", "commit", "-m", f"plan {slug}: feedback-{iteration}"], check=False)


def run_planning_phase(slug: str) -> bool:
    """
    Run the planning review loop.
    Returns True when planning is complete (3 iterations done).

    Iteration flow (Claude plans, Codex reviews):
    - iteration 0: Claude creates plan-v1.md
    - iteration 1: Codex reviews plan-v1 → feedback-1, Claude creates plan-v2.md
    - iteration 2: Codex reviews plan-v2 → feedback-2, Claude creates plan-v3.md
    - iteration 3: Codex reviews plan-v3 → feedback-3 (final review), copy plan-v3 → plan.md
    """
    task_dir = TASKS_IN_PROGRESS / slug
    planning_state = SESSIONS_DIR / f"{slug}.planning"

    # Get current iteration (0 = not started, 1-3 = in progress)
    iteration = 0
    if planning_state.exists():
        try:
            iteration = int(planning_state.read_text().strip())
        except ValueError:
            iteration = 0

    print(f"[PLANNING] Task {slug} at iteration {iteration}/3")

    # Iteration 0: Claude creates initial plan-v1
    if iteration == 0:
        plan_v1 = task_dir / "plan-v1.md"
        if not plan_v1.exists():
            run_claude_planning(slug, version=1)
        planning_state.write_text("1")
        return False

    # Iterations 1-2: Codex reviews, Claude revises
    if iteration < 3:
        # Codex reviews current plan
        run_codex_review(slug, iteration)

        # Read feedback for Claude to use in revision
        feedback_file = task_dir / f"feedback-{iteration}.md"
        feedback_content = feedback_file.read_text() if feedback_file.exists() else ""

        # Claude creates next plan version
        run_claude_planning(slug, version=iteration + 1, feedback_content=feedback_content)
        planning_state.write_text(str(iteration + 1))
        return False

    # Iteration 3: Final review by Codex, then copy to plan.md
    if iteration == 3:
        run_codex_review(slug, 3)  # Final feedback-3

        # Copy plan-v3.md to plan.md for execution
        plan_v3 = task_dir / "plan-v3.md"
        plan_final = task_dir / "plan.md"
        if plan_v3.exists():
            plan_final.write_text(plan_v3.read_text())
            run(["git", "add", str(plan_final)], check=False)
            run(["git", "commit", "-m", f"plan {slug}: finalize plan.md from plan-v3"], check=False)

        print(f"[PLANNING] Complete for {slug}")
        planning_state.unlink(missing_ok=True)
        return True

    return False


# =============================================================================
# Task Execution Functions
# =============================================================================

def setup_task_for_work(slug: str, task_dir: Path) -> bool:
    """
    Switch to task branch, move task to in-progress, and commit.
    Returns True if successful.
    """
    import shutil

    branch = f"task/{slug}"
    target_dir = TASKS_IN_PROGRESS / slug

    print(f"Setting up task {slug} on branch {branch}")

    # Stash any dirty changes before switching
    stashed = git_stash_if_dirty()

    # FIRST: create/switch to task branch
    if git_branch_exists(branch):
        if not git_switch(branch):
            print(f"Failed to switch to branch {branch}")
            if stashed:
                git_stash_pop()
            return False
    else:
        if not git_switch(branch, create=True):
            print(f"Failed to create branch {branch}")
            if stashed:
                git_stash_pop()
            return False

    # THEN: move task directory to in-progress and commit (on task branch)
    if task_dir.parent != TASKS_IN_PROGRESS:
        if target_dir.exists():
            shutil.rmtree(target_dir)
        run(["git", "mv", str(task_dir), str(target_dir)], check=False)
        run(["git", "commit", "-m", f"start task {slug}: todo → in-progress"], check=False)

    return True


def start_agent_for_task(slug: str, task_dir: Path):
    """Start a codex agent for execution (planning must be complete)."""
    session_file = session_file_for(slug)
    target_dir = TASKS_IN_PROGRESS / slug

    # Ensure task is set up
    if not target_dir.exists():
        if not setup_task_for_work(slug, task_dir):
            return

    prompt = f"""You are working on task: {slug}

Task directory: ./workspace/tasks/in-progress/{slug}/
- ticket.md: The task description
- plan.md: The implementation plan (already reviewed and finalized)

This branch is dedicated to this task: task/{slug}

The plan.md has been reviewed 3 times and is ready for implementation.
Follow the plan closely.

You are responsible for:
- Implementing according to plan.md
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

    print(f"[EXECUTION] Starting codex for {slug}")
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

    # Verify we are on the task branch - if not, something went wrong
    current = git_current_branch()
    if current != branch:
        print(f"ERROR: outbound task {slug} exists but we're on branch '{current}', not '{branch}'")
        print("This indicates the previous session didn't complete properly.")
        print("Manual intervention required: switch to the task branch and investigate.")
        return False

    # Verify state - check for directory with ticket.md
    if not outbound_dir.exists() or not (outbound_dir / "ticket.md").exists():
        print(f"ERROR: outbound directory missing for {slug} — refusing merge")
        return False

    # Stash any dirty changes before switching to main
    stashed = git_stash_if_dirty()

    # Switch to main for merge
    if not git_switch("main"):
        print("ERROR: Failed to switch to main branch")
        if stashed:
            git_stash_pop()
        return False

    run(["git", "pull", "--ff-only"], check=False)

    # Get all commits from task branch for the commit message body
    commits = run_capture([
        "git", "log", "--oneline", f"main..{branch}"
    ], check=False)

    result = subprocess.run(["git", "merge", "--squash", branch], check=False)
    if result.returncode != 0:
        print("Merge failed — leaving branch unmerged")
        if stashed:
            git_stash_pop()
        return False

    # Move directory to done before committing (so it's all in one commit)
    if done_dir.exists():
        shutil.rmtree(done_dir)
    run(["git", "mv", str(outbound_dir), str(done_dir)], check=False)

    # Single commit with squash merge + archive
    commit_msg = f"complete task {slug}\n\nCommits:\n{commits}" if commits else f"complete task {slug}"
    run(["git", "commit", "-m", commit_msg], check=False)

    # Cleanup session file and planning state
    if session_file.exists():
        session_file.unlink()
    planning_file = SESSIONS_DIR / f"{slug}.planning"
    if planning_file.exists():
        planning_file.unlink()

    # Force delete the task branch (squash merge means it won't be "fully merged")
    run(["git", "branch", "-D", branch], check=False)

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
    """Validate that the git branch and task state are consistent."""
    count = current_in_progress_count()
    current_branch = git_current_branch()

    if current_branch.startswith("task/"):
        slug = current_branch[5:]  # Remove "task/" prefix

        in_progress_dir = TASKS_IN_PROGRESS / slug
        outbound_dir = TASKS_OUTBOUND / slug

        # Valid states when on a task branch:
        # 1. Task is in in-progress (working on it)
        # 2. Task is in outbound (completed, ready to merge)
        in_progress = in_progress_dir.exists() and (in_progress_dir / "ticket.md").exists()
        in_outbound = outbound_dir.exists() and (outbound_dir / "ticket.md").exists()

        if not in_progress and not in_outbound:
            print(f"ERROR: on task branch {current_branch} but task not in in-progress or outbound")
            print("This is an inconsistent state — stop and fix manually.")
            sys.exit(1)


def is_task_in_planning(slug: str) -> bool:
    """Check if a task is currently in planning phase."""
    planning_file = SESSIONS_DIR / f"{slug}.planning"
    return planning_file.exists()


def is_planning_complete(slug: str) -> bool:
    """Check if planning is complete (plan.md exists, which is copied from plan-v3)."""
    planning_file = SESSIONS_DIR / f"{slug}.planning"
    task_dir = TASKS_IN_PROGRESS / slug
    plan_final = task_dir / "plan.md"

    if not planning_file.exists():
        # No planning file means either not started or complete
        # Complete if final plan.md exists
        return plan_final.exists()

    # If planning file exists, check iteration
    try:
        iteration = int(planning_file.read_text().strip())
        # Complete only after iteration 3 finishes and plan.md is created
        return iteration > 3 and plan_final.exists()
    except ValueError:
        return False


def main():
    setup_directories()

    while True:
        step_ts = datetime.now().isoformat()
        print(f"[{step_ts}] loop tick")

        validate_in_progress_state_or_die()

        # CASE 1: outbound task → merge
        outbound_dirs = []
        if TASKS_OUTBOUND.exists():
            outbound_dirs = [
                d for d in TASKS_OUTBOUND.iterdir()
                if d.is_dir() and (d / "ticket.md").exists()
            ]
        if outbound_dirs:
            slug = outbound_dirs[0].name
            merge_outbound_task(slug)
            time.sleep(2)
            continue

        # CASE 2: task in planning phase → continue planning
        planning_files = list(SESSIONS_DIR.glob("*.planning"))
        if planning_files:
            slug = planning_files[0].stem
            in_progress_dir = TASKS_IN_PROGRESS / slug
            todo_dir = TASKS_TODO / slug

            # Check if task is actually in in-progress
            if not (in_progress_dir.exists() and (in_progress_dir / "ticket.md").exists()):
                # Task not in in-progress - check if it's in todo
                if todo_dir.exists() and (todo_dir / "ticket.md").exists():
                    # Move it to in-progress first
                    print(f"[PLANNING] Task {slug} still in todo, setting up...")
                    if not setup_task_for_work(slug, todo_dir):
                        print(f"[PLANNING] Failed to setup task {slug}")
                        time.sleep(2)
                        continue
                else:
                    # Task doesn't exist anywhere - stale planning file
                    print(f"[PLANNING] Stale planning file for {slug}, cleaning up")
                    planning_files[0].unlink()
                    time.sleep(2)
                    continue

            # Ensure we're on the right branch
            branch = f"task/{slug}"
            if git_branch_exists(branch):
                git_switch(branch)

            if run_planning_phase(slug):
                # Planning complete, will start execution on next tick
                print(f"[PLANNING] Complete for {slug}, ready for execution")
            time.sleep(2)
            continue

        # CASE 3: task in progress (planning complete) → resume or execute
        if current_in_progress_count() == 1:
            task_dir = the_single_in_progress_task()
            if task_dir:
                slug = task_dir.name
                if is_planning_complete(slug):
                    # Planning done, execute
                    resume_task_session(slug)
                else:
                    # Need to start planning
                    planning_state = SESSIONS_DIR / f"{slug}.planning"
                    planning_state.write_text("0")
                time.sleep(3)
                continue

        # CASE 4: pick next task from todo
        next_task = pick_next_task()
        if next_task:
            slug = next_task.name  # Directory name is the slug

            # Set up task and start planning
            if setup_task_for_work(slug, next_task):
                planning_state = SESSIONS_DIR / f"{slug}.planning"
                planning_state.write_text("0")
                print(f"[NEW TASK] {slug} - starting planning phase")
            time.sleep(2)
            continue

        print("Idle — no tasks")
        time.sleep(10)


if __name__ == "__main__":
    main()
