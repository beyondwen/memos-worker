import { useEffect, useState } from "preact/hooks";
import { route } from "preact-router";
import { isHeaderNavActive } from "../headerNav";
import { personalPrimaryNavItems } from "../personalMode";
import type { CurrentUser } from "../App";

interface HeaderProps {
  currentUser: CurrentUser | null;
  onLogout: () => void;
  activePath: string;
}

export function Header({ currentUser, onLogout, activePath }: HeaderProps) {
  const [menuOpen, setMenuOpen] = useState(false);
  const [localActivePath, setLocalActivePath] = useState(activePath);
  const displayName = currentUser?.nickname || currentUser?.username || "";
  const avatarInitial = displayName.trim().charAt(0).toUpperCase() || "M";

  useEffect(() => {
    setLocalActivePath(activePath);
  }, [activePath]);

  const nav = (href: string, label: string) => (
    <a
      href={href}
      class={isHeaderNavActive(localActivePath, href) ? "active" : ""}
      onClick={(e) => {
        e.preventDefault();
        setLocalActivePath(href);
        route(href);
        setMenuOpen(false);
      }}
    >
      {label}
    </a>
  );

  return (
    <header class="header">
      <a href="/" class="header-logo" onClick={(e) => { e.preventDefault(); setLocalActivePath("/"); route("/"); }}>
        Memos
      </a>

      <nav class={`header-nav${menuOpen ? " open" : ""}`}>
        {personalPrimaryNavItems(!!currentUser).map((item) =>
          nav(item.href, item.label)
        )}
      </nav>

      <div class="header-spacer" />

      {currentUser ? (
        <div class="header-user">
          <span class="header-avatar" aria-hidden="true">
            {avatarInitial}
          </span>
          <span class="header-user-name">
            {displayName}
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
