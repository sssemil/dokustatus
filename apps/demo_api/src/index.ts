import express, { Request, Response, NextFunction } from 'express';
import { createServerClient } from '@reauth/sdk/server';
import type { User } from '@reauth/sdk';
import { Redis } from 'ioredis';
import { v4 as uuid } from 'uuid';

// Extend Express Request to include user
declare global {
  namespace Express {
    interface Request {
      user?: User;
    }
  }
}

// Types
interface Todo {
  id: string;
  text: string;
  completed: boolean;
  createdAt: string;
}

// Config
const PORT = process.env.PORT || 3003;
const REDIS_URL = process.env.REDIS_URL || 'redis://localhost:6379';
const DOMAIN = process.env.DOMAIN || 'demo.test';
const REAUTH_API_KEY = process.env.REAUTH_API_KEY;

// Clients
const redis = new Redis(REDIS_URL);
const reauth = createServerClient({
  domain: DOMAIN,
  apiKey: REAUTH_API_KEY,
});

// App
const app = express();
app.use(express.json());

// Auth middleware
// Supports two authentication methods:
// 1. Cookie-based: Forward cookies to reauth for session validation
// 2. Token-based: Bearer token in Authorization header, verified via API key
async function authMiddleware(req: Request, res: Response, next: NextFunction) {
  // Try Authorization header first (JWT token)
  const authHeader = req.headers.authorization;
  if (authHeader?.startsWith('Bearer ')) {
    const token = authHeader.slice(7);

    // Only use verifyToken if API key is configured
    if (REAUTH_API_KEY) {
      try {
        const result = await reauth.verifyToken(token);
        if (result.valid && result.user) {
          req.user = {
            id: result.user.id,
            email: result.user.email,
            roles: result.user.roles,
          };
          next();
          return;
        }
      } catch {
        // Token verification failed, fall through to cookie auth
      }
    }
  }

  // Fall back to cookie-based authentication
  const cookies = req.headers.cookie || '';
  const user = await reauth.getUser(cookies);

  if (!user) {
    res.status(401).json({ error: 'Unauthorized' });
    return;
  }

  req.user = user;
  next();
}

// Helper to get user's todos key
function todosKey(userId: string): string {
  return `todos:${userId}`;
}

// Helper to get/set todos from Redis
async function getTodos(userId: string): Promise<Todo[]> {
  const data = await redis.get(todosKey(userId));
  return data ? JSON.parse(data) : [];
}

async function setTodos(userId: string, todos: Todo[]): Promise<void> {
  await redis.set(todosKey(userId), JSON.stringify(todos));
}

// Routes

// Health check (public)
app.get('/api/health', (_req, res) => {
  res.json({ status: 'ok' });
});

// List todos
app.get('/api/todos', authMiddleware, async (req, res) => {
  const todos = await getTodos(req.user!.id);
  res.json(todos);
});

// Create todo
app.post('/api/todos', authMiddleware, async (req, res) => {
  const { text } = req.body;

  if (!text || typeof text !== 'string') {
    res.status(400).json({ error: 'text is required' });
    return;
  }

  const todos = await getTodos(req.user!.id);
  const newTodo: Todo = {
    id: uuid(),
    text: text.trim(),
    completed: false,
    createdAt: new Date().toISOString(),
  };

  todos.push(newTodo);
  await setTodos(req.user!.id, todos);

  res.status(201).json(newTodo);
});

// Update todo
app.put('/api/todos/:id', authMiddleware, async (req, res) => {
  const { id } = req.params;
  const { text, completed } = req.body;

  const todos = await getTodos(req.user!.id);
  const todoIndex = todos.findIndex((t) => t.id === id);

  if (todoIndex === -1) {
    res.status(404).json({ error: 'Todo not found' });
    return;
  }

  if (text !== undefined) {
    todos[todoIndex].text = String(text).trim();
  }
  if (completed !== undefined) {
    todos[todoIndex].completed = Boolean(completed);
  }

  await setTodos(req.user!.id, todos);
  res.json(todos[todoIndex]);
});

// Delete todo
app.delete('/api/todos/:id', authMiddleware, async (req, res) => {
  const { id } = req.params;

  const todos = await getTodos(req.user!.id);
  const todoIndex = todos.findIndex((t) => t.id === id);

  if (todoIndex === -1) {
    res.status(404).json({ error: 'Todo not found' });
    return;
  }

  todos.splice(todoIndex, 1);
  await setTodos(req.user!.id, todos);

  res.status(204).send();
});

// Start server
app.listen(PORT, () => {
  console.log(`Demo API running on http://localhost:${PORT}`);
});
