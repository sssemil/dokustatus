"use client";

import { useAuth } from "@reauth/sdk/react";
import { useRouter } from "next/navigation";
import { useEffect, useState, useCallback } from "react";
import { useToken } from "../token-context";

const DOMAIN = process.env.NEXT_PUBLIC_DOMAIN || "demo.test";

interface Todo {
  id: string;
  text: string;
  completed: boolean;
  createdAt: string;
}

export default function TodosPage() {
  const { user, loading, logout } = useAuth({ domain: DOMAIN });
  const { token, fetchToken, clearToken } = useToken();
  const router = useRouter();
  const [todos, setTodos] = useState<Todo[]>([]);
  const [newTodoText, setNewTodoText] = useState("");
  const [isLoading, setIsLoading] = useState(true);

  // Helper to make authenticated API calls with Bearer token
  const authFetch = useCallback(
    async (url: string, options: RequestInit = {}): Promise<Response> => {
      let currentToken = token;

      // If no token, try to fetch one
      if (!currentToken) {
        currentToken = await fetchToken();
      }

      if (!currentToken) {
        throw new Error("No token available");
      }

      const res = await fetch(url, {
        ...options,
        headers: {
          ...options.headers,
          Authorization: `Bearer ${currentToken}`,
        },
      });

      // If 401, try to refresh token and retry once
      if (res.status === 401) {
        const newToken = await fetchToken();
        if (newToken) {
          return fetch(url, {
            ...options,
            headers: {
              ...options.headers,
              Authorization: `Bearer ${newToken}`,
            },
          });
        }
      }

      return res;
    },
    [token, fetchToken]
  );

  // Redirect if not authenticated
  useEffect(() => {
    if (!loading && !user) {
      router.push("/");
    }
  }, [user, loading, router]);

  // Fetch todos
  const fetchTodos = useCallback(async () => {
    try {
      const res = await authFetch("/api/todos");
      if (res.ok) {
        const data = await res.json();
        setTodos(data);
      } else if (res.status === 401) {
        router.push("/");
      }
    } catch (err) {
      console.error("Failed to fetch todos:", err);
    } finally {
      setIsLoading(false);
    }
  }, [authFetch, router]);

  useEffect(() => {
    if (user && token) {
      fetchTodos();
    } else if (user && !token) {
      // Try to fetch token on page load (e.g., after refresh)
      fetchToken().then((t) => {
        if (!t) {
          router.push("/");
        }
      });
    }
  }, [user, token, fetchTodos, fetchToken, router]);

  // Add todo
  const addTodo = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newTodoText.trim()) return;

    try {
      const res = await authFetch("/api/todos", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ text: newTodoText }),
      });

      if (res.ok) {
        const todo = await res.json();
        setTodos([...todos, todo]);
        setNewTodoText("");
      }
    } catch (err) {
      console.error("Failed to add todo:", err);
    }
  };

  // Toggle todo
  const toggleTodo = async (id: string, completed: boolean) => {
    try {
      const res = await authFetch(`/api/todos/${id}`, {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ completed: !completed }),
      });

      if (res.ok) {
        setTodos(
          todos.map((t) => (t.id === id ? { ...t, completed: !completed } : t))
        );
      }
    } catch (err) {
      console.error("Failed to toggle todo:", err);
    }
  };

  // Delete todo
  const deleteTodo = async (id: string) => {
    try {
      const res = await authFetch(`/api/todos/${id}`, {
        method: "DELETE",
      });

      if (res.ok) {
        setTodos(todos.filter((t) => t.id !== id));
      }
    } catch (err) {
      console.error("Failed to delete todo:", err);
    }
  };

  // Handle logout
  const handleLogout = async () => {
    clearToken();
    await logout();
    router.push("/");
  };

  if (loading || !user) {
    return (
      <div style={styles.container}>
        <p>Loading...</p>
      </div>
    );
  }

  return (
    <div style={styles.container}>
      <header style={styles.header}>
        <div>
          <h1 style={styles.title}>My Todos</h1>
          <p style={styles.email}>{user.email}</p>
        </div>
        <div style={styles.headerButtons}>
          <button
            onClick={() => router.push("/account")}
            style={styles.accountButton}
          >
            Account
          </button>
          <button onClick={handleLogout} style={styles.logoutButton}>
            Sign out
          </button>
        </div>
      </header>

      <form onSubmit={addTodo} style={styles.form}>
        <input
          type="text"
          value={newTodoText}
          onChange={(e) => setNewTodoText(e.target.value)}
          placeholder="What needs to be done?"
          style={styles.input}
        />
        <button type="submit" style={styles.addButton}>
          Add
        </button>
      </form>

      {isLoading ? (
        <p style={styles.loading}>Loading todos...</p>
      ) : todos.length === 0 ? (
        <p style={styles.empty}>No todos yet. Add one above!</p>
      ) : (
        <ul style={styles.list}>
          {todos.map((todo) => (
            <li key={todo.id} style={styles.listItem}>
              <label style={styles.label}>
                <input
                  type="checkbox"
                  checked={todo.completed}
                  onChange={() => toggleTodo(todo.id, todo.completed)}
                  style={styles.checkbox}
                />
                <span
                  style={{
                    textDecoration: todo.completed ? "line-through" : "none",
                    color: todo.completed ? "#888" : "inherit",
                  }}
                >
                  {todo.text}
                </span>
              </label>
              <button
                onClick={() => deleteTodo(todo.id)}
                style={styles.deleteButton}
              >
                Delete
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  container: {
    maxWidth: "600px",
    margin: "40px auto",
    padding: "30px",
    backgroundColor: "white",
    borderRadius: "8px",
    boxShadow: "0 2px 4px rgba(0,0,0,0.1)",
  },
  header: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    marginBottom: "30px",
    paddingBottom: "20px",
    borderBottom: "1px solid #eee",
  },
  title: {
    margin: "0 0 5px 0",
    fontSize: "24px",
  },
  email: {
    margin: 0,
    color: "#666",
    fontSize: "14px",
  },
  headerButtons: {
    display: "flex",
    gap: "10px",
    alignItems: "center",
  },
  accountButton: {
    backgroundColor: "#0070f3",
    color: "white",
    border: "none",
    padding: "8px 16px",
    borderRadius: "4px",
    fontSize: "14px",
    cursor: "pointer",
  },
  logoutButton: {
    backgroundColor: "transparent",
    border: "1px solid #ddd",
    padding: "8px 16px",
    borderRadius: "4px",
    cursor: "pointer",
    color: "#666",
  },
  form: {
    display: "flex",
    gap: "10px",
    marginBottom: "20px",
  },
  input: {
    flex: 1,
    padding: "12px",
    fontSize: "16px",
    border: "1px solid #ddd",
    borderRadius: "4px",
  },
  addButton: {
    backgroundColor: "#0070f3",
    color: "white",
    border: "none",
    padding: "12px 24px",
    fontSize: "16px",
    borderRadius: "4px",
    cursor: "pointer",
  },
  list: {
    listStyle: "none",
    padding: 0,
    margin: 0,
  },
  listItem: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    padding: "12px",
    borderBottom: "1px solid #eee",
  },
  label: {
    display: "flex",
    alignItems: "center",
    gap: "10px",
    flex: 1,
    cursor: "pointer",
  },
  checkbox: {
    width: "18px",
    height: "18px",
    cursor: "pointer",
  },
  deleteButton: {
    backgroundColor: "transparent",
    border: "none",
    color: "#ff4444",
    cursor: "pointer",
    fontSize: "14px",
  },
  loading: {
    textAlign: "center",
    color: "#666",
  },
  empty: {
    textAlign: "center",
    color: "#888",
    padding: "40px",
  },
};
