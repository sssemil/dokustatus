import { CopyButton } from './CopyButton';

interface CodeBlockProps {
  value: string;
  className?: string;
  onCopy?: () => void;
}

export function CodeBlock({ value, className = '', onCopy }: CodeBlockProps) {
  return (
    <div
      className={`
        flex items-center justify-between
        bg-zinc-950 border border-zinc-800 rounded
        px-3 py-2 font-mono text-sm group
        ${className}
      `}
    >
      <code className="text-zinc-300 truncate mr-2">{value}</code>
      <CopyButton
        text={value}
        onCopy={onCopy}
        className="opacity-50 group-hover:opacity-100 flex-shrink-0"
      />
    </div>
  );
}
