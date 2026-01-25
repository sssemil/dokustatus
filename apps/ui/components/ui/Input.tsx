import { InputHTMLAttributes, forwardRef } from "react";

interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  label?: string;
  error?: string;
  helpText?: string;
}

export const Input = forwardRef<HTMLInputElement, InputProps>(
  ({ label, error, helpText, className = "", id, ...props }, ref) => {
    const inputId = id || label?.toLowerCase().replace(/\s+/g, "-");

    return (
      <div className="w-full">
        {label && (
          <label
            htmlFor={inputId}
            className="block text-sm font-medium text-zinc-300 mb-1.5"
          >
            {label}
          </label>
        )}
        <input
          ref={ref}
          id={inputId}
          className={`
            w-full bg-zinc-900 border rounded-lg px-3 py-2 text-sm
            placeholder-zinc-500 text-white
            transition-all duration-200
            focus:outline-none focus:ring-2 focus:ring-blue-500/50
            ${error ? "border-red-500" : "border-zinc-700 hover:border-zinc-600 focus:border-blue-500"}
            ${className}
          `}
          {...props}
        />
        {error && <p className="mt-1 text-sm text-red-400">{error}</p>}
        {helpText && !error && (
          <p className="mt-1 text-xs text-zinc-500">{helpText}</p>
        )}
      </div>
    );
  },
);

Input.displayName = "Input";
