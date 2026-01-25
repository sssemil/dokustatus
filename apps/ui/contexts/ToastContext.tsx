"use client";

import {
  createContext,
  useContext,
  useState,
  useCallback,
  ReactNode,
} from "react";
import { createPortal } from "react-dom";
import { CheckCircle, XCircle, Info, X } from "lucide-react";
import { zIndex } from "@/lib/design-tokens";

type ToastType = "success" | "error" | "info";

interface Toast {
  id: number;
  message: string;
  type: ToastType;
}

interface ToastContextValue {
  addToast: (message: string, type?: ToastType) => void;
}

const ToastContext = createContext<ToastContextValue | null>(null);

export function useToast() {
  const context = useContext(ToastContext);
  if (!context) {
    throw new Error("useToast must be used within a ToastProvider");
  }
  return context;
}

interface ToastProviderProps {
  children: ReactNode;
}

export function ToastProvider({ children }: ToastProviderProps) {
  const [toasts, setToasts] = useState<Toast[]>([]);

  const addToast = useCallback(
    (message: string, type: ToastType = "success") => {
      const id = Date.now();
      setToasts((prev) => [...prev, { id, message, type }]);

      // Auto-dismiss after 3 seconds
      setTimeout(() => {
        setToasts((prev) => prev.filter((t) => t.id !== id));
      }, 3000);
    },
    [],
  );

  const removeToast = useCallback((id: number) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  const toastIcons: Record<ToastType, ReactNode> = {
    success: <CheckCircle size={18} />,
    error: <XCircle size={18} />,
    info: <Info size={18} />,
  };

  const toastStyles: Record<ToastType, string> = {
    success: "bg-emerald-600 text-white",
    error: "bg-red-600 text-white",
    info: "bg-zinc-700 text-white",
  };

  return (
    <ToastContext.Provider value={{ addToast }}>
      {children}
      {typeof document !== "undefined" &&
        createPortal(
          <div
            className="fixed bottom-4 right-4 flex flex-col gap-2"
            style={{ zIndex: zIndex.toast }}
            role="region"
            aria-label="Notifications"
          >
            {toasts.map((toast) => (
              <div
                key={toast.id}
                role="alert"
                aria-live="polite"
                className={`
                  flex items-center gap-2 px-4 py-3 rounded-lg shadow-lg
                  animate-slide-up min-w-[200px] max-w-[400px]
                  ${toastStyles[toast.type]}
                `}
              >
                {toastIcons[toast.type]}
                <span className="flex-1 text-sm">{toast.message}</span>
                <button
                  onClick={() => removeToast(toast.id)}
                  className="text-white/70 hover:text-white transition-colors p-0.5"
                  aria-label="Dismiss notification"
                >
                  <X size={14} />
                </button>
              </div>
            ))}
          </div>,
          document.body,
        )}
    </ToastContext.Provider>
  );
}
