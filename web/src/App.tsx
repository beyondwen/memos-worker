import { useState, useEffect, useCallback } from "preact/hooks";
import Router, { route } from "preact-router";
import { api, setToken, clearToken, setAuthExpiredHandler } from "./api";
import { Header } from "./components/Header";
import { Home } from "./pages/Home";
import { AuthPage } from "./pages/AuthPage";
import { MemoDetailPage } from "./pages/MemoDetailPage";
import { SettingsPage } from "./pages/SettingsPage";
import { ExplorePage } from "./pages/ExplorePage";
import { SharePage } from "./pages/SharePage";
import { InboxPage } from "./pages/InboxPage";
import { TimelinePage } from "./pages/TimelinePage";
import { FeedbackProvider, useFeedback } from "./components/Feedback";
import { buildAuthRedirectPath, currentRoutePath } from "./authFlow";

export interface CurrentUser {
  id: number;
  username: string;
  role: string;
  nickname: string;
  email: string;
  avatarUrl: string;
  description: string;
}

export function App() {
  return (
    <FeedbackProvider>
      <AppContent />
    </FeedbackProvider>
  );
}

function AppContent() {
  const { notify } = useFeedback();
  const [currentUser, setCurrentUser] = useState<CurrentUser | null>(null);
  const [authLoaded, setAuthLoaded] = useState(false);
  const [activePath, setActivePath] = useState(() => currentRoutePath());

  const checkAuth = useCallback(async () => {
    try {
      const data = await api<{ user: CurrentUser }>("/api/v1/auth/user");
      setCurrentUser(data.user);
    } catch (err) {
      console.warn("[auth] current user check failed:", err);
      setCurrentUser(null);
    } finally {
      setAuthLoaded(true);
    }
  }, []);

  useEffect(() => {
    checkAuth();
  }, [checkAuth]);

  useEffect(() => {
    setAuthExpiredHandler(() => {
      setCurrentUser(null);
      notify("登录已过期，请重新登录", "error");
      route(buildAuthRedirectPath(currentRoutePath()), true);
    });
    return () => setAuthExpiredHandler(null);
  }, [notify]);

  const handleLogin = useCallback((user: CurrentUser, accessToken: string) => {
    setToken(accessToken);
    setCurrentUser(user);
  }, []);

  const handleLogout = useCallback(async () => {
    try {
      await api("/api/v1/auth/signout", { method: "POST" });
    } catch (err) {
      console.warn("[auth] signout failed:", err);
    }
    clearToken();
    setCurrentUser(null);
    route("/");
  }, []);

  if (!authLoaded) {
    return (
      <div class="loading-screen">
        <span class="loading-spinner" />
      </div>
    );
  }

  return (
    <>
      <Header currentUser={currentUser} onLogout={handleLogout} activePath={activePath} />
      <Router onChange={(event) => setActivePath(event.url)}>
        <Home path="/" currentUser={currentUser} />
        <AuthPage
          path="/auth"
          currentUser={currentUser}
          onLogin={handleLogin}
        />
        <MemoDetailPage path="/memos/:uid" currentUser={currentUser} />
        <ExplorePage path="/explore" currentUser={currentUser} />
        <InboxPage path="/inbox" currentUser={currentUser} />
        <TimelinePage path="/timeline" currentUser={currentUser} />
        <SettingsPage path="/settings" currentUser={currentUser} />
        <SharePage path="/shares/:uid" />
      </Router>
    </>
  );
}
