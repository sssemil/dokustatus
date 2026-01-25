import { ReactNode } from "react";

interface FormFieldProps {
  label?: string;
  error?: string;
  helpText?: string;
  required?: boolean;
  children: ReactNode;
  className?: string;
}

export function FormField({
  label,
  error,
  helpText,
  required,
  children,
  className = "",
}: FormFieldProps) {
  return (
    <div className={`w-full ${className}`}>
      {label && (
        <label className="block text-sm font-medium text-zinc-300 mb-1.5">
          {label}
          {required && <span className="text-red-400 ml-1">*</span>}
        </label>
      )}
      {children}
      {error && <p className="mt-1 text-sm text-red-400">{error}</p>}
      {helpText && !error && (
        <p className="mt-1 text-xs text-zinc-500">{helpText}</p>
      )}
    </div>
  );
}
