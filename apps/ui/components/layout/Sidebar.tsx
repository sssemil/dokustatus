"use client";

import { useEffect, useState } from "react";
import { usePathname } from "next/navigation";
import Link from "next/link";
import { Shield, LayoutDashboard, Globe, Settings, Menu } from "lucide-react";
import { UserMenu } from "./UserMenu";

interface SidebarProps {
  email: string;
  onLogout: () => void;
  onProfileClick: () => void;
}

const navItems = [
  {
    id: "dashboard",
    href: "/dashboard",
    icon: LayoutDashboard,
    label: "Dashboard",
  },
  { id: "domains", href: "/domains", icon: Globe, label: "Domains" },
  { id: "settings", href: "/settings", icon: Settings, label: "Settings" },
];

export function Sidebar({ email, onLogout, onProfileClick }: SidebarProps) {
  const pathname = usePathname();
  const [collapsed, setCollapsed] = useState(false);

  // Auto-collapse on narrow screens
  useEffect(() => {
    const checkWidth = () => {
      if (window.innerWidth < 768) {
        setCollapsed(true);
      }
    };

    checkWidth();
    window.addEventListener("resize", checkWidth);
    return () => window.removeEventListener("resize", checkWidth);
  }, []);

  const isActive = (href: string) => {
    if (href === "/dashboard") {
      return pathname === "/dashboard";
    }
    return pathname.startsWith(href);
  };

  return (
    <div
      className={`
        bg-zinc-900 border-r border-zinc-800 flex flex-col flex-shrink-0
        transition-all duration-300
        ${collapsed ? "w-16" : "w-56"}
      `}
    >
      {/* Logo */}
      <div
        className={`flex items-center gap-2 p-4 ${collapsed ? "justify-center" : ""}`}
      >
        <div className="w-8 h-8 bg-gradient-to-br from-blue-500 to-purple-600 rounded-lg flex items-center justify-center flex-shrink-0">
          <Shield size={18} className="text-white" />
        </div>
        {!collapsed && <span className="font-bold text-lg">reauth.dev</span>}
      </div>

      {/* Toggle button */}
      <button
        onClick={() => setCollapsed(!collapsed)}
        className="mx-2 mb-4 p-2 text-zinc-500 hover:text-white hover:bg-zinc-800 rounded-md transition-colors"
        title={collapsed ? "Expand sidebar" : "Collapse sidebar"}
      >
        <Menu size={18} />
      </button>

      {/* Navigation */}
      <nav className="flex-1 px-2 space-y-1">
        {navItems.map((item) => (
          <Link
            key={item.id}
            href={item.href}
            title={collapsed ? item.label : undefined}
            className={`
              w-full flex items-center gap-3 px-3 py-2 rounded-md text-sm
              transition-all duration-200
              ${
                isActive(item.href)
                  ? "bg-zinc-800 text-white"
                  : "text-zinc-400 hover:text-white hover:bg-zinc-800/50"
              }
              ${collapsed ? "justify-center" : ""}
            `}
          >
            <item.icon size={18} />
            {!collapsed && item.label}
          </Link>
        ))}
      </nav>

      {/* User menu */}
      <div
        className={`p-2 border-t border-zinc-800 ${collapsed ? "px-1" : ""}`}
      >
        <UserMenu
          email={email}
          collapsed={collapsed}
          onLogout={onLogout}
          onProfileClick={onProfileClick}
        />
      </div>
    </div>
  );
}
