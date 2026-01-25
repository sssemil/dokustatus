"use client";

import { ReactNode, useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import { X } from "lucide-react";
import { zIndex } from "@/lib/design-tokens";

interface ModalProps {
  open: boolean;
  onClose: () => void;
  title: string;
  children: ReactNode;
  size?: "sm" | "md" | "lg";
  variant?: "default" | "danger";
  className?: string;
}

const sizeClasses = {
  sm: "max-w-sm",
  md: "max-w-md",
  lg: "max-w-lg",
};

const variantStyles = {
  default: {
    modal: "bg-zinc-900 border-zinc-700",
    header: "border-zinc-800",
    title: "text-white",
    closeButton: "text-zinc-400 hover:text-white hover:bg-zinc-800",
  },
  danger: {
    modal: "bg-red-950 border-red-800",
    header: "border-red-900",
    title: "text-red-100",
    closeButton: "text-red-300 hover:text-white hover:bg-red-900",
  },
};

export function Modal({
  open,
  onClose,
  title,
  children,
  size = "md",
  variant = "default",
  className = "",
}: ModalProps) {
  const modalRef = useRef<HTMLDivElement>(null);
  const previousActiveElement = useRef<HTMLElement | null>(null);

  useEffect(() => {
    if (open) {
      previousActiveElement.current = document.activeElement as HTMLElement;
      document.body.style.overflow = "hidden";

      // Focus the modal
      setTimeout(() => {
        modalRef.current?.focus();
      }, 0);
    } else {
      document.body.style.overflow = "";
      previousActiveElement.current?.focus();
    }

    return () => {
      document.body.style.overflow = "";
    };
  }, [open]);

  useEffect(() => {
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === "Escape" && open) {
        onClose();
      }
    };

    document.addEventListener("keydown", handleEscape);
    return () => document.removeEventListener("keydown", handleEscape);
  }, [open, onClose]);

  // Focus trap
  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key !== "Tab") return;

    const focusableElements = modalRef.current?.querySelectorAll(
      'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])',
    );

    if (!focusableElements?.length) return;

    const firstElement = focusableElements[0] as HTMLElement;
    const lastElement = focusableElements[
      focusableElements.length - 1
    ] as HTMLElement;

    if (e.shiftKey && document.activeElement === firstElement) {
      e.preventDefault();
      lastElement.focus();
    } else if (!e.shiftKey && document.activeElement === lastElement) {
      e.preventDefault();
      firstElement.focus();
    }
  };

  if (!open) return null;

  const modalContent = (
    <div
      className="fixed inset-0 flex items-center justify-center p-4"
      style={{ zIndex: zIndex.modal }}
    >
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-black/60 backdrop-blur-sm"
        onClick={onClose}
        aria-hidden="true"
      />

      {/* Modal */}
      <div
        ref={modalRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="modal-title"
        tabIndex={-1}
        onKeyDown={handleKeyDown}
        className={`
          relative border rounded-xl shadow-2xl
          w-full ${sizeClasses[size]} ${variantStyles[variant].modal} animate-scale-in
          ${className}
        `}
      >
        {/* Header */}
        <div
          className={`flex items-center justify-between p-4 border-b ${variantStyles[variant].header}`}
        >
          <h2
            id="modal-title"
            className={`text-lg font-semibold ${variantStyles[variant].title}`}
          >
            {title}
          </h2>
          <button
            onClick={onClose}
            className={`transition-colors p-1 rounded-md ${variantStyles[variant].closeButton}`}
          >
            <X size={20} />
          </button>
        </div>

        {/* Content */}
        <div className="p-4">{children}</div>
      </div>
    </div>
  );

  // Use portal to render at document root
  if (typeof document !== "undefined") {
    return createPortal(modalContent, document.body);
  }

  return null;
}
