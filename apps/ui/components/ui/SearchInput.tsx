import { InputHTMLAttributes, forwardRef } from 'react';
import { Search } from 'lucide-react';

interface SearchInputProps extends Omit<InputHTMLAttributes<HTMLInputElement>, 'type'> {
  value: string;
  onChange: (e: React.ChangeEvent<HTMLInputElement>) => void;
}

export const SearchInput = forwardRef<HTMLInputElement, SearchInputProps>(
  ({ value, onChange, placeholder = 'Search...', className = '', ...props }, ref) => {
    return (
      <div className={`relative ${className}`}>
        <Search
          size={16}
          className="absolute left-3 top-1/2 -translate-y-1/2 text-zinc-500"
        />
        <input
          ref={ref}
          type="text"
          value={value}
          onChange={onChange}
          placeholder={placeholder}
          className="
            w-full bg-zinc-900 border border-zinc-700 rounded-lg
            pl-9 pr-3 py-2 text-sm
            placeholder-zinc-500 text-white
            transition-all duration-200
            hover:border-zinc-600
            focus:outline-none focus:border-blue-500 focus:ring-2 focus:ring-blue-500/50
          "
          {...props}
        />
      </div>
    );
  }
);

SearchInput.displayName = 'SearchInput';
