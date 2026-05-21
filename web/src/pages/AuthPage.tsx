import { useState, useEffect } from "preact/hooks";
import { route } from "preact-router";
import { api } from "../api";
import type { CurrentUser } from "../App";

interface AuthPageProps {
  path: string;
  currentUser: CurrentUser | null;
  onLogin: (user: CurrentUser, accessToken: string) => void;
}

interface InstanceInfo {
  name: string;
  setupRequired: boolean;
}

export function AuthPage({ currentUser, onLogin }: AuthPageProps) {
  const [setupRequired, setSetupRequired] = useState<boolean | null>(null);
  const [isSignup, setIsSignup] = useState(false);
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [nickname, setNickname] = useState("");
  const [error, setError] = useState("");
  const [instanceError, setInstanceError] = useState("");
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (currentUser) {
      route("/", true);
      return;
    }
    setInstanceError("");
    api<InstanceInfo>("/api/v1/instance")
      .then((data) => setSetupRequired(data.setupRequired))
      .catch((err) => {
        setInstanceError((err as Error).message || "无法读取实例状态");
        setSetupRequired(null);
      });
  }, [currentUser]);

  if (currentUser) return null;

  if (instanceError) {
    return (
      <div class="auth-page">
        <div class="auth-card">
          <h1 class="auth-title">无法连接实例</h1>
          <div class="form-error" style={{ marginBottom: "16px", textAlign: "center" }}>
            {instanceError}
          </div>
          <button
            class="btn btn-primary"
            type="button"
            style={{ width: "100%" }}
            onClick={() => window.location.reload()}
          >
            重新加载
          </button>
        </div>
      </div>
    );
  }

  if (setupRequired === null) {
    return (
      <div class="auth-page">
        <div class="auth-card">
          <div class="loading-screen" style={{ minHeight: "120px" }}>
            <span class="loading-spinner" />
          </div>
        </div>
      </div>
    );
  }

  const handleSubmit = async (e: Event) => {
    e.preventDefault();
    setError("");
    setLoading(true);

    try {
      let endpoint: string;
      const body: Record<string, string> = { username, password };

      if (setupRequired) {
        endpoint = "/api/v1/setup";
        if (nickname) body.nickname = nickname;
      } else if (isSignup) {
        endpoint = "/api/v1/auth/signup";
        if (nickname) body.nickname = nickname;
      } else {
        endpoint = "/api/v1/auth/signin";
      }

      const data = await api<{ accessToken: string; user: CurrentUser }>(
        endpoint,
        { method: "POST", body: JSON.stringify(body) }
      );

      onLogin(data.user, data.accessToken);
      route("/");
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setLoading(false);
    }
  };

  const title = setupRequired
    ? "创建管理员账号"
    : isSignup
    ? "注册新账号"
    : "登录";

  const btnText = loading
    ? "请稍候..."
    : setupRequired
    ? "创建账号"
    : isSignup
    ? "注册"
    : "登录";

  return (
    <div class="auth-page">
      <div class="auth-card">
        <h1 class="auth-title">{title}</h1>

        {error && <div class="form-error" style={{ marginBottom: "16px", textAlign: "center" }}>{error}</div>}

        <form onSubmit={handleSubmit}>
          <div class="form-group">
            <label class="form-label">用户名</label>
            <input
              class="form-input"
              type="text"
              value={username}
              onInput={(e) => setUsername((e.target as HTMLInputElement).value)}
              required
              autoComplete="username"
              autoFocus
            />
          </div>

          <div class="form-group">
            <label class="form-label">密码</label>
            <input
              class="form-input"
              type="password"
              value={password}
              onInput={(e) => setPassword((e.target as HTMLInputElement).value)}
              required
              autoComplete={(setupRequired || isSignup) ? "new-password" : "current-password"}
            />
          </div>

          {(setupRequired || isSignup) && (
            <div class="form-group">
              <label class="form-label">昵称（可选）</label>
              <input
                class="form-input"
                type="text"
                value={nickname}
                onInput={(e) => setNickname((e.target as HTMLInputElement).value)}
                autoComplete="nickname"
              />
            </div>
          )}

          <button
            class="btn btn-primary"
            type="submit"
            disabled={loading || !username || !password}
            style={{ width: "100%", marginTop: "8px" }}
          >
            {btnText}
          </button>
        </form>

        {!setupRequired && (
          <div style={{ textAlign: "center", marginTop: "16px" }}>
            <button
              class="btn-link"
              onClick={() => { setIsSignup(!isSignup); setError(""); }}
              style={{ background: "none", border: "none", color: "var(--primary)", cursor: "pointer", fontSize: "0.9rem" }}
            >
              {isSignup ? "已有账号？去登录" : "没有账号？去注册"}
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
