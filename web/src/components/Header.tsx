import { useState } from "preact/hooks";
import { route } from "preact-router";
import type { CurrentUser } from "../App";

interface HeaderProps {
  currentUser: CurrentUser | null;
  onLogout: () => void;
}

export function Header({ currentUser, onLogout }: HeaderProps) {
  const [menuOpen, setMenuOpen] = useState(false);
  const p = typeof window !== "undefined" ? window.location.pathname : "";

  const nav = (href: string, label: string) => (
    <a
      href={href}
      class={p === href ? "active" : ""}
      onClick={(e) => { e.preventDefault(); route(href); setMenuOpen(false); }}
    >
      {label}
    </a>
  );

  return (
    <header class="header">
      <a href="/" class="header-logo" onClick={(e) => { e.preventDefault(); route("/"); }}>
        Memos
      </a>

      <nav class={`header-nav${menuOpen ? " open" : ""}`}>
        {nav("/", "首页")}
        {nav("/explore", "发现")}
        {currentUser && nav("/settings", "设置")}
      </nav>

      <div class="header-spacer" />

      {currentUser ? (
        <div class="header-user">
          <span class="header-user-name">
            {currentUser.nickname || currentUser.username}
          </span>
          <button class="btn btn-ghost btn-sm" onClick={onLogout}>
            退出
          </button>
        </div>
      ) : (
        <a
          href="/auth"
          class="btn btn-primary btn-sm"
          onClick={(e) => { e.preventDefault(); route("/auth"); }}
        >
          登录
        </a>
      )}

      <button
        class="hamburger"
        onClick={() => setMenuOpen(!menuOpen)}
        aria-label="菜单"
      >
        {menuOpen ? "\u2715" : "\u2630"}
      </button>
    </header>
  );
}
