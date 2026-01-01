#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""
Agent loop that manages parallel task execution via git worktrees.

ARCHITECTURE OVERVIEW
=====================

Task Lifecycle:
  todo/ -> in-progress/ -> outbound/ -> done/

Each task runs in its own git worktree at ../worktrees/task-{slug}/ for isolation.

Phases per task:
  1. PLANNING  - Iterative planning with alternating agents:
       - Claude plan-v1 → Codex/Claude feedback-1
       - Claude plan-v2 → Codex/Claude feedback-2
       - Claude plan-v3 → Codex/Claude feedback-3 → plan.md (finalize)
  2. EXECUTING - Codex/Claude executes the finalized plan
  3. OUTBOUND  - Execution complete, queued for merge
  4. MERGING   - Claude agent rebases, resolves conflicts → squash merge → cleanup

Artifacts created in {task_dir}/:
  - plan-v1.md, plan-v2.md, plan-v3.md, plan.md  (plans)
  - feedback-1.md, feedback-2.md, feedback-3.md  (reviews)
  - agent_logs/claude-plan-v*.log                (planning logs)
  - agent_logs/{codex,claude}-review-*.log       (review logs)
  - agent_logs/{codex,claude}-exec-*.log         (execution logs)
  - agent_logs/claude-merge-*.log                (merge logs)

Rate Limit Fallback:
  - If Codex hits usage_limit_reached, task switches to Claude for remaining work
  - codex_rate_limited flag persists in .task-state across restarts
  - Affects both reviews (planning phase) and execution

Parallel Execution:
  - Up to N concurrent tasks (default 3, configurable via -j)
  - Priority queue consumed as slots open (./agent_loop.py 5 6 runs those first)
  - First-wins merge: tasks race, first to OUTBOUND merges first
  - fcntl exclusive lock prevents concurrent merges

Branch Handling on worktree creation:
  - No branch exists        → create from main
  - Branch at main          → reset to latest main
  - Branch ahead, no exec   → reset to main (incomplete planning)
  - Branch ahead, has exec  → continue work (preserve progress)

State & Locking Files:
  - .task-state             Task phase/iteration for crash recovery
  - .merge.lock             fcntl exclusive lock during merge
  - .merge-requested        Sentinel for merge freeze protocol
  - .needs-manual-rebase    Flag when rebase needs human intervention

Error Handling:
  - Crashed subprocesses auto-restart
  - Stale locks from dead processes are detected and cleaned
  - Rebase conflicts trigger manual intervention mode

Usage:
  ./agent_loop.py                 # default 3 concurrent
  ./agent_loop.py -j 5            # 5 concurrent tasks
  ./agent_loop.py -j 3 22 21 5    # priority queue: run 22, 21, 5 first
"""

import argparse
import fcntl
import os
import signal
import subprocess
import sys
import time
import threading
from collections import deque
from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
from pathlib import Path
from typing import Optional

WORKSPACE = Path("./workspace")
TASKS_TODO = WORKSPACE / "tasks" / "todo"
TASKS_IN_PROGRESS = WORKSPACE / "tasks" / "in-progress"
TASKS_OUTBOUND = WORKSPACE / "tasks" / "outbound"
TASKS_DONE = WORKSPACE / "tasks" / "done"
SESSIONS_DIR = WORKSPACE / "sessions"
LOGS_DIR = WORKSPACE / "logs"

# Parallel execution settings
WORKTREES_ROOT = Path("..") / "worktrees"
DEFAULT_MAX_CONCURRENT_TASKS = 3
MERGE_LOCK_FILE = WORKSPACE / ".merge.lock"

# =============================================================================
# tmux Single-Session Multi-Pane Configuration
# =============================================================================
AGENT_SESSION = "agent-loop"  # Single tmux session for all tasks
OVERSEER_CHECK_INTERVAL = 120  # Check every 2 minutes for stuck tasks
STUCK_TIMEOUT_MINUTES = 30  # Consider task stuck after 30 min no progress
MAX_RECOVERY_ATTEMPTS = 3  # Max times to try recovering a stuck task


# =============================================================================
# Data Structures for Parallel Task Management
# =============================================================================

class TaskPhase(Enum):
    """Task lifecycle phases."""
    PLANNING = "planning"
    EXECUTING = "executing"
    OUTBOUND = "outbound"
    MERGING = "merging"


class TaskState(Enum):
    """State within a phase for tmux-based execution."""
    IDLE = "idle"  # Not running, waiting for next command
    RUNNING = "running"  # Command executing in pane
    AWAITING_CHECK = "awaiting_check"  # Command done, needs result check
    RECOVERING = "recovering"  # Overseer is fixing stuck task
    COMPLETE = "complete"  # Task finished successfully
    FAILED = "failed"  # Task failed, needs manual intervention


@dataclass
class ActiveTask:
    """Represents a task being worked on in a worktree."""
    slug: str
    worktree_path: Path
    branch: str
    phase: TaskPhase
    # tmux pane tracking (replaces subprocess tracking)
    pane_id: str = ""  # e.g., "%3" - unique pane identifier
    state: TaskState = field(default=TaskState.IDLE)
    current_step: str = ""  # Current operation, e.g., "plan_v1", "review_2", "execute"
    last_command_time: Optional[datetime] = None
    recovery_attempts: int = 0
    # Legacy subprocess fields (still used during transition)
    process: Optional[subprocess.Popen] = None
    planning_iteration: int = 0
    log_handle: Optional[object] = field(default=None, repr=False)
    output_thread: Optional[threading.Thread] = field(default=None, repr=False)
    codex_rate_limited: bool = False  # Fallback to Claude for all Codex operations

    @property
    def is_alive(self) -> bool:
        """Check if the subprocess is still running (legacy) or pane is alive."""
        if self.pane_id:
            return is_pane_alive(self.pane_id)
        if self.process is None:
            return False
        return self.process.poll() is None

    def worktree_task_dir(self, status: str = "in-progress") -> Path:
        """Get task directory path within the worktree."""
        return self.worktree_path / "workspace" / "tasks" / status / self.slug


@dataclass
class ParallelTaskManager:
    """Manages multiple concurrent tasks."""
    max_tasks: int = DEFAULT_MAX_CONCURRENT_TASKS
    priority_queue: deque = field(default_factory=deque)  # Queue of priority task patterns to pop from
    active_tasks: dict[str, ActiveTask] = field(default_factory=dict)
    merge_queue: list[str] = field(default_factory=list)

    def can_start_new_task(self) -> bool:
        """Check if we can start another task."""
        return len(self.active_tasks) < self.max_tasks

    def get_task(self, slug: str) -> Optional[ActiveTask]:
        """Get a task by slug."""
        return self.active_tasks.get(slug)

    def add_task(self, task: ActiveTask):
        """Add a task to the manager."""
        self.active_tasks[task.slug] = task

    def remove_task(self, slug: str):
        """Remove a task from the manager."""
        if slug in self.active_tasks:
            del self.active_tasks[slug]
        if slug in self.merge_queue:
            self.merge_queue.remove(slug)

    def queue_for_merge(self, slug: str):
        """Add a task to the merge queue."""
        if slug not in self.merge_queue:
            self.merge_queue.append(slug)

    def next_to_merge(self) -> Optional[str]:
        """Get the next task slug to merge."""
        return self.merge_queue[0] if self.merge_queue else None

    def get_running_tasks(self) -> list[ActiveTask]:
        """Get all tasks with running subprocesses."""
        return [t for t in self.active_tasks.values() if t.is_alive]

    def get_tasks_in_phase(self, phase: TaskPhase) -> list[ActiveTask]:
        """Get all tasks in a specific phase."""
        return [t for t in self.active_tasks.values() if t.phase == phase]


# Task directory structure:
# workspace/tasks/todo/<slug>/ticket.md - the task description
# workspace/tasks/in-progress/<slug>/ticket.md - task being worked on
# workspace/tasks/in-progress/<slug>/plan.md - detailed plan written by agent


# =============================================================================
# tmux Pane Management Functions
# =============================================================================

def check_tmux_installed() -> bool:
    """Check if tmux is installed."""
    result = subprocess.run(["which", "tmux"], capture_output=True, check=False)
    if result.returncode != 0:
        print("[ERROR] tmux is not installed. Please install tmux first.")
        sys.exit(1)
    return True


def ensure_agent_session():
    """Create the agent-loop session if it doesn't exist."""
    result = subprocess.run(
        ["tmux", "has-session", "-t", AGENT_SESSION],
        capture_output=True, check=False
    )
    if result.returncode != 0:
        # Create new session with a placeholder window
        subprocess.run([
            "tmux", "new-session", "-d", "-s", AGENT_SESSION, "-n", "main"
        ], check=False)
        print(f"[TMUX] Created session: {AGENT_SESSION}")
        print(f"[TMUX] Attach with: tmux attach -t {AGENT_SESSION}")


def create_task_pane(slug: str, worktree_path: Path) -> str:
    """
    Create a new pane for a task in the agent-loop session.
    Returns the pane ID (e.g., '%3') or empty string on failure.
    """
    # Split current window to create new pane
    result = subprocess.run([
        "tmux", "split-window", "-t", AGENT_SESSION,
        "-c", str(worktree_path),
        "-P", "-F", "#{pane_id}"  # Print pane ID
    ], capture_output=True, text=True, check=False)

    if result.returncode != 0:
        print(f"[TMUX] Failed to create pane for {slug}: {result.stderr}")
        return ""

    pane_id = result.stdout.strip()

    # Rebalance layout to tiled (show all panes equally)
    subprocess.run(
        ["tmux", "select-layout", "-t", AGENT_SESSION, "tiled"],
        capture_output=True, check=False
    )

    # Set pane title for identification
    subprocess.run([
        "tmux", "select-pane", "-t", f"{AGENT_SESSION}:{pane_id}",
        "-T", slug
    ], capture_output=True, check=False)

    print(f"[TMUX] Created pane {pane_id} for {slug}")
    return pane_id


def send_to_pane(pane_id: str, command: str) -> bool:
    """
    Send a command to a specific pane.
    Returns True on success.
    """
    target = f"{AGENT_SESSION}:{pane_id}"
    result = subprocess.run([
        "tmux", "send-keys", "-t", target, command, "Enter"
    ], capture_output=True, check=False)
    return result.returncode == 0


def send_long_command_to_pane(pane_id: str, command: str) -> bool:
    """
    Send a long command using load-buffer + paste-buffer to avoid truncation.
    Returns True on success.
    """
    import tempfile
    target = f"{AGENT_SESSION}:{pane_id}"

    # Write command to temp file
    with tempfile.NamedTemporaryFile(mode='w', suffix='.sh', delete=False) as f:
        f.write(command)
        temp_path = f.name

    try:
        # Load into tmux buffer
        result = subprocess.run([
            "tmux", "load-buffer", temp_path
        ], capture_output=True, check=False)

        if result.returncode != 0:
            return False

        # Paste buffer into pane
        result = subprocess.run([
            "tmux", "paste-buffer", "-t", target
        ], capture_output=True, check=False)

        if result.returncode != 0:
            return False

        # Send Enter to execute
        subprocess.run([
            "tmux", "send-keys", "-t", target, "Enter"
        ], capture_output=True, check=False)

        return True
    finally:
        Path(temp_path).unlink(missing_ok=True)


def capture_pane_output(pane_id: str, lines: int = 100) -> str:
    """Capture recent output from a pane."""
    target = f"{AGENT_SESSION}:{pane_id}"
    result = subprocess.run([
        "tmux", "capture-pane", "-t", target, "-p", "-S", f"-{lines}"
    ], capture_output=True, text=True, check=False)
    return result.stdout if result.returncode == 0 else ""


def is_pane_alive(pane_id: str) -> bool:
    """Check if a pane still exists."""
    if not pane_id:
        return False
    result = subprocess.run([
        "tmux", "list-panes", "-t", AGENT_SESSION, "-F", "#{pane_id}"
    ], capture_output=True, text=True, check=False)
    return pane_id in result.stdout.split('\n')


def kill_pane(pane_id: str):
    """Kill a pane."""
    if pane_id:
        subprocess.run([
            "tmux", "kill-pane", "-t", f"{AGENT_SESSION}:{pane_id}"
        ], capture_output=True, check=False)


def list_session_panes() -> list[dict]:
    """List all panes in the agent session with their info."""
    result = subprocess.run([
        "tmux", "list-panes", "-t", AGENT_SESSION,
        "-F", "#{pane_id}|#{pane_current_path}|#{pane_title}"
    ], capture_output=True, text=True, check=False)

    panes = []
    if result.returncode == 0:
        for line in result.stdout.strip().split('\n'):
            if '|' in line:
                parts = line.split('|')
                panes.append({
                    'pane_id': parts[0],
                    'path': parts[1] if len(parts) > 1 else '',
                    'title': parts[2] if len(parts) > 2 else ''
                })
    return panes


def escape_for_shell(text: str) -> str:
    """Escape text for safe shell interpolation."""
    return text.replace("'", "'\\''")


def setup_directories():
    """Create required directories if they don't exist."""
    for d in [TASKS_TODO, TASKS_IN_PROGRESS, TASKS_OUTBOUND, TASKS_DONE, SESSIONS_DIR, LOGS_DIR]:
        d.mkdir(parents=True, exist_ok=True)
    # Also ensure worktrees root exists
    WORKTREES_ROOT.mkdir(parents=True, exist_ok=True)


# =============================================================================
# Worktree Management Functions
# =============================================================================

def sanitize_slug(slug: str) -> str:
    """Sanitize a slug to prevent path traversal attacks."""
    # Remove any path separators and dangerous characters
    sanitized = slug.replace("/", "-").replace("\\", "-").replace("..", "-")
    # Only allow alphanumeric, dash, underscore
    return "".join(c for c in sanitized if c.isalnum() or c in "-_")


def create_worktree(slug: str) -> Optional[Path]:
    """
    Create a git worktree for a task.
    Returns the worktree path or None on failure.

    If branch already exists with commits, continues from that state.
    If branch doesn't exist, creates it from main.
    """
    import shutil

    slug = sanitize_slug(slug)
    branch = f"task/{slug}"
    worktree_path = (WORKTREES_ROOT / f"task-{slug}").resolve()

    # Ensure parent directory exists
    WORKTREES_ROOT.mkdir(parents=True, exist_ok=True)

    # Always prune stale worktree metadata first
    subprocess.run(["git", "worktree", "prune"], capture_output=True, check=False)

    # Remove stale worktree directory if exists
    if worktree_path.exists():
        print(f"[WORKTREE] Removing stale worktree dir: {worktree_path}")
        # Try git remove first
        subprocess.run(
            ["git", "worktree", "remove", "--force", str(worktree_path)],
            capture_output=True, check=False
        )
        # Fallback to rm if still exists
        if worktree_path.exists():
            shutil.rmtree(worktree_path, ignore_errors=True)
        subprocess.run(["git", "worktree", "prune"], capture_output=True, check=False)

    # Check if branch exists
    branch_exists = git_branch_exists(branch)

    if branch_exists:
        # Check if branch has commits ahead of main
        ahead = subprocess.run(
            ["git", "rev-list", "--count", f"main..{branch}"],
            capture_output=True, text=True, check=False
        )
        ahead_count = int(ahead.stdout.strip()) if ahead.returncode == 0 else 0

        if ahead_count > 0:
            # Check if planning was completed by looking for execution logs
            exec_log_path = f"workspace/tasks/in-progress/{slug}/agent_logs/claude-execute.log"
            has_exec_log = subprocess.run(
                ["git", "cat-file", "-e", f"{branch}:{exec_log_path}"],
                capture_output=True, check=False
            ).returncode == 0

            if has_exec_log:
                print(f"[WORKTREE] Branch {branch} has execution in progress, continuing work")
            else:
                # Planning not completed - nuke and start fresh
                print(f"[WORKTREE] Branch {branch} has incomplete planning ({ahead_count} commits), resetting to main")
                subprocess.run(
                    ["git", "branch", "-f", branch, "main"],
                    capture_output=True, check=False
                )
        else:
            print(f"[WORKTREE] Branch {branch} exists at main, resetting to latest main")
            subprocess.run(
                ["git", "branch", "-f", branch, "main"],
                capture_output=True, check=False
            )
    else:
        # Create branch from main
        result = subprocess.run(
            ["git", "branch", branch, "main"],
            capture_output=True, check=False
        )
        if result.returncode != 0:
            print(f"[WORKTREE] Failed to create branch {branch}")
            return None
        print(f"[WORKTREE] Created new branch {branch} from main")

    # Create the worktree
    result = subprocess.run(
        ["git", "worktree", "add", str(worktree_path), branch],
        capture_output=True, text=True, check=False
    )

    if result.returncode != 0:
        print(f"[WORKTREE] Failed to create worktree: {result.stderr}")
        return None

    print(f"[WORKTREE] Created: {worktree_path} on branch {branch}")
    return worktree_path


def cleanup_worktree(slug: str) -> bool:
    """
    Remove a worktree and optionally its branch.
    Returns True if successful.
    """
    import shutil

    slug = sanitize_slug(slug)
    worktree_path = (WORKTREES_ROOT / f"task-{slug}").resolve()

    # Safety check: verify path matches expected pattern
    if not str(worktree_path).startswith(str(WORKTREES_ROOT.resolve())):
        print(f"[WORKTREE] Safety check failed: {worktree_path} not under {WORKTREES_ROOT}")
        return False

    # Remove worktree via git (handles git metadata)
    result = subprocess.run(
        ["git", "worktree", "remove", "--force", str(worktree_path)],
        capture_output=True, check=False
    )

    if result.returncode != 0:
        # Fallback: manually remove if git command fails
        if worktree_path.exists():
            shutil.rmtree(worktree_path, ignore_errors=True)
        # Prune stale worktree entries
        subprocess.run(["git", "worktree", "prune"], check=False)

    print(f"[WORKTREE] Cleaned up: {worktree_path}")
    return True


def list_worktrees() -> list[dict]:
    """
    List all git worktrees with their paths and branches.
    Returns list of {path, branch, head} dicts.
    """
    result = subprocess.run(
        ["git", "worktree", "list", "--porcelain"],
        capture_output=True, text=True, check=False
    )

    worktrees = []
    current: dict = {}

    for line in result.stdout.strip().split('\n'):
        if not line:
            continue
        if line.startswith('worktree '):
            if current:
                worktrees.append(current)
            current = {'path': line[9:]}
        elif line.startswith('HEAD '):
            current['head'] = line[5:]
        elif line.startswith('branch '):
            current['branch'] = line[7:]

    if current:
        worktrees.append(current)

    return worktrees


def is_worktree_healthy(worktree_path: Path) -> bool:
    """Check if a worktree exists and has a valid git state."""
    if not worktree_path.exists():
        return False

    git_file = worktree_path / ".git"
    if not git_file.exists():
        return False

    # Check git status works
    result = subprocess.run(
        ["git", "-C", str(worktree_path), "status", "--porcelain"],
        capture_output=True, check=False
    )
    return result.returncode == 0


def get_worktree_for_slug(slug: str) -> Optional[Path]:
    """Get the worktree path for a task slug, if it exists."""
    slug = sanitize_slug(slug)
    worktree_path = (WORKTREES_ROOT / f"task-{slug}").resolve()
    if is_worktree_healthy(worktree_path):
        return worktree_path
    return None


# =============================================================================
# Non-Blocking Subprocess Execution
# =============================================================================

def drain_output(process: subprocess.Popen, log_file: Path, label: str):
    """
    Drain stdout/stderr from a process to a log file.
    Runs in a separate thread to prevent pipe deadlock.
    """
    log_file.parent.mkdir(parents=True, exist_ok=True)

    try:
        with open(log_file, "w") as f:
            if process.stdout:
                for line in process.stdout:
                    line = line.rstrip('\n')
                    f.write(line + "\n")
                    f.flush()
    except Exception as e:
        print(f"[{label}] Output drain error: {e}")


def start_agent_subprocess(
    task: ActiveTask,
    cmd: list[str],
    log_file: Path,
    label: str,
    input_text: Optional[str] = None
) -> subprocess.Popen:
    """
    Start an agent subprocess without blocking.
    Returns the Popen object for tracking.
    """
    log_file.parent.mkdir(parents=True, exist_ok=True)

    # Force non-TTY environment
    env = os.environ.copy()
    env.pop("TERM", None)

    print(f"[{label}] Starting in {task.worktree_path}, logging to {log_file}")

    process = subprocess.Popen(
        cmd,
        stdin=subprocess.PIPE if input_text else None,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        bufsize=1,
        cwd=str(task.worktree_path),  # Run in worktree!
        env=env
    )

    if input_text and process.stdin:
        process.stdin.write(input_text)
        process.stdin.close()

    # Start output drain thread to prevent pipe deadlock
    output_thread = threading.Thread(
        target=drain_output,
        args=(process, log_file, label),
        daemon=True
    )
    output_thread.start()

    task.process = process
    task.output_thread = output_thread

    return process


def check_task_subprocess(task: ActiveTask) -> tuple[bool, Optional[int]]:
    """
    Check if a task's subprocess is still running.
    Returns (is_running, exit_code_if_finished).
    """
    if task.process is None:
        return (False, None)

    exit_code = task.process.poll()
    if exit_code is None:
        return (True, None)
    else:
        return (False, exit_code)


def kill_task_subprocess(task: ActiveTask, timeout: int = 30):
    """Gracefully terminate a task's subprocess."""
    if task.process is None:
        return

    if task.is_alive:
        print(f"[KILL] Terminating task {task.slug} (PID {task.process.pid})")
        task.process.terminate()
        try:
            task.process.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            print(f"[KILL] Force killing task {task.slug}")
            task.process.kill()
            task.process.wait()


# =============================================================================
# State Persistence
# =============================================================================

def persist_task_state(task: ActiveTask):
    """Write task state to worktree for crash recovery."""
    state_file = task.worktree_path / ".task-state"
    rate_limited = "1" if task.codex_rate_limited else "0"
    state_file.write_text(f"{task.phase.value}\n{task.planning_iteration}\n{rate_limited}\n")


def load_task_state(worktree_path: Path) -> tuple[TaskPhase, int, bool]:
    """Load task state from worktree. Returns (phase, iteration, codex_rate_limited)."""
    state_file = worktree_path / ".task-state"
    if state_file.exists():
        try:
            lines = state_file.read_text().strip().split("\n")
            phase = TaskPhase(lines[0]) if lines else TaskPhase.PLANNING
            iteration = int(lines[1]) if len(lines) > 1 else 0
            rate_limited = lines[2] == "1" if len(lines) > 2 else False
            return phase, iteration, rate_limited
        except (ValueError, IndexError):
            pass
    return TaskPhase.PLANNING, 0, False


# =============================================================================
# tmux-Based Command Functions
# =============================================================================

def send_planning_command_tmux(task: ActiveTask, version: int):
    """Send Claude planning command to task's tmux pane."""
    step = f"plan_v{version}"
    plan_file = f"plan-v{version}.md"
    log_dir = task.worktree_task_dir() / "agent_logs"
    log_file = log_dir / f"claude-plan-v{version}.log"
    log_dir.mkdir(parents=True, exist_ok=True)

    if version == 1:
        prompt = f"""Create a detailed implementation plan for task: {task.slug}

Read the ticket at ./workspace/tasks/in-progress/{task.slug}/ticket.md

Explore the codebase to understand:
- Current implementation patterns
- Files that will need modification
- Testing patterns used

Write a detailed plan to ./workspace/tasks/in-progress/{task.slug}/{plan_file} with:
- Summary of what needs to be done
- Step-by-step implementation approach
- Specific files to modify (with paths)
- Testing approach
- Edge cases to handle

Then stop."""
    else:
        prompt = f"""Revise the implementation plan for task: {task.slug}. This is revision {version}/3.

Read:
- ./workspace/tasks/in-progress/{task.slug}/ticket.md (the task)
- ./workspace/tasks/in-progress/{task.slug}/plan-v{version - 1}.md (previous plan)
- ./workspace/tasks/in-progress/{task.slug}/feedback-{version - 1}.md (feedback to address)

Create an improved plan at ./workspace/tasks/in-progress/{task.slug}/{plan_file}
Address the feedback while keeping what works well.

Then stop."""

    escaped = escape_for_shell(prompt)
    # Build command with completion marker and exit code capture
    cmd = f"claude -p '{escaped}' --allowedTools Read,Write,Glob,Grep,Edit,Bash 2>&1 | tee {log_file}; echo '__DONE_{step}_EXIT_'$?"

    task.current_step = step
    task.state = TaskState.RUNNING
    task.last_command_time = datetime.now()
    persist_task_state(task)

    # Use load-buffer for long prompts
    send_long_command_to_pane(task.pane_id, cmd)
    print(f"[TMUX] Sent {step} command to pane {task.pane_id} for {task.slug}")


def send_review_command_tmux(task: ActiveTask, iteration: int):
    """Send review command to task's tmux pane."""
    step = f"review_{iteration}"
    log_dir = task.worktree_task_dir() / "agent_logs"

    # Determine which agent to use
    use_claude = task.codex_rate_limited
    agent_name = "claude" if use_claude else "codex"
    log_file = log_dir / f"{agent_name}-review-{iteration}.log"
    log_dir.mkdir(parents=True, exist_ok=True)

    prompt = f"""Review the implementation plan for task: {task.slug}

Read ./workspace/tasks/in-progress/{task.slug}/ticket.md (the task)
Read ./workspace/tasks/in-progress/{task.slug}/plan-v{iteration}.md (the plan)

Write feedback to ./workspace/tasks/in-progress/{task.slug}/feedback-{iteration}.md with:
- What's good about the plan
- What's missing or unclear
- Suggested improvements
- Any risks or concerns

Be specific and actionable. Focus on catching issues before implementation.
Then EXIT."""

    escaped = escape_for_shell(prompt)

    if use_claude:
        cmd = f"claude -p '{escaped}' --allowedTools Edit,Write,Bash,Glob,Grep,Read 2>&1 | tee {log_file}; echo '__DONE_{step}_EXIT_'$?"
    else:
        cmd = f"codex exec --dangerously-bypass-approvals-and-sandbox -C . '{escaped}' 2>&1 | tee {log_file}; echo '__DONE_{step}_EXIT_'$?"

    task.current_step = step
    task.state = TaskState.RUNNING
    task.last_command_time = datetime.now()
    persist_task_state(task)

    send_long_command_to_pane(task.pane_id, cmd)
    print(f"[TMUX] Sent {step} command to pane {task.pane_id} for {task.slug}")


def send_execution_command_tmux(task: ActiveTask):
    """Send execution command to task's tmux pane."""
    step = "execute"
    timestamp = datetime.now().strftime("%Y%m%d-%H%M%S")
    log_dir = task.worktree_task_dir() / "agent_logs"

    # Determine which agent to use
    use_claude = task.codex_rate_limited
    agent_name = "claude" if use_claude else "codex"
    log_file = log_dir / f"{agent_name}-exec-{timestamp}.log"
    log_dir.mkdir(parents=True, exist_ok=True)

    prompt = f"""You are working on task: {task.slug}

Task directory: ./workspace/tasks/in-progress/{task.slug}/
- ticket.md: The task description
- plan.md: The implementation plan (already reviewed and finalized)

This branch is dedicated to this task: task/{task.slug}
You are running in an isolated worktree at {task.worktree_path}

The plan.md has been reviewed 3 times and is ready for implementation.
Follow the plan closely.

IMPORTANT: Check for .merge-requested file periodically. If it exists, commit your
work immediately and stop - the orchestrator needs to merge.

You are responsible for:
- Implementing according to plan.md
- Making bounded edits to the codebase
- Appending timestamped History entries to ticket.md
- Committing your own changes

Completion protocol:

When the task is fully complete:
1) Append a final History entry to ticket.md describing completion
2) Move the entire task directory to:
   ./workspace/tasks/outbound/{task.slug}/
3) EXIT the session

Do NOT merge to main yourself.
The script will handle squash-merge once the directory
appears in outbound.

Stay focused on this task."""

    escaped = escape_for_shell(prompt)

    if use_claude:
        cmd = f"claude -p '{escaped}' --allowedTools Edit,Write,Bash,Glob,Grep,Read 2>&1 | tee {log_file}; echo '__DONE_{step}_EXIT_'$?"
    else:
        cmd = f"codex exec --dangerously-bypass-approvals-and-sandbox -C . '{escaped}' 2>&1 | tee {log_file}; echo '__DONE_{step}_EXIT_'$?"

    task.phase = TaskPhase.EXECUTING
    task.current_step = step
    task.state = TaskState.RUNNING
    task.last_command_time = datetime.now()
    persist_task_state(task)

    send_long_command_to_pane(task.pane_id, cmd)
    print(f"[TMUX] Sent {step} command to pane {task.pane_id} for {task.slug}")


def send_merge_command_tmux(task: ActiveTask):
    """Send merge agent command to task's tmux pane."""
    step = "merge"
    timestamp = datetime.now().strftime("%Y%m%d-%H%M%S")
    log_dir = task.worktree_task_dir("outbound") / "agent_logs"
    log_file = log_dir / f"claude-merge-{timestamp}.log"
    log_dir.mkdir(parents=True, exist_ok=True)

    prompt = f"""You are the MERGE AGENT for task: {task.slug}

CONTEXT:
- Task branch: {task.branch}
- Worktree: {task.worktree_path}
- Task completed and is in: ./workspace/tasks/outbound/{task.slug}/

YOUR MISSION - Rebase this branch onto main and resolve any conflicts:

STEP 1: Fetch and rebase
git fetch origin main
git rebase origin/main

STEP 2: If conflicts occur during rebase:
- Read the conflicting files to understand both sides
- Resolve conflicts intelligently:
  - Keep BOTH changes when they don't overlap
  - For overlapping changes, understand the intent and merge logically
  - Remove conflict markers (<<<<<<, =======, >>>>>>>)
- Stage resolved files: git add <file>
- Continue rebase: git rebase --continue
- Repeat until rebase completes

STEP 3: After successful rebase, signal completion:
# Move task to done
mv ./workspace/tasks/outbound/{task.slug} ./workspace/tasks/done/{task.slug}

# Commit the move
git add -A
git commit -m "merge {task.slug}: rebase complete, ready for squash"

STEP 4: EXIT the session

IF CONFLICTS CANNOT BE RESOLVED:
git rebase --abort
touch .needs-manual-rebase
Then EXIT - a human will need to resolve this.

IMPORTANT RULES:
- Work ONLY in this worktree
- Do NOT push to origin
- Do NOT merge to main yourself
- The orchestrator handles the final squash merge after you succeed
"""

    escaped = escape_for_shell(prompt)
    cmd = f"claude -p '{escaped}' --allowedTools Edit,Write,Bash,Glob,Grep,Read 2>&1 | tee {log_file}; echo '__DONE_{step}_EXIT_'$?"

    task.phase = TaskPhase.MERGING
    task.current_step = step
    task.state = TaskState.RUNNING
    task.last_command_time = datetime.now()
    persist_task_state(task)

    send_long_command_to_pane(task.pane_id, cmd)
    print(f"[TMUX] Sent {step} command to pane {task.pane_id} for {task.slug}")


def check_step_completion_tmux(task: ActiveTask) -> tuple[bool, Optional[int]]:
    """
    Check if the current step completed by looking for completion marker in pane output.
    Returns (completed, exit_code) where exit_code is None if not completed.
    """
    if not task.pane_id or not task.current_step:
        return (False, None)

    output = capture_pane_output(task.pane_id, lines=50)
    marker = f"__DONE_{task.current_step}_EXIT_"

    for line in output.split('\n'):
        if marker in line:
            # Extract exit code from marker
            try:
                exit_code = int(line.split(marker)[1].strip())
                return (True, exit_code)
            except (ValueError, IndexError):
                return (True, 0)

    return (False, None)


# =============================================================================
# Merge Freeze Protocol
# =============================================================================

def request_merge_freeze(task: ActiveTask):
    """Signal agent to stop gracefully by touching .merge-requested file."""
    freeze_file = task.worktree_path / ".merge-requested"
    freeze_file.touch()
    print(f"[FREEZE] Requested merge freeze for {task.slug}")


def wait_for_clean_worktree(task: ActiveTask, timeout: int = 60) -> bool:
    """Wait for agent to exit and worktree to be clean."""
    deadline = time.time() + timeout

    while time.time() < deadline:
        if not task.is_alive:
            # Verify clean working tree
            result = subprocess.run(
                ["git", "-C", str(task.worktree_path), "status", "--porcelain"],
                capture_output=True, text=True
            )
            if not result.stdout.strip():
                return True
            # Worktree dirty - commit any pending changes
            subprocess.run(
                ["git", "-C", str(task.worktree_path), "add", "-A"],
                check=False
            )
            subprocess.run(
                ["git", "-C", str(task.worktree_path), "commit", "-m", "auto-commit before merge"],
                check=False
            )
            return True
        time.sleep(2)

    print(f"[FREEZE] Timeout waiting for {task.slug} to stop")
    return False


# =============================================================================
# Merge Lock Management
# =============================================================================

def acquire_merge_lock() -> Optional[int]:
    """
    Acquire exclusive merge lock.
    Returns file descriptor or None if lock unavailable.
    """
    MERGE_LOCK_FILE.parent.mkdir(parents=True, exist_ok=True)

    try:
        fd = os.open(str(MERGE_LOCK_FILE), os.O_CREAT | os.O_RDWR)
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        # Write PID for stale lock detection
        os.write(fd, str(os.getpid()).encode())
        return fd
    except (OSError, IOError):
        return None


def release_merge_lock(fd: int):
    """Release the merge lock."""
    try:
        fcntl.flock(fd, fcntl.LOCK_UN)
        os.close(fd)
    except (OSError, IOError):
        pass


def check_stale_merge_lock() -> bool:
    """Check if merge lock is stale (held by dead process) and clean up if so."""
    if not MERGE_LOCK_FILE.exists():
        return False

    try:
        with open(MERGE_LOCK_FILE, 'r') as f:
            pid_str = f.read().strip()
            if pid_str:
                pid = int(pid_str)
                # Check if process is still alive
                try:
                    os.kill(pid, 0)  # Signal 0 just checks existence
                    return False  # Process still alive
                except OSError:
                    # Process dead, lock is stale
                    print(f"[LOCK] Removing stale merge lock from dead PID {pid}")
                    MERGE_LOCK_FILE.unlink()
                    return True
    except (ValueError, OSError):
        pass
    return False


# =============================================================================
# Worktree-Aware Task Setup
# =============================================================================

def setup_task_in_worktree(slug: str, manager: ParallelTaskManager) -> Optional[ActiveTask]:
    """
    Set up a task to run in its own worktree.
    Creates worktree, moves task to in-progress, returns ActiveTask.
    """
    import shutil

    slug = sanitize_slug(slug)
    todo_dir = TASKS_TODO / slug

    # Verify task exists in todo
    if not todo_dir.exists() or not (todo_dir / "ticket.md").exists():
        print(f"[SETUP] Task {slug} not found in todo")
        return None

    # Create the worktree
    worktree_path = create_worktree(slug)
    if not worktree_path:
        return None

    branch = f"task/{slug}"

    # In the worktree, the task should still be in todo (same as main)
    # Move it from todo to in-progress within the worktree
    worktree_todo = worktree_path / "workspace" / "tasks" / "todo" / slug
    worktree_in_progress = worktree_path / "workspace" / "tasks" / "in-progress" / slug

    # Ensure in-progress dir exists
    (worktree_path / "workspace" / "tasks" / "in-progress").mkdir(parents=True, exist_ok=True)

    # Move task directory in worktree
    if worktree_todo.exists():
        shutil.move(str(worktree_todo), str(worktree_in_progress))

        # Commit the move in the worktree
        subprocess.run(
            ["git", "-C", str(worktree_path), "add", "-A"],
            check=False
        )
        subprocess.run(
            ["git", "-C", str(worktree_path), "commit", "-m", f"start task {slug}: todo -> in-progress"],
            check=False
        )

    # Create tmux pane for this task
    pane_id = create_task_pane(slug, worktree_path)
    if not pane_id:
        print(f"[SETUP] Failed to create tmux pane for {slug}")
        return None

    # Create ActiveTask
    task = ActiveTask(
        slug=slug,
        worktree_path=worktree_path,
        branch=branch,
        phase=TaskPhase.PLANNING,
        pane_id=pane_id,
        state=TaskState.IDLE
    )

    manager.add_task(task)
    persist_task_state(task)
    print(f"[SETUP] Task {slug} ready in worktree {worktree_path}, pane {pane_id}")

    return task


# =============================================================================
# Safe Rebase Before Merge
# =============================================================================

MAX_REBASE_ATTEMPTS = 3


def rebase_before_merge(task: ActiveTask) -> bool:
    """
    Rebase task branch onto latest main before merging.
    Returns True if successful, False if conflicts require manual intervention.
    """
    for attempt in range(MAX_REBASE_ATTEMPTS):
        # Fetch latest main
        subprocess.run(
            ["git", "-C", str(task.worktree_path), "fetch", "origin", "main"],
            capture_output=True, check=False
        )

        result = subprocess.run(
            ["git", "-C", str(task.worktree_path), "rebase", "origin/main"],
            capture_output=True, text=True, check=False
        )

        if result.returncode == 0:
            print(f"[REBASE] Successfully rebased {task.slug} onto main")
            return True

        print(f"[REBASE] Attempt {attempt + 1}/{MAX_REBASE_ATTEMPTS} failed for {task.slug}")

        # Abort failed rebase
        subprocess.run(
            ["git", "-C", str(task.worktree_path), "rebase", "--abort"],
            capture_output=True, check=False
        )

    # Mark for manual intervention
    (task.worktree_path / ".needs-manual-rebase").touch()
    print(f"[REBASE] {task.slug} needs manual intervention")
    return False


# =============================================================================
# Async Planning Functions (Non-Blocking)
# =============================================================================

def start_claude_planning_async(task: ActiveTask, version: int) -> subprocess.Popen:
    """Start Claude planning in the task's worktree (non-blocking)."""
    plan_file = f"plan-v{version}.md"
    log_dir = task.worktree_task_dir() / "agent_logs"
    log_file = log_dir / f"claude-plan-v{version}.log"

    if version == 1:
        prompt = f"""Create a detailed implementation plan for task: {task.slug}

Read the ticket at ./workspace/tasks/in-progress/{task.slug}/ticket.md

Explore the codebase to understand:
- Current implementation patterns
- Files that will need modification
- Testing patterns used

Write a detailed plan to ./workspace/tasks/in-progress/{task.slug}/{plan_file} with:
- Summary of what needs to be done
- Step-by-step implementation approach
- Specific files to modify (with paths)
- Testing approach
- Edge cases to handle

Then stop."""
    else:
        prompt = f"""Revise the implementation plan for task: {task.slug}. This is revision {version}/3.

Read:
- ./workspace/tasks/in-progress/{task.slug}/ticket.md (the task)
- ./workspace/tasks/in-progress/{task.slug}/plan-v{version - 1}.md (previous plan)
- ./workspace/tasks/in-progress/{task.slug}/feedback-{version - 1}.md (feedback to address)

Create an improved plan at ./workspace/tasks/in-progress/{task.slug}/{plan_file}
Address the feedback while keeping what works well.

Then stop."""

    return start_agent_subprocess(
        task=task,
        cmd=[
            "claude", "-p",
            "--tools", "Read,Write,Glob,Grep,Edit,Bash",
            "--dangerously-skip-permissions"
        ],
        log_file=log_file,
        label=f"Claude plan-v{version} ({task.slug})",
        input_text=prompt
    )


def start_review_async(task: ActiveTask, iteration: int) -> subprocess.Popen:
    """Start plan review in the task's worktree (non-blocking).

    Uses Codex by default, falls back to Claude if Codex hit rate limit.
    """
    log_dir = task.worktree_task_dir() / "agent_logs"

    # Determine which agent to use
    use_claude = task.codex_rate_limited
    agent_name = "claude" if use_claude else "codex"
    log_file = log_dir / f"{agent_name}-review-{iteration}.log"

    prompt = f"""Review the implementation plan for task: {task.slug}

Read ./workspace/tasks/in-progress/{task.slug}/ticket.md (the task)
Read ./workspace/tasks/in-progress/{task.slug}/plan-v{iteration}.md (the plan)

Write feedback to ./workspace/tasks/in-progress/{task.slug}/feedback-{iteration}.md with:
- What's good about the plan
- What's missing or unclear
- Suggested improvements
- Any risks or concerns

Be specific and actionable. Focus on catching issues before implementation.
Then EXIT."""

    if use_claude:
        cmd = [
            "claude", "-p", prompt,
            "--allowedTools", "Edit,Write,Bash,Glob,Grep,Read",
        ]
        label = f"Claude review-{iteration} ({task.slug})"
    else:
        cmd = [
            "codex", "exec",
            "--dangerously-bypass-approvals-and-sandbox",
            "-C", ".",
            prompt
        ]
        label = f"Codex review-{iteration} ({task.slug})"

    return start_agent_subprocess(
        task=task,
        cmd=cmd,
        log_file=log_file,
        label=label,
        input_text=prompt if use_claude else None
    )


def check_codex_rate_limit(task: ActiveTask) -> bool:
    """Check if any recent Codex log shows a rate limit error."""
    log_dir = task.worktree_task_dir() / "agent_logs"
    if not log_dir.exists():
        return False

    # Check both exec and review logs
    exec_logs = list(log_dir.glob("codex-exec-*.log"))
    review_logs = list(log_dir.glob("codex-review-*.log"))
    all_logs = sorted(exec_logs + review_logs, key=lambda p: p.stat().st_mtime, reverse=True)

    if not all_logs:
        return False

    # Check the most recent Codex log
    latest_log = all_logs[0]
    try:
        content = latest_log.read_text()
        return "usage_limit_reached" in content or "You've hit your usage limit" in content
    except Exception:
        return False


def start_task_execution_async(task: ActiveTask) -> subprocess.Popen:
    """Start execution in the task's worktree (non-blocking).

    Uses Codex by default, falls back to Claude if Codex hit rate limit.
    """
    timestamp = datetime.now().strftime("%Y%m%d-%H%M%S")
    log_dir = task.worktree_task_dir() / "agent_logs"

    # Determine which agent to use
    use_claude = task.codex_rate_limited
    agent_name = "claude" if use_claude else "codex"
    log_file = log_dir / f"{agent_name}-exec-{timestamp}.log"

    prompt = f"""You are working on task: {task.slug}

Task directory: ./workspace/tasks/in-progress/{task.slug}/
- ticket.md: The task description
- plan.md: The implementation plan (already reviewed and finalized)

This branch is dedicated to this task: task/{task.slug}
You are running in an isolated worktree at {task.worktree_path}

The plan.md has been reviewed 3 times and is ready for implementation.
Follow the plan closely.

IMPORTANT: Check for .merge-requested file periodically. If it exists, commit your
work immediately and stop - the orchestrator needs to merge.

You are responsible for:
- Implementing according to plan.md
- Making bounded edits to the codebase
- Appending timestamped History entries to ticket.md
- Committing your own changes

Completion protocol:

When the task is fully complete:
1) Append a final History entry to ticket.md describing completion
2) Move the entire task directory to:
   ./workspace/tasks/outbound/{task.slug}/
3) EXIT the session

Do NOT merge to main yourself.
The script will handle squash-merge once the directory
appears in outbound.

Stay focused on this task."""

    task.phase = TaskPhase.EXECUTING
    persist_task_state(task)

    if use_claude:
        # Use Claude Code for execution
        cmd = [
            "claude", "-p", prompt,
            "--allowedTools", "Edit,Write,Bash,Glob,Grep,Read",
        ]
        label = f"Claude exec ({task.slug})"
    else:
        # Use Codex for execution
        cmd = [
            "codex", "exec",
            "--dangerously-bypass-approvals-and-sandbox",
            "-C", ".",
            prompt
        ]
        label = f"Codex exec ({task.slug})"

    return start_agent_subprocess(
        task=task,
        cmd=cmd,
        log_file=log_file,
        label=label
    )


def start_merge_agent_async(task: ActiveTask) -> subprocess.Popen:
    """Start merge agent in the task's worktree (non-blocking).

    Uses Claude by default for merges since it needs careful conflict resolution.
    Falls back based on codex_rate_limited flag.
    """
    timestamp = datetime.now().strftime("%Y%m%d-%H%M%S")
    log_dir = task.worktree_task_dir() / "agent_logs"

    # Always use Claude for merges - needs careful conflict resolution
    use_claude = True
    agent_name = "claude"
    log_file = log_dir / f"{agent_name}-merge-{timestamp}.log"

    prompt = f"""You are the MERGE AGENT for task: {task.slug}

CONTEXT:
- Task branch: {task.branch}
- Worktree: {task.worktree_path}
- Task completed and is in: ./workspace/tasks/outbound/{task.slug}/

YOUR MISSION - Rebase this branch onto main and resolve any conflicts:

STEP 1: Fetch and rebase
```bash
git fetch origin main
git rebase origin/main
```

STEP 2: If conflicts occur during rebase:
- Read the conflicting files to understand both sides
- Resolve conflicts intelligently:
  - Keep BOTH changes when they don't overlap
  - For overlapping changes, understand the intent and merge logically
  - Remove conflict markers (<<<<<<, =======, >>>>>>>)
- Stage resolved files: git add <file>
- Continue rebase: git rebase --continue
- Repeat until rebase completes

STEP 3: After successful rebase, signal completion:
```bash
# Move task to done
mv ./workspace/tasks/outbound/{task.slug} ./workspace/tasks/done/{task.slug}

# Commit the move
git add -A
git commit -m "merge {task.slug}: rebase complete, ready for squash"
```

STEP 4: EXIT the session

IF CONFLICTS CANNOT BE RESOLVED:
```bash
git rebase --abort
touch .needs-manual-rebase
```
Then EXIT - a human will need to resolve this.

IMPORTANT RULES:
- Work ONLY in this worktree
- Do NOT push to origin
- Do NOT merge to main yourself
- The orchestrator handles the final squash merge after you succeed
"""

    task.phase = TaskPhase.MERGING
    persist_task_state(task)

    # Use Claude Code for merge (needs careful conflict resolution)
    cmd = [
        "claude", "-p", prompt,
        "--allowedTools", "Edit,Write,Bash,Glob,Grep,Read",
    ]
    label = f"Claude merge ({task.slug})"

    return start_agent_subprocess(
        task=task,
        cmd=cmd,
        log_file=log_file,
        label=label
    )


# =============================================================================
# Recovery Functions
# =============================================================================

def recover_existing_worktrees(manager: ParallelTaskManager):
    """
    On startup, recover state from existing worktrees.
    """
    worktrees = list_worktrees()

    for wt in worktrees:
        path = Path(wt.get('path', ''))
        branch = wt.get('branch', '')

        # Skip main worktree
        if not branch.startswith('refs/heads/task/'):
            continue

        slug = branch.replace('refs/heads/task/', '')

        print(f"[RECOVER] Found existing worktree for {slug} at {path}")

        # Check for incomplete merge/rebase
        merge_head = path / ".git" / "MERGE_HEAD"
        rebase_dir = path / ".git" / "rebase-merge"

        if merge_head.exists():
            print(f"[RECOVER] Aborting incomplete merge in {slug}")
            subprocess.run(["git", "-C", str(path), "merge", "--abort"], check=False)

        if rebase_dir.exists():
            print(f"[RECOVER] Aborting incomplete rebase in {slug}")
            subprocess.run(["git", "-C", str(path), "rebase", "--abort"], check=False)

        # Load persisted state
        phase, iteration, use_claude = load_task_state(path)

        # Override phase based on directory structure
        in_progress = path / "workspace" / "tasks" / "in-progress" / slug
        outbound = path / "workspace" / "tasks" / "outbound" / slug

        if outbound.exists() and (outbound / "ticket.md").exists():
            phase = TaskPhase.OUTBOUND
        elif in_progress.exists() and (in_progress / "plan.md").exists():
            phase = TaskPhase.EXECUTING
        elif in_progress.exists():
            phase = TaskPhase.PLANNING

        task = ActiveTask(
            slug=slug,
            worktree_path=path,
            branch=f"task/{slug}",
            phase=phase,
            planning_iteration=iteration,
            codex_rate_limited=use_claude
        )

        manager.add_task(task)

        if phase == TaskPhase.OUTBOUND:
            manager.queue_for_merge(slug)
            print(f"[RECOVER] {slug} queued for merge")
        else:
            print(f"[RECOVER] {slug} in phase {phase.value}, iteration {iteration}")


def setup_signal_handlers(manager: ParallelTaskManager):
    """Setup handlers for graceful shutdown."""

    def signal_handler(signum, frame):
        print(f"\n[SHUTDOWN] Received signal {signum}, cleaning up...")

        for task in manager.active_tasks.values():
            # Kill pane if using tmux
            if task.pane_id:
                print(f"[SHUTDOWN] Killing pane {task.pane_id} for {task.slug}...")
                kill_pane(task.pane_id)
            # Kill subprocess if using legacy mode
            elif task.is_alive:
                print(f"[SHUTDOWN] Terminating {task.slug}...")
                kill_task_subprocess(task, timeout=30)

        print("[SHUTDOWN] Cleanup complete, exiting.")
        print(f"[SHUTDOWN] tmux session '{AGENT_SESSION}' preserved (attach with: tmux attach -t {AGENT_SESSION})")
        sys.exit(0)

    signal.signal(signal.SIGINT, signal_handler)
    signal.signal(signal.SIGTERM, signal_handler)


# =============================================================================
# Merge Task from Worktree
# =============================================================================

def finalize_squash_merge(task: ActiveTask, manager: ParallelTaskManager) -> bool:
    """
    Finalize squash merge after merge agent has rebased successfully.
    Agent already moved task to done/ and committed.
    We just need to push, squash merge to main, and cleanup.
    Returns True on success.
    """
    import shutil

    # Acquire merge lock (only one merge at a time)
    lock_fd = acquire_merge_lock()
    if lock_fd is None:
        print(f"[MERGE] Cannot acquire lock, another merge in progress")
        return False

    try:
        # Push worktree branch
        subprocess.run(
            ["git", "-C", str(task.worktree_path), "push", "-u", "origin", task.branch, "--force"],
            capture_output=True, check=False
        )

        # In main repo, perform squash merge
        current = git_current_branch()
        if current != "main":
            run(["git", "switch", "main"], check=False)

        # Ensure main is clean before merging
        status = subprocess.run(
            ["git", "status", "--porcelain"],
            capture_output=True, text=True, check=False
        )
        if status.stdout.strip():
            # Main has uncommitted changes or conflicts - abort any pending merge and reset
            print(f"[MERGE] Main repo is dirty, aborting pending operations")
            subprocess.run(["git", "merge", "--abort"], capture_output=True, check=False)
            subprocess.run(["git", "rebase", "--abort"], capture_output=True, check=False)
            subprocess.run(["git", "reset", "--hard", "HEAD"], capture_output=True, check=False)

        run(["git", "fetch", "origin"], check=False)
        run(["git", "pull", "--ff-only"], check=False)

        # Get commits for message
        commits = run_capture([
            "git", "log", "--oneline", f"main..{task.branch}"
        ], check=False)

        # Squash merge
        result = subprocess.run(
            ["git", "merge", "--squash", task.branch],
            capture_output=True, text=True, check=False
        )

        if result.returncode != 0:
            print(f"[MERGE] Squash merge failed for {task.slug}: {result.stderr}")
            subprocess.run(["git", "merge", "--abort"], check=False)
            return False

        # Copy done dir from worktree to main
        worktree_done = task.worktree_path / "workspace" / "tasks" / "done" / task.slug
        done_dir = TASKS_DONE / task.slug

        if worktree_done.exists():
            if done_dir.exists():
                shutil.rmtree(done_dir)
            shutil.copytree(str(worktree_done), str(done_dir))
            run(["git", "add", str(done_dir)], check=False)

        # Commit
        commit_msg = f"complete task {task.slug}\n\nCommits:\n{commits}" if commits else f"complete task {task.slug}"
        run(["git", "commit", "-m", commit_msg], check=False)

        # Delete remote and local task branches
        subprocess.run(["git", "push", "origin", "--delete", task.branch], capture_output=True, check=False)
        subprocess.run(["git", "branch", "-D", task.branch], capture_output=True, check=False)

        # Cleanup worktree
        cleanup_worktree(task.slug)

        # Cleanup session files
        session_file = session_file_for(task.slug)
        if session_file.exists():
            session_file.unlink()
        planning_file = SESSIONS_DIR / f"{task.slug}.planning"
        if planning_file.exists():
            planning_file.unlink()

        # Remove from merge queue and task manager
        if task.slug in manager.merge_queue:
            manager.merge_queue.remove(task.slug)
        manager.remove_task(task.slug)

        return True

    finally:
        release_merge_lock(lock_fd)


def merge_task_from_worktree(task: ActiveTask, manager: ParallelTaskManager) -> bool:
    """
    Merge a completed task from its worktree to main.
    First-wins: if merge succeeds, task is done.
    Returns True on success.
    """
    import shutil

    # Acquire merge lock (only one merge at a time)
    lock_fd = acquire_merge_lock()
    if lock_fd is None:
        print(f"[MERGE] Cannot acquire lock, another merge in progress")
        return False

    try:
        task.phase = TaskPhase.MERGING
        persist_task_state(task)

        # Request merge freeze and wait for clean worktree
        request_merge_freeze(task)
        if not wait_for_clean_worktree(task, timeout=60):
            # Force kill if still running
            kill_task_subprocess(task, timeout=10)

        # Commit any pending changes in worktree
        subprocess.run(
            ["git", "-C", str(task.worktree_path), "add", "-A"],
            capture_output=True, check=False
        )
        subprocess.run(
            ["git", "-C", str(task.worktree_path), "commit", "-m", f"finalize task {task.slug}"],
            capture_output=True, check=False
        )

        # Rebase onto latest main before merging
        if not rebase_before_merge(task):
            print(f"[MERGE] Rebase failed for {task.slug}, needs manual intervention")
            task.phase = TaskPhase.OUTBOUND  # Revert phase
            persist_task_state(task)
            return False

        # Push worktree branch (for merge) - this handles case where origin is configured
        subprocess.run(
            ["git", "-C", str(task.worktree_path), "push", "-u", "origin", task.branch, "--force"],
            capture_output=True, check=False
        )

        # In main repo, perform squash merge
        # Ensure we're on main
        current = git_current_branch()
        if current != "main":
            run(["git", "switch", "main"], check=False)

        run(["git", "fetch", "origin"], check=False)
        run(["git", "pull", "--ff-only"], check=False)

        # Get commits for message
        commits = run_capture([
            "git", "log", "--oneline", f"main..{task.branch}"
        ], check=False)

        # Squash merge
        result = subprocess.run(
            ["git", "merge", "--squash", task.branch],
            capture_output=True, text=True, check=False
        )

        if result.returncode != 0:
            print(f"[MERGE] Squash merge failed for {task.slug}: {result.stderr}")
            # Abort merge
            subprocess.run(["git", "merge", "--abort"], check=False)
            task.phase = TaskPhase.OUTBOUND
            persist_task_state(task)
            return False

        # Move outbound to done in main repo
        outbound_dir = TASKS_OUTBOUND / task.slug
        done_dir = TASKS_DONE / task.slug

        # Also need to handle the worktree's outbound dir - copy to main's done
        worktree_outbound = task.worktree_path / "workspace" / "tasks" / "outbound" / task.slug
        if worktree_outbound.exists():
            if done_dir.exists():
                shutil.rmtree(done_dir)
            shutil.copytree(str(worktree_outbound), str(done_dir))
            run(["git", "add", str(done_dir)], check=False)

        # If there's also a local outbound dir, remove it
        if outbound_dir.exists():
            shutil.rmtree(outbound_dir)

        # Commit
        commit_msg = f"complete task {task.slug}\n\nCommits:\n{commits}" if commits else f"complete task {task.slug}"
        run(["git", "commit", "-m", commit_msg], check=False)

        # Delete remote task branch (if exists)
        subprocess.run(["git", "push", "origin", "--delete", task.branch], capture_output=True, check=False)

        # Delete local task branch
        subprocess.run(["git", "branch", "-D", task.branch], capture_output=True, check=False)

        # Cleanup worktree
        cleanup_worktree(task.slug)

        # Cleanup session files
        session_file = session_file_for(task.slug)
        if session_file.exists():
            session_file.unlink()
        planning_file = SESSIONS_DIR / f"{task.slug}.planning"
        if planning_file.exists():
            planning_file.unlink()

        # Remove from task manager
        manager.remove_task(task.slug)

        print(f"[MERGE] Successfully merged and archived: {task.slug}")
        return True

    finally:
        release_merge_lock(lock_fd)


# =============================================================================
# Parallel Main Loop Helper Functions
# =============================================================================

def check_completed_tasks(manager: ParallelTaskManager):
    """Check each active task for completion (outbound directory)."""
    for task in list(manager.active_tasks.values()):
        if task.phase not in (TaskPhase.EXECUTING, TaskPhase.PLANNING):
            continue

        outbound_dir = task.worktree_path / "workspace" / "tasks" / "outbound" / task.slug

        if outbound_dir.exists() and (outbound_dir / "ticket.md").exists():
            print(f"[COMPLETE] Task {task.slug} ready for merge")
            task.phase = TaskPhase.OUTBOUND
            persist_task_state(task)
            manager.queue_for_merge(task.slug)


def process_merge_queue(manager: ParallelTaskManager):
    """Process one task from the merge queue - start merge agent if needed."""
    slug = manager.next_to_merge()
    if slug is None:
        return

    task = manager.get_task(slug)
    if task is None:
        manager.merge_queue.remove(slug)
        return

    # Check for manual intervention flag
    if (task.worktree_path / ".needs-manual-rebase").exists():
        print(f"[MERGE] {slug} needs manual intervention, skipping")
        return

    # If already in MERGING phase, let handle_merging_tasks() handle it
    if task.phase == TaskPhase.MERGING:
        return

    # Wait for execution to finish before starting merge
    if task.state == TaskState.RUNNING:
        print(f"[MERGE] Waiting for {slug} to finish execution...")
        request_merge_freeze(task)
        return

    # Start merge agent via tmux (will set phase to MERGING)
    print(f"[MERGE] Starting merge agent for {slug}")
    send_merge_command_tmux(task)


def advance_planning_tasks(manager: ParallelTaskManager):
    """Advance planning phase for all tasks in PLANNING phase."""
    for task in list(manager.get_tasks_in_phase(TaskPhase.PLANNING)):
        # If task is running in tmux, check for completion
        if task.state == TaskState.RUNNING:
            completed, exit_code = check_step_completion_tmux(task)
            if not completed:
                continue  # Still running
            # Mark as idle to proceed to next step
            task.state = TaskState.IDLE
            print(f"[PLANNING] {task.slug} step {task.current_step} completed (exit {exit_code})")

        # Skip if still running (legacy subprocess mode)
        if task.process and task.is_alive:
            continue

        # Skip if in recovery mode
        if task.state == TaskState.RECOVERING:
            continue

        # Check for Codex rate limit before retrying
        if not task.codex_rate_limited and check_codex_rate_limit(task):
            print(f"[PLANNING] Task {task.slug} hit Codex rate limit, switching to Claude")
            task.codex_rate_limited = True
            persist_task_state(task)

        # Check planning progress based on files
        task_dir = task.worktree_task_dir()
        plan_v3 = task_dir / "plan-v3.md"
        plan_final = task_dir / "plan.md"
        feedback_3 = task_dir / "feedback-3.md"

        # If plan.md exists, planning is complete
        if plan_final.exists():
            print(f"[PLANNING] {task.slug} complete, starting execution")
            send_execution_command_tmux(task)
            continue

        # Determine current iteration from files
        # Each iteration: Claude creates plan-v{n}, Codex creates feedback-{n}
        iteration = task.planning_iteration

        if iteration == 0:
            # Start: Claude creates plan-v1
            plan_v1 = task_dir / "plan-v1.md"
            if not plan_v1.exists():
                send_planning_command_tmux(task, version=1)
            else:
                task.planning_iteration = 1
                persist_task_state(task)

        elif iteration == 1:
            # Codex reviews plan-v1 → feedback-1, then Claude creates plan-v2
            feedback_1 = task_dir / "feedback-1.md"
            plan_v2 = task_dir / "plan-v2.md"
            if not feedback_1.exists():
                send_review_command_tmux(task, iteration=1)
            elif not plan_v2.exists():
                send_planning_command_tmux(task, version=2)
            else:
                task.planning_iteration = 2
                persist_task_state(task)

        elif iteration == 2:
            # Codex reviews plan-v2 → feedback-2, then Claude creates plan-v3
            feedback_2 = task_dir / "feedback-2.md"
            if not feedback_2.exists():
                send_review_command_tmux(task, iteration=2)
            elif not plan_v3.exists():
                send_planning_command_tmux(task, version=3)
            else:
                task.planning_iteration = 3
                persist_task_state(task)

        elif iteration >= 3:
            # Codex final review → feedback-3, then copy plan-v3 to plan.md
            if not feedback_3.exists():
                send_review_command_tmux(task, iteration=3)
            elif plan_v3.exists() and not plan_final.exists():
                # Copy plan-v3 to plan.md
                plan_final.write_text(plan_v3.read_text())
                # Commit in worktree
                subprocess.run(
                    ["git", "-C", str(task.worktree_path), "add", "-A"],
                    check=False
                )
                subprocess.run(
                    ["git", "-C", str(task.worktree_path), "commit", "-m", f"plan {task.slug}: finalize plan.md from plan-v3"],
                    check=False
                )
                print(f"[PLANNING] {task.slug} finalized, starting execution")
                send_execution_command_tmux(task)


def start_new_task_if_available(manager: ParallelTaskManager):
    """Pick next tasks from todo and start them in worktrees until capacity is full."""
    active_slugs = set(manager.active_tasks.keys())

    while manager.can_start_new_task():
        next_task_dir = pick_next_task(skip_slugs=active_slugs, manager=manager)
        if next_task_dir is None:
            return

        slug = next_task_dir.name
        active_slugs.add(slug)  # Don't pick this one again

        print(f"[NEW] Starting task {slug}")
        task = setup_task_in_worktree(slug, manager)
        if task:
            # Start planning via tmux
            send_planning_command_tmux(task, version=1)


def handle_execution_tasks(manager: ParallelTaskManager):
    """Handle execution phase tasks - check completion or restart if crashed."""
    for task in list(manager.get_tasks_in_phase(TaskPhase.EXECUTING)):
        # Check tmux pane for completion
        if task.state == TaskState.RUNNING:
            completed, exit_code = check_step_completion_tmux(task)
            if not completed:
                continue  # Still running

            # Check if task completed (moved to outbound)
            outbound_dir = task.worktree_path / "workspace" / "tasks" / "outbound" / task.slug

            if outbound_dir.exists() and (outbound_dir / "ticket.md").exists():
                print(f"[EXEC] Task {task.slug} completed, queuing for merge")
                task.phase = TaskPhase.OUTBOUND
                task.state = TaskState.IDLE
                persist_task_state(task)
                manager.queue_for_merge(task.slug)
            elif exit_code != 0:
                # Task crashed - check if it's a Codex rate limit
                if not task.codex_rate_limited and check_codex_rate_limit(task):
                    print(f"[EXEC] Task {task.slug} hit Codex rate limit, switching to Claude")
                    task.codex_rate_limited = True
                    persist_task_state(task)
                else:
                    print(f"[EXEC] Task {task.slug} crashed (exit {exit_code}), restarting...")
                send_execution_command_tmux(task)
            else:
                # Exited 0 but not complete - restart
                print(f"[EXEC] Task {task.slug} exited but not complete, restarting...")
                send_execution_command_tmux(task)
            continue

        # Skip if in recovery
        if task.state == TaskState.RECOVERING:
            continue

        # If idle (e.g., recovered task), start execution
        if task.state == TaskState.IDLE:
            send_execution_command_tmux(task)


def handle_merging_tasks(manager: ParallelTaskManager):
    """Handle merging phase tasks - check if merge agent completed."""
    for task in list(manager.get_tasks_in_phase(TaskPhase.MERGING)):
        # Check tmux pane for completion
        if task.state == TaskState.RUNNING:
            completed, exit_code = check_step_completion_tmux(task)
            if not completed:
                continue  # Still running

            # Merge agent finished - check result
            done_dir = task.worktree_path / "workspace" / "tasks" / "done" / task.slug
            needs_manual = task.worktree_path / ".needs-manual-rebase"

            if done_dir.exists() and (done_dir / "ticket.md").exists():
                # Merge agent succeeded - do final squash merge
                print(f"[MERGE] Task {task.slug} rebased successfully, doing final squash merge")
                task.state = TaskState.IDLE
                if finalize_squash_merge(task, manager):
                    print(f"[MERGE] Task {task.slug} merged to main successfully")
                    # Kill the pane since task is done
                    kill_pane(task.pane_id)
                else:
                    print(f"[MERGE] Task {task.slug} squash merge failed, will retry")
                    send_merge_command_tmux(task)
            elif needs_manual.exists():
                # Agent couldn't resolve conflicts
                print(f"[MERGE] Task {task.slug} needs manual conflict resolution")
                task.state = TaskState.FAILED
                # Remove from merge queue, keep in MERGING phase
                if task.slug in manager.merge_queue:
                    manager.merge_queue.remove(task.slug)
            elif exit_code != 0:
                # Merge agent crashed - restart
                print(f"[MERGE] Task {task.slug} merge agent crashed (exit {exit_code}), restarting...")
                send_merge_command_tmux(task)
            else:
                # Exited 0 but not complete - restart
                print(f"[MERGE] Task {task.slug} merge agent exited incomplete, restarting...")
                send_merge_command_tmux(task)
            continue

        # Skip if in recovery
        if task.state == TaskState.RECOVERING:
            continue


def session_file_for(slug: str) -> Path:
    return SESSIONS_DIR / f"{slug}.session"


def match_task_filter(slug: str, filters: list[str]) -> bool:
    """Check if a task slug matches any of the filter patterns.

    Filters can be:
    - Full slug: "0005-transactional-domain-delete"
    - Task number prefix: "0005" or just "5"
    """
    for f in filters:
        # Normalize: "5" -> "0005", "05" -> "0005"
        if f.isdigit():
            f = f.zfill(4)
        if slug == f or slug.startswith(f"{f}-"):
            return True
    return False


def pick_next_task(skip_slugs: set[str] | None = None, manager: ParallelTaskManager | None = None) -> Path | None:
    """Get the next task directory from todo.

    If manager has priority_queue entries, tries to match those first (popping when matched),
    then falls back to regular sorted order.
    """
    skip_slugs = skip_slugs or set()

    # Get all available tasks
    all_task_dirs = [
        d for d in TASKS_TODO.iterdir()
        if d.is_dir() and (d / "ticket.md").exists() and d.name not in skip_slugs
    ]

    if not all_task_dirs:
        return None

    # Try priority queue first - pop patterns until we find a matching task
    if manager and manager.priority_queue:
        while manager.priority_queue:
            prio = manager.priority_queue[0]  # Peek
            for d in all_task_dirs:
                if match_task_filter(d.name, [prio]):
                    manager.priority_queue.popleft()  # Pop only when matched
                    return d
            # No match for this priority pattern, remove it and try next
            manager.priority_queue.popleft()

    # Fall back to regular sorted order
    all_task_dirs.sort(key=lambda d: d.name)
    return all_task_dirs[0] if all_task_dirs else None


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


def run_agent_with_logs(
    cmd: list[str],
    log_file: Path,
    label: str,
    input_text: str | None = None,
    tail_lines: int = 3,
    watch_file: Path | None = None,
    watch_min_size: int = 100,
    use_script: bool = False
) -> int:
    """
    Run an agent command, streaming output to log file and showing last N lines in terminal.

    If watch_file is provided, kills the process once that file exists and has >= watch_min_size bytes.
    If use_script is True, wraps command with `script` to capture TUI output.

    Returns the exit code.
    """
    log_file.parent.mkdir(parents=True, exist_ok=True)

    # Track last N lines for display
    last_lines: deque[str] = deque(maxlen=tail_lines)
    lines_displayed = 0
    killed_by_watcher = False

    def display_tail():
        """Clear previous lines and redisplay the last N lines."""
        nonlocal lines_displayed
        if not last_lines:
            return

        # Move cursor up to overwrite previous output
        if lines_displayed > 0:
            sys.stdout.write(f"\033[{lines_displayed}A")

        # Clear and print each line
        for line in last_lines:
            display = line[:100] + "..." if len(line) > 100 else line
            sys.stdout.write(f"\033[2K  {display}\n")

        lines_displayed = len(last_lines)
        sys.stdout.flush()

    def file_watcher(proc: subprocess.Popen):
        """Watch for expected output file and kill process when it's ready."""
        nonlocal killed_by_watcher
        while proc.poll() is None:  # While process is running
            if watch_file and watch_file.exists():
                try:
                    size = watch_file.stat().st_size
                    if size >= watch_min_size:
                        print(f"\n[{label}] Output file ready ({size} bytes), terminating...")
                        killed_by_watcher = True
                        proc.terminate()
                        return
                except OSError:
                    pass
            time.sleep(2)

    print(f"[{label}] Logging to {log_file}")

    # Build command - wrap with script if needed to capture TUI output
    if use_script:
        # script -q -c "command" logfile captures terminal output including TUI
        cmd_str = " ".join(cmd)
        actual_cmd = ["script", "-q", "-c", cmd_str, str(log_file)]
        # For script mode, we read from script's stdout for tail display
        # The log file is created by script itself
    else:
        actual_cmd = cmd

    # Force non-TTY environment - unset TERM to prevent TUI mode detection
    env = os.environ.copy()
    env.pop("TERM", None)  # Remove TERM to force non-interactive mode

    print(f"[{label}] Running: {' '.join(actual_cmd)}")

    process = subprocess.Popen(
        actual_cmd,
        stdin=subprocess.PIPE if input_text else None,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        bufsize=1,
        env=env
    )

    # Start file watcher thread if watch_file provided
    if watch_file:
        watcher_thread = threading.Thread(target=file_watcher, args=(process,), daemon=True)
        watcher_thread.start()

    # Send input if provided
    if input_text and process.stdin:
        process.stdin.write(input_text)
        process.stdin.close()

    # For non-script mode, also write to log file manually
    if not use_script:
        line_count = 0
        with open(log_file, "w") as f:
            if process.stdout:
                for line in process.stdout:
                    line = line.rstrip('\n')
                    f.write(line + "\n")
                    f.flush()
                    last_lines.append(line)
                    line_count += 1
                    display_tail()
        if line_count == 0:
            print(f"[{label}] WARNING: No output captured from subprocess")
    else:
        # For script mode, just read stdout for tail display
        if process.stdout:
            for line in process.stdout:
                line = line.rstrip('\n')
                last_lines.append(line)
                display_tail()

    process.wait()

    if killed_by_watcher:
        print(f"[{label}] Terminated (output file ready)")
    else:
        print(f"[{label}] Exit code: {process.returncode}")

    return process.returncode


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


def commit_new_workspace_files() -> bool:
    """
    Check for and commit any new/modified files in workspace/.
    Returns True if files were committed.
    """
    # Check for untracked or modified files in workspace/
    result = subprocess.run(
        ["git", "status", "--porcelain", "workspace/"],
        capture_output=True, text=True, check=False
    )

    if not result.stdout.strip():
        return False

    # Count new files for commit message
    lines = result.stdout.strip().split('\n')
    new_files = [l for l in lines if l.startswith('?') or l.startswith('A')]
    modified_files = [l for l in lines if l.startswith('M') or l.startswith(' M')]

    print(f"[HOUSEKEEPING] Found {len(new_files)} new, {len(modified_files)} modified files in workspace/")

    # Add and commit
    run(["git", "add", "workspace/"], check=False)

    # Build commit message
    parts = []
    if new_files:
        parts.append(f"{len(new_files)} new")
    if modified_files:
        parts.append(f"{len(modified_files)} modified")
    msg = f"workspace: add {', '.join(parts)} files"

    run(["git", "commit", "-m", msg], check=False)
    return True


# =============================================================================
# Planning Phase Functions
# =============================================================================

def run_claude_planning(slug: str, version: int, feedback_content: str = ""):
    """Have Claude CLI create a versioned plan (with codebase exploration)."""
    task_dir = TASKS_IN_PROGRESS / slug
    plan_file = f"plan-v{version}.md"
    log_dir = task_dir / "agent_logs"
    log_file = log_dir / f"claude-plan-v{version}.log"

    if version == 1:
        prompt = f"""Create a detailed implementation plan for task: {slug}

Read the ticket at ./workspace/tasks/in-progress/{slug}/ticket.md

Explore the codebase to understand:
- Current implementation patterns
- Files that will need modification
- Testing patterns used

Write a detailed plan to ./workspace/tasks/in-progress/{slug}/{plan_file} with:
- Summary of what needs to be done
- Step-by-step implementation approach
- Specific files to modify (with paths)
- Testing approach
- Edge cases to handle

Then stop."""
    else:
        prompt = f"""Revise the implementation plan for task: {slug}. This is revision {version}/3.

Read:
- ./workspace/tasks/in-progress/{slug}/ticket.md (the task)
- ./workspace/tasks/in-progress/{slug}/plan-v{version - 1}.md (previous plan)
- ./workspace/tasks/in-progress/{slug}/feedback-{version - 1}.md (feedback to address)

Create an improved plan at ./workspace/tasks/in-progress/{slug}/{plan_file}
Address the feedback while keeping what works well.

Then stop."""

    # Watch for the plan file - kill Claude once it's written
    expected_plan = task_dir / plan_file

    run_agent_with_logs(
        cmd=[
            "claude", "-p",
            "--tools", "Read,Write,Glob,Grep,Edit,Bash",
            "--dangerously-skip-permissions"
        ],
        log_file=log_file,
        label=f"Claude {plan_file}",
        input_text=prompt,
        watch_file=expected_plan,
        watch_min_size=100  # Plan should be at least 100 bytes
    )

    # Commit the plan and log
    run(["git", "add", str(task_dir)], check=False)
    run(["git", "commit", "-m", f"plan {slug}: create {plan_file}"], check=False)


def run_codex_review(slug: str, iteration: int):
    """Have Codex review the plan and write versioned feedback."""
    task_dir = TASKS_IN_PROGRESS / slug
    plan_file = f"plan-v{iteration}.md"
    feedback_file = f"feedback-{iteration}.md"
    log_dir = task_dir / "agent_logs"
    log_file = log_dir / f"codex-review-{iteration}.log"

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

    run_agent_with_logs(
        cmd=[
            "codex", "exec",
            "--dangerously-bypass-approvals-and-sandbox",
            "-C", ".",
            prompt
        ],
        log_file=log_file,
        label=f"Codex review-{iteration}"
    )

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
    log_dir = target_dir / "agent_logs"
    timestamp = datetime.now().strftime("%Y%m%d-%H%M%S")
    log_file = log_dir / f"codex-exec-{timestamp}.log"

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

    run_agent_with_logs(
        cmd=[
            "codex", "exec",
            "--dangerously-bypass-approvals-and-sandbox",
            "-C", ".",
            prompt
        ],
        log_file=log_file,
        label=f"Codex exec {slug}"
    )

    # Commit the execution log
    run(["git", "add", str(log_file)], check=False)
    run(["git", "commit", "-m", f"log: codex exec {slug}"], check=False)


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


# =============================================================================
# Overseer Functions (Stuck Detection and Recovery)
# =============================================================================

def check_if_stuck_with_claude(task: ActiveTask, pane_output: str) -> tuple[bool, str]:
    """
    Use Claude to analyze pane output and determine if task is stuck.
    Returns (is_stuck, reason) tuple.
    """
    if not pane_output.strip():
        return (False, "")

    # First check for obvious stuck patterns without calling Claude
    obvious_stuck_patterns = [
        ("error:", "Error detected in output"),
        ("panic:", "Panic detected"),
        ("SIGTERM", "Process was terminated"),
        ("rate limit", "Rate limit hit"),
        ("usage_limit_reached", "Usage limit reached"),
    ]

    output_lower = pane_output.lower()
    for pattern, reason in obvious_stuck_patterns:
        if pattern.lower() in output_lower:
            return (True, reason)

    # Check if no progress for a long time (output hasn't changed)
    # This is a simple heuristic - if the last 20 lines are all the same character
    # or empty, consider it potentially stuck
    last_lines = pane_output.strip().split('\n')[-20:]
    if all(not line.strip() for line in last_lines):
        return (True, "No output activity")

    # For more complex analysis, could spawn Claude to analyze
    # But for now, rely on timeout-based detection
    return (False, "")


def spawn_recovery_in_pane(task: ActiveTask, pane_output: str, reason: str):
    """
    Spawn a recovery Claude instance in the task's pane.
    """
    timestamp = datetime.now().strftime("%Y%m%d-%H%M%S")
    log_dir = task.worktree_task_dir() / "agent_logs"
    log_file = log_dir / f"claude-recovery-{timestamp}.log"
    log_dir.mkdir(parents=True, exist_ok=True)

    # Kill any running process in the pane first
    send_to_pane(task.pane_id, "\x03")  # Ctrl+C
    time.sleep(1)

    prompt = f"""You are a RECOVERY AGENT for task: {task.slug}

CONTEXT:
- The previous agent appears to be stuck or has failed
- Reason detected: {reason}
- Current step: {task.current_step}
- Phase: {task.phase.value}
- Worktree: {task.worktree_path}

RECENT PANE OUTPUT (last 100 lines):
{pane_output[-5000:]}

YOUR MISSION:
1. Analyze what went wrong
2. Fix any immediate issues (resolve conflicts, fix errors, etc.)
3. Resume the task from where it left off

Task files are in: ./workspace/tasks/in-progress/{task.slug}/

If the task was in PLANNING phase:
- Check which plan version exists, create the next one if needed
- Ensure feedback files are written

If the task was in EXECUTING phase:
- Check ticket.md History for progress
- Continue implementation from where it stopped

If the task was in MERGING phase:
- Check for rebase conflicts
- Resolve conflicts or mark for manual intervention

When done, EXIT the session. The orchestrator will restart the next step.
"""

    escaped = escape_for_shell(prompt)
    cmd = f"claude -p '{escaped}' --allowedTools Edit,Write,Bash,Glob,Grep,Read 2>&1 | tee {log_file}; echo '__DONE_recovery_EXIT_'$?"

    task.current_step = "recovery"
    task.state = TaskState.RECOVERING
    task.last_command_time = datetime.now()

    send_long_command_to_pane(task.pane_id, cmd)
    print(f"[OVERSEER] Spawned recovery agent for {task.slug} (reason: {reason})")


def run_overseer(manager: ParallelTaskManager):
    """
    Background thread that monitors all panes for stuck tasks.
    Runs every OVERSEER_CHECK_INTERVAL seconds.
    """
    print(f"[OVERSEER] Started (check interval: {OVERSEER_CHECK_INTERVAL}s, stuck timeout: {STUCK_TIMEOUT_MINUTES}min)")

    while True:
        time.sleep(OVERSEER_CHECK_INTERVAL)

        for task in list(manager.active_tasks.values()):
            # Skip tasks that have exceeded recovery attempts
            if task.recovery_attempts >= MAX_RECOVERY_ATTEMPTS:
                if task.state != TaskState.FAILED:
                    print(f"[OVERSEER] Task {task.slug} exceeded max recovery attempts, marking as FAILED")
                    task.state = TaskState.FAILED
                continue

            # Skip tasks without panes
            if not task.pane_id:
                continue

            # Check if pane is still alive
            if not is_pane_alive(task.pane_id):
                print(f"[OVERSEER] Pane {task.pane_id} for {task.slug} is dead, recreating...")
                task.pane_id = create_task_pane(task.slug, task.worktree_path)
                if task.pane_id:
                    # Restart the current step
                    task.state = TaskState.IDLE
                continue

            # Check for stuck based on timeout
            if task.last_command_time and task.state == TaskState.RUNNING:
                elapsed = (datetime.now() - task.last_command_time).total_seconds()
                if elapsed > STUCK_TIMEOUT_MINUTES * 60:
                    print(f"[OVERSEER] Task {task.slug} appears stuck (no completion after {STUCK_TIMEOUT_MINUTES}min)")
                    output = capture_pane_output(task.pane_id, 300)
                    is_stuck, reason = check_if_stuck_with_claude(task, output)

                    if is_stuck or elapsed > STUCK_TIMEOUT_MINUTES * 60 * 2:
                        # Definitely stuck, spawn recovery
                        task.recovery_attempts += 1
                        reason = reason or f"Timeout after {int(elapsed / 60)} minutes"
                        spawn_recovery_in_pane(task, output, reason)


def recover_existing_panes(manager: ParallelTaskManager):
    """
    On startup, check for existing panes in the agent session and recover their state.
    """
    panes = list_session_panes()

    for pane_info in panes:
        pane_id = pane_info.get('pane_id', '')
        title = pane_info.get('title', '')

        # Skip the main pane
        if not title or title == 'main':
            continue

        # Title should be the task slug
        slug = title
        task = manager.get_task(slug)

        if task:
            # Task exists, update its pane_id
            print(f"[RECOVER] Reconnecting task {slug} to pane {pane_id}")
            task.pane_id = pane_id
            task.state = TaskState.RUNNING  # Assume it's running until we check
        else:
            # Orphan pane - kill it
            print(f"[RECOVER] Killing orphan pane {pane_id} (title: {title})")
            kill_pane(pane_id)


def parse_args():
    """Parse command line arguments."""
    parser = argparse.ArgumentParser(
        description="Agent loop that manages parallel task execution via git worktrees."
    )
    parser.add_argument(
        "-j", "--max-jobs",
        type=int,
        default=DEFAULT_MAX_CONCURRENT_TASKS,
        metavar="N",
        help=f"Maximum concurrent tasks (default: {DEFAULT_MAX_CONCURRENT_TASKS})"
    )
    parser.add_argument(
        "tasks",
        nargs="*",
        metavar="TASK",
        help="Priority task numbers to run first (e.g., 5 6 20), then continues with regular order"
    )
    return parser.parse_args()


def main_parallel():
    """
    Parallel main loop that manages multiple concurrent tasks.
    Each task runs in its own git worktree with tmux panes for visibility.
    """
    args = parse_args()

    # Initialize tmux
    check_tmux_installed()
    ensure_agent_session()

    setup_directories()

    # Check for stale merge lock from previous crash
    check_stale_merge_lock()

    # Create task manager with configured max tasks and priority queue
    manager = ParallelTaskManager(max_tasks=args.max_jobs, priority_queue=deque(args.tasks))

    # Setup signal handlers for graceful shutdown
    setup_signal_handlers(manager)

    # Recover any existing worktrees from previous runs
    recover_existing_worktrees(manager)

    # Recover existing tmux panes
    recover_existing_panes(manager)

    # Create panes for recovered tasks that don't have them
    for task in list(manager.active_tasks.values()):
        if not task.pane_id:
            task.pane_id = create_task_pane(task.slug, task.worktree_path)
            task.state = TaskState.IDLE  # Will be restarted by main loop

    # Start overseer thread for stuck detection
    overseer = threading.Thread(target=run_overseer, args=(manager,), daemon=True)
    overseer.start()

    print(f"[STARTUP] Parallel agent loop started (max {manager.max_tasks} concurrent tasks)")
    print(f"[STARTUP] tmux session: {AGENT_SESSION}")
    print(f"[STARTUP] Attach to see all tasks: tmux attach -t {AGENT_SESSION}")
    if args.tasks:
        print(f"[STARTUP] Priority queue: {', '.join(args.tasks)}")
    print(f"[STARTUP] Worktrees location: {WORKTREES_ROOT.resolve()}")

    while True:
        step_ts = datetime.now().isoformat()
        active_count = len(manager.active_tasks)
        running_count = len(manager.get_running_tasks())
        print(f"[{step_ts}] loop tick - {active_count} active, {running_count} running")

        # PHASE 1: Check completed tasks → queue for merge
        check_completed_tasks(manager)

        # PHASE 2: Process merge queue (one at a time, first-wins)
        process_merge_queue(manager)

        # PHASE 3: Advance planning for tasks in PLANNING phase
        advance_planning_tasks(manager)

        # PHASE 4: Start new tasks if capacity available
        start_new_task_if_available(manager)

        # PHASE 5: Handle execution tasks (restart if crashed)
        handle_execution_tasks(manager)

        # PHASE 6: Handle merging tasks (check merge agent status)
        handle_merging_tasks(manager)

        # Commit any housekeeping files in main repo
        # Only when no tasks are actively merging
        if git_current_branch() == "main" and not manager.get_tasks_in_phase(TaskPhase.MERGING):
            commit_new_workspace_files()

        # Status summary
        if manager.active_tasks:
            for task in manager.active_tasks.values():
                pane_info = f"pane {task.pane_id}" if task.pane_id else "no pane"
                step_info = f" ({task.current_step})" if task.current_step else ""
                print(f"  [{task.slug}] {task.phase.value}/{task.state.value}{step_info} [{pane_info}]")

        if not manager.active_tasks and not pick_next_task(skip_slugs=set(), manager=manager):
            print("Idle — no tasks")

        time.sleep(5)  # Poll every 5 seconds


# Keep old main for backwards compatibility (can be removed later)
def main():
    """Original sequential main loop (deprecated, use main_parallel)."""
    print("[DEPRECATED] Sequential main() is deprecated. Use main_parallel() for parallel execution.")
    print("[DEPRECATED] Running main_parallel() instead...")
    main_parallel()


if __name__ == "__main__":
    main_parallel()
