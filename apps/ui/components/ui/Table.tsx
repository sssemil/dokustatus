import { ReactNode } from 'react';

interface Column<T> {
  key: string;
  header: string;
  render?: (item: T) => ReactNode;
  className?: string;
}

interface TableProps<T> {
  columns: Column<T>[];
  data: T[];
  keyExtractor: (item: T) => string;
  onRowClick?: (item: T) => void;
  emptyMessage?: string;
  className?: string;
}

export function Table<T extends Record<string, unknown>>({
  columns,
  data,
  keyExtractor,
  onRowClick,
  emptyMessage = 'No data',
  className = '',
}: TableProps<T>) {
  if (data.length === 0) {
    return (
      <div className="text-center py-8 text-zinc-500 text-sm">
        {emptyMessage}
      </div>
    );
  }

  return (
    <div className={`overflow-x-auto ${className}`}>
      <table className="w-full min-w-[500px]">
        <thead>
          <tr className="bg-zinc-800/50 text-left text-xs text-zinc-400 uppercase tracking-wider">
            {columns.map((column) => (
              <th
                key={column.key}
                className={`px-4 py-3 font-medium ${column.className || ''}`}
              >
                {column.header}
              </th>
            ))}
          </tr>
        </thead>
        <tbody className="divide-y divide-zinc-800">
          {data.map((item) => (
            <tr
              key={keyExtractor(item)}
              onClick={() => onRowClick?.(item)}
              className={`
                bg-zinc-900/50 transition-colors
                ${onRowClick ? 'hover:bg-zinc-900 cursor-pointer' : ''}
              `}
            >
              {columns.map((column) => (
                <td key={column.key} className={`px-4 py-3 ${column.className || ''}`}>
                  {column.render
                    ? column.render(item)
                    : (item[column.key] as ReactNode)}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
