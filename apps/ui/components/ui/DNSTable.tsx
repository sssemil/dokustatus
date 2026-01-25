"use client";

import { RefreshCw, CheckCircle, XCircle, Clock } from "lucide-react";
import { Badge } from "./Badge";
import { CodeBlock } from "./CodeBlock";
import { Button } from "./Button";

interface DNSRecord {
  type: string;
  name: string;
  value: string;
  verified: boolean;
}

interface DNSTableProps {
  records: DNSRecord[];
  lastChecked?: string;
  onRefresh?: () => void;
  refreshing?: boolean;
  className?: string;
}

export function DNSTable({
  records,
  lastChecked,
  onRefresh,
  refreshing = false,
  className = "",
}: DNSTableProps) {
  return (
    <div className={`space-y-3 ${className}`}>
      <div className="border border-zinc-700 rounded-lg overflow-x-auto">
        <table className="w-full min-w-[500px]">
          <thead>
            <tr className="bg-zinc-800/50 text-left text-xs text-zinc-400 uppercase tracking-wider border-b border-zinc-700">
              <th className="px-4 py-3 w-20">Type</th>
              <th className="px-4 py-3">Name</th>
              <th className="px-4 py-3">Value</th>
              <th className="px-4 py-3 w-24 text-center">Status</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-zinc-800">
            {records.map((record, idx) => (
              <tr
                key={idx}
                className="bg-zinc-900/50 hover:bg-zinc-900 transition-colors"
              >
                <td className="px-4 py-3">
                  <Badge>{record.type}</Badge>
                </td>
                <td className="px-4 py-3">
                  <CodeBlock value={record.name} />
                </td>
                <td className="px-4 py-3">
                  <CodeBlock value={record.value} />
                </td>
                <td className="px-4 py-3 text-center">
                  {record.verified ? (
                    <CheckCircle
                      className="text-emerald-400 inline"
                      size={18}
                    />
                  ) : (
                    <XCircle className="text-red-400 inline" size={18} />
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {(lastChecked || onRefresh) && (
        <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-2 text-sm">
          {lastChecked && (
            <span className="text-zinc-500 flex items-center gap-1">
              <Clock size={14} />
              Last checked: {lastChecked}
            </span>
          )}
          {onRefresh && (
            <Button
              variant="ghost"
              size="sm"
              onClick={onRefresh}
              loading={refreshing}
            >
              <RefreshCw size={14} />
              Check now
            </Button>
          )}
        </div>
      )}
    </div>
  );
}
