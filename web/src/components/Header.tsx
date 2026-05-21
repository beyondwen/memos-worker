import { useCallback, useEffect, useState } from "preact/hooks";
import { route } from "preact-router";
import { api, getToken } from "../api";
import { createMemoEventSource, shouldRefreshForSseEvent } from "../sseEvents";
import { isHeaderNavActive } from "../headerNav";
import type { CurrentUser } from "../App";

interface HeaderProps {
  currentUser: CurrentUser | null;
  onLogout: () => void;
  activePath: string;
}

export function Header({ currentUser, onLogout, activePath }: HeaderProps) {
  const [menuOpen, setMenuOpen] = useState(false);
  const [localActivePath, setLocalActivePath] = useState(activePath);
  const [unreadCount, setUnreadCount] = useState(0);
  const displayName = currentUser?.nickname || currentUser?.username || "";
  const avatarInitial = displayName.trim().charAt(0).toUpperCase() || "M";

  useEffect(() => {
    setLocalActivePath(activePath);
  }, [activePath]);

  const refreshInbox = useCallback(async () => {
    if (!currentUser) {
      setUnreadCount(0);
      return;
    }
    try {
      const data = await api<{ unreadCount: number }>("/api/v1/inbox");
      setUnreadCount(data.unreadCount || 0);
    } catch {
      // ignore
    }
  }, [currentUser]);

  useEffect(() => {
    refreshInbox();
    const listener = () => refreshInbox();
    window.addEventListener("memos:inbox-refresh", listener);
    return () => window.removeEventListener("memos:inbox-refresh", listener);
  }, [refreshInbox]);

  useEffect(() => {
    if (!currentUser) return;
    const source = createMemoEventSource(getToken());
    if (!source) return;
    const refresh = (message: MessageEvent) => {
      try {
        const event = JSON.parse(message.data);
        if (shouldRefreshForSseEvent(event)) refreshInbox();
      } catch {
        // ignore
      }
    };
    source.addEventListener("memo.comment.created", refresh);
    return () => source.close();
  }, [currentUser, refreshInbox]);

  const nav = (href: string, label: string, badge?: number) => (
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
      {!!badge && <span class="nav-badge">{badge > 99 ? "99+" : badge}</span>}
    </a>
  );

  return (
    <header class="header">
      <a href="/" class="header-logo" onClick={(e) => { e.preventDefault(); setLocalActivePath("/"); route("/"); }}>
        Memos
      </a>

      <nav class={`header-nav${menuOpen ? " open" : ""}`}>
        {nav("/", "首页")}
        {nav("/explore", "发现")}
        {currentUser && nav("/timeline", "时间线")}
        {currentUser && nav("/inbox", "通知", unreadCount)}
        {currentUser && nav("/settings", "设置")}
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
