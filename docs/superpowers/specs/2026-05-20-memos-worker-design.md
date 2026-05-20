# Memos → Cloudflare Workers 迁移设计文档

> 日期: 2026-05-20
> 状态: Draft
> 目标: 将 usememos/memos (Go) 分阶段迁移到 TypeScript + Cloudflare Workers

---

## 1. 概述

将 Memos 从 Go 单体应用迁移到 Cloudflare Workers 无状态边缘计算平台。这不是简单的移植，而是用 TypeScript 在 Workers 生态中重新实现所有功能。

### 核心约束
- **运行时**: Cloudflare Workers (V8 isolates, 无 Node.js API)
- **语言**: TypeScript
- **数据库**: D1 (SQLite 兼容)
- **文件存储**: R2 (S3 兼容)
- **实时通信**: Durable Objects (替代进程内 SSE)
- **缓存**: Workers KV

### 落地决策

第一版目标不是一次性复刻原版 Memos 的所有边角功能，而是先做一个可长期自用、可迁移数据、可继续扩展的 Worker 版本。

- **MVP 优先级**: 账号登录、Memo CRUD、基础可见性、附件上传下载、分页列表、标签解析、导入导出。
- **兼容策略**: API 路径尽量保持 `/api/v1/...`，但内部实现不追求 Go 代码结构一比一移植。
- **安全默认值**: Memo 默认 `PRIVATE`，附件默认不公开，所有下载先经过 Worker 鉴权。
- **实时协议**: 第一版保留浏览器 `EventSource` / SSE 体验，Durable Object 只负责连接管理和广播；WebSocket 作为后续增强。
- **部署形态**: 一个主 Worker 承载 API 和静态资源；Durable Object 是同一个 Worker 内的类绑定，不单独部署第二个公开 Worker。

---

## 2. 架构设计

```
┌─────────────────────────────────────────────────────────────┐
│                    Cloudflare Edge                           │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │              API Worker (Hono Router)                │   │
│  │                                                     │   │
│  │  /api/v1/auth/*    → AuthService                    │   │
│  │  /api/v1/memos/*   → MemoService                    │   │
│  │  /api/v1/users/*   → UserService                    │   │
│  │  /api/v1/attachments/* → AttachmentService          │   │
│  │  /api/v1/sse       → SSEProxy (→ DO)                │   │
│  │  /api/v1/rss/*     → RSSService                     │   │
│  │  /api/v1/webhook/* → WebhookService                 │   │
│  │  /file/*           → FileService (R2 proxy)          │   │
│  │  /*                → Static Assets (React SPA)       │   │
│  └─────────────┬───────────────────────────────────────┘   │
│                │                                            │
│  ┌─────────────┴───────────────────────────────────────┐   │
│  │           Durable Objects                            │   │
│  │  ┌─────────────────────────────────────────────┐   │   │
│  │  │  SSEHub DO                                   │   │   │
│  │  │  - 维护 SSE 长连接                            │   │   │
│  │  │  - 按 visibility 过滤广播                     │   │   │
│  │  │  - 心跳 30s                                  │   │   │
│  │  └─────────────────────────────────────────────┘   │   │
│  │  ┌─────────────────────────────────────────────┐   │   │
│  │  │  RealtimeChannel DO (per memo)               │   │   │
│  │  │  - 评论/Reaction 实时同步                     │   │   │
│  │  └─────────────────────────────────────────────┘   │   │
│  └────────────────────────────────────────────────────┘   │
│                                                             │
│  ┌────────────────────────────────────────────────────┐   │
│  │           Storage Bindings                          │   │
│  │  ┌──────┐  ┌──────┐  ┌──────┐  ┌──────────────┐  │   │
│  │  │  D1  │  │  KV  │  │  R2  │  │  DO Binding  │  │   │
│  │  └──────┘  └──────┘  └──────┘  └──────────────┘  │   │
│  └────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

### Worker 拆分

| Worker | 职责 | 路由 |
|--------|------|------|
| **main** | API + 静态资源 | `/api/*`, `/file/*`, `/*` |
| **sse-hub** | SSE 长连接管理 | DO 内部 |
| **scheduled** | 定时任务 (清理过期 share token 等) | Cron Trigger |

---

## 3. 技术选型

| 组件 | 选择 | 理由 |
|------|------|------|
| **Web 框架** | Web Standard API Router | MVP 阶段无运行时依赖，后续路由复杂后再引入 Hono |
| **ORM** | D1 prepared statements | MVP 阶段 SQL 显式可控，后续查询复杂后再引入 Drizzle |
| **认证** | Workers Crypto API | HMAC JWT、PBKDF2、哈希都使用 Workers 原生 Web Crypto |
| **密码** | Workers Crypto API (PBKDF2-SHA256) | 不引入 bcrypt WASM，降低 bundle 和冷启动成本 |
| **Markdown** | marked + DOMPurify | 前端渲染，goldmark 的 TS 等价物 |
| **CEL 过滤** | 自研 TS 解析器 | Go cel-go 无 TS 移植，需重写 |
| **验证** | Zod | 请求参数校验 |
| **测试** | Vitest + Miniflare | Workers 本地测试 |
| **构建** | Wrangler + Vite | Workers 部署 + 前端构建 |

---

## 4. 数据库设计 (D1)

### Schema

```sql
-- =============================================
-- Memos on Cloudflare D1 - Complete Schema
-- =============================================

-- 系统设置
CREATE TABLE IF NOT EXISTS system_setting (
  name TEXT PRIMARY KEY,
  value TEXT NOT NULL DEFAULT '',
  description TEXT NOT NULL DEFAULT ''
);

-- 实例设置
CREATE TABLE IF NOT EXISTS instance_setting (
  name TEXT PRIMARY KEY,
  value TEXT NOT NULL DEFAULT ''
);

-- 用户
CREATE TABLE IF NOT EXISTS "user" (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  created_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  updated_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  row_status TEXT NOT NULL DEFAULT 'NORMAL' CHECK(row_status IN ('NORMAL', 'ARCHIVED')),
  username TEXT UNIQUE NOT NULL,
  role TEXT NOT NULL DEFAULT 'USER' CHECK(role IN ('ADMIN', 'USER')),
  email TEXT NOT NULL DEFAULT '',
  nickname TEXT NOT NULL DEFAULT '',
  password_hash TEXT NOT NULL DEFAULT '',
  avatar_url TEXT NOT NULL DEFAULT '',
  description TEXT NOT NULL DEFAULT ''
);

CREATE INDEX idx_user_username ON "user"(username);
CREATE INDEX idx_user_row_status ON "user"(row_status);

-- 用户设置
CREATE TABLE IF NOT EXISTS user_setting (
  user_id INTEGER NOT NULL,
  key TEXT NOT NULL,
  value TEXT NOT NULL DEFAULT '',
  PRIMARY KEY (user_id, key),
  FOREIGN KEY (user_id) REFERENCES "user"(id) ON DELETE CASCADE
);

-- 登录会话 / Refresh Token
-- refresh_token 明文只返回给客户端，DB 仅保存哈希，便于撤销和轮转。
CREATE TABLE IF NOT EXISTS user_session (
  id TEXT PRIMARY KEY,
  user_id INTEGER NOT NULL,
  refresh_token_hash TEXT NOT NULL UNIQUE,
  created_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  updated_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  last_used_ts INTEGER,
  expires_ts INTEGER NOT NULL,
  row_status TEXT NOT NULL DEFAULT 'NORMAL' CHECK(row_status IN ('NORMAL', 'REVOKED')),
  user_agent TEXT NOT NULL DEFAULT '',
  ip_address TEXT NOT NULL DEFAULT '',
  FOREIGN KEY (user_id) REFERENCES "user"(id) ON DELETE CASCADE
);

CREATE INDEX idx_user_session_user_id ON user_session(user_id);
CREATE INDEX idx_user_session_expires_ts ON user_session(expires_ts);

-- Personal Access Token
-- token 明文只在创建时展示一次，DB 保存带前缀的哈希用于 Bearer token 验证。
CREATE TABLE IF NOT EXISTS user_access_token (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  user_id INTEGER NOT NULL,
  name TEXT NOT NULL,
  token_prefix TEXT NOT NULL,
  token_hash TEXT NOT NULL UNIQUE,
  created_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  updated_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  last_used_ts INTEGER,
  expires_ts INTEGER,
  row_status TEXT NOT NULL DEFAULT 'NORMAL' CHECK(row_status IN ('NORMAL', 'REVOKED')),
  FOREIGN KEY (user_id) REFERENCES "user"(id) ON DELETE CASCADE
);

CREATE INDEX idx_user_access_token_user_id ON user_access_token(user_id);
CREATE INDEX idx_user_access_token_prefix ON user_access_token(token_prefix);

-- 用户身份 (SSO)
CREATE TABLE IF NOT EXISTS user_identity (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  user_id INTEGER NOT NULL,
  provider TEXT NOT NULL,
  extern_uid TEXT NOT NULL,
  created_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  updated_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  UNIQUE (provider, extern_uid),
  UNIQUE (user_id, provider),
  FOREIGN KEY (user_id) REFERENCES "user"(id) ON DELETE CASCADE
);

-- 身份提供者
CREATE TABLE IF NOT EXISTS identity_provider (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  type TEXT NOT NULL,
  identifier_filter TEXT NOT NULL DEFAULT '',
  config TEXT NOT NULL DEFAULT '{}'
);

-- Memo
CREATE TABLE IF NOT EXISTS memo (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  uid TEXT UNIQUE NOT NULL,
  creator_id INTEGER NOT NULL,
  created_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  updated_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  row_status TEXT NOT NULL DEFAULT 'NORMAL' CHECK(row_status IN ('NORMAL', 'ARCHIVED')),
  content TEXT NOT NULL DEFAULT '',
  visibility TEXT NOT NULL DEFAULT 'PRIVATE' CHECK(visibility IN ('PUBLIC', 'PROTECTED', 'PRIVATE')),
  pinned INTEGER NOT NULL DEFAULT 0,
  payload TEXT NOT NULL DEFAULT '{}',
  FOREIGN KEY (creator_id) REFERENCES "user"(id) ON DELETE CASCADE
);

CREATE INDEX idx_memo_creator_id ON memo(creator_id);
CREATE INDEX idx_memo_visibility ON memo(visibility);
CREATE INDEX idx_memo_row_status ON memo(row_status);
CREATE INDEX idx_memo_created_ts ON memo(created_ts);
CREATE INDEX idx_memo_updated_ts ON memo(updated_ts);
CREATE INDEX idx_memo_pinned ON memo(pinned);

-- Memo 关系 (评论/引用)
CREATE TABLE IF NOT EXISTS memo_relation (
  memo_id INTEGER NOT NULL,
  related_memo_id INTEGER NOT NULL,
  type TEXT NOT NULL CHECK(type IN ('REFERENCE', 'COMMENT')),
  PRIMARY KEY (memo_id, related_memo_id, type),
  FOREIGN KEY (memo_id) REFERENCES memo(id) ON DELETE CASCADE,
  FOREIGN KEY (related_memo_id) REFERENCES memo(id) ON DELETE CASCADE
);

CREATE INDEX idx_memo_relation_related ON memo_relation(related_memo_id);

-- 附件
CREATE TABLE IF NOT EXISTS attachment (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  uid TEXT UNIQUE NOT NULL,
  creator_id INTEGER NOT NULL,
  created_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  updated_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  filename TEXT NOT NULL,
  blob BLOB,
  type TEXT NOT NULL DEFAULT '',
  size INTEGER NOT NULL DEFAULT 0,
  memo_id INTEGER,
  storage_type TEXT NOT NULL DEFAULT 'DATABASE' CHECK(storage_type IN ('DATABASE', 'LOCAL', 'S3')),
  reference TEXT NOT NULL DEFAULT '',
  payload TEXT NOT NULL DEFAULT '{}',
  FOREIGN KEY (creator_id) REFERENCES "user"(id) ON DELETE CASCADE,
  FOREIGN KEY (memo_id) REFERENCES memo(id) ON DELETE SET NULL
);

CREATE INDEX idx_attachment_creator_id ON attachment(creator_id);
CREATE INDEX idx_attachment_memo_id ON attachment(memo_id);

-- 反应 (emoji)
CREATE TABLE IF NOT EXISTS reaction (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  created_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  creator_id INTEGER NOT NULL,
  content_type TEXT NOT NULL DEFAULT 'MEMO' CHECK(content_type IN ('MEMO')),
  content_id INTEGER NOT NULL,
  reaction_type TEXT NOT NULL,
  UNIQUE (creator_id, content_type, content_id, reaction_type),
  FOREIGN KEY (creator_id) REFERENCES "user"(id) ON DELETE CASCADE
);

CREATE INDEX idx_reaction_content ON reaction(content_type, content_id);

-- Memo 分享
CREATE TABLE IF NOT EXISTS memo_share (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  uid TEXT UNIQUE NOT NULL,
  memo_id INTEGER NOT NULL,
  creator_id INTEGER NOT NULL,
  created_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  expires_ts INTEGER,
  FOREIGN KEY (memo_id) REFERENCES memo(id) ON DELETE CASCADE,
  FOREIGN KEY (creator_id) REFERENCES "user"(id) ON DELETE CASCADE
);

CREATE INDEX idx_memo_share_uid ON memo_share(uid);
CREATE INDEX idx_memo_share_expires_ts ON memo_share(expires_ts);

-- 通知收件箱
CREATE TABLE IF NOT EXISTS inbox (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  created_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  sender_id INTEGER,
  receiver_id INTEGER NOT NULL,
  status TEXT NOT NULL DEFAULT 'UNREAD' CHECK(status IN ('UNREAD', 'READ')),
  message TEXT NOT NULL DEFAULT '{}',
  FOREIGN KEY (sender_id) REFERENCES "user"(id) ON DELETE SET NULL,
  FOREIGN KEY (receiver_id) REFERENCES "user"(id) ON DELETE CASCADE
);

CREATE INDEX idx_inbox_receiver ON inbox(receiver_id, status);

-- Webhook
CREATE TABLE IF NOT EXISTS webhook (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  created_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  updated_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  row_status TEXT NOT NULL DEFAULT 'NORMAL',
  creator_id INTEGER NOT NULL,
  name TEXT NOT NULL,
  url TEXT NOT NULL,
  FOREIGN KEY (creator_id) REFERENCES "user"(id) ON DELETE CASCADE
);

-- 快捷方式
CREATE TABLE IF NOT EXISTS shortcut (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  creator_id INTEGER NOT NULL,
  title TEXT NOT NULL DEFAULT '',
  payload TEXT NOT NULL DEFAULT '{}',
  row_status TEXT NOT NULL DEFAULT 'NORMAL',
  created_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  updated_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  FOREIGN KEY (creator_id) REFERENCES "user"(id) ON DELETE CASCADE
);
```

---

## 5. API 设计

### 认证

所有需要认证的端点通过 `Authorization: Bearer <token>` 或 Cookie 验证。

```
POST   /api/v1/auth/signin          → 登录
POST   /api/v1/auth/signout         → 登出
POST   /api/v1/auth/refresh         → 刷新 token
GET    /api/v1/auth/user            → 获取当前用户
```

### Memo

```
POST   /api/v1/memos                → 创建 memo
GET    /api/v1/memos                → 列表 memos
       ?page_size=50
       &page_token=<cursor>
       &filter=<CEL expression>
       &order_by=created_ts desc
       &state=NORMAL|ARCHIVED
GET    /api/v1/memos/:name          → 获取单个 memo (name = memos/{uid})
PATCH  /api/v1/memos/:name          → 更新 memo
DELETE /api/v1/memos/:name          → 删除 memo

POST   /api/v1/memos/:name/comments → 创建评论
GET    /api/v1/memos/:name/comments → 列出评论

GET    /api/v1/memos/:name/relations → 获取关联

POST   /api/v1/memos/:name/reactions → 创建反应
DELETE /api/v1/memos/:name/reactions/:id → 删除反应

POST   /api/v1/memos/:name/shares   → 创建分享链接
GET    /api/v1/memos/:name/shares   → 列出分享链接
DELETE /api/v1/memos/:name/shares/:id → 删除分享链接

GET    /api/v1/shares/:token        → 通过分享 token 访问 memo
```

### User

```
GET    /api/v1/users                → 列出用户
GET    /api/v1/users/:name          → 获取用户
PATCH  /api/v1/users/:name          → 更新用户
DELETE /api/v1/users/:name          → 删除用户

POST   /api/v1/users/:name/access-tokens   → 创建 PAT
GET    /api/v1/users/:name/access-tokens   → 列出 PAT
DELETE /api/v1/users/:name/access-tokens/:id → 删除 PAT

GET    /api/v1/users/:name/setting   → 获取用户设置
PATCH  /api/v1/users/:name/setting   → 更新用户设置
```

### Attachment

```
POST   /api/v1/attachments          → 上传附件
GET    /api/v1/attachments          → 列出附件
GET    /api/v1/attachments/:name    → 获取附件元数据
PATCH  /api/v1/attachments/:name    → 更新附件
DELETE /api/v1/attachments/:name    → 删除附件
POST   /api/v1/attachments:batchDelete → 批量删除
```

### Other

```
GET    /api/v1/sse                  → SSE 实时推送
GET    /api/v1/instance             → 实例信息
GET    /api/v1/instance/stats       → 实例统计
GET    /api/v1/identityProviders    → 列出 IdP
POST   /api/v1/identityProviders    → 创建 IdP
GET    /api/v1/inbox                → 通知收件箱
GET    /api/v1/shortcuts            → 快捷方式
GET    /api/v1/link/metadata        → 链接元数据抓取
GET    /api/v1/explore/rss.xml      → RSS feed
GET    /api/v1/u/:username/rss.xml  → 用户 RSS feed
POST   /api/v1/import/memos         → 导入 Memos JSON / SQLite 转换结果
GET    /api/v1/export/memos         → 导出当前实例数据

GET    /file/attachments/:uid/:filename → 附件下载
GET    /file/users/:identifier/avatar   → 头像
```

---

## 6. 认证机制设计

### JWT Token 结构

```typescript
// Access Token (15 分钟有效)
interface AccessTokenClaims {
  type: "access";
  role: "ADMIN" | "USER";
  status: "NORMAL" | "ARCHIVED";
  username: string;
  iss: "memos";
  aud: ["user.access-token"];
  sub: string;       // user ID
  iat: number;
  exp: number;
}

// Refresh Token (30 天有效)
interface RefreshTokenClaims {
  type: "refresh";
  tid: string;       // token ID (用于撤销)
  iss: "memos";
  aud: ["user.refresh-token"];
  sub: string;       // user ID
  iat: number;
  exp: number;
}
```

### 认证流程

```
登录:
1. 验证密码 (PBKDF2-SHA256) 或 SSO
2. 生成 access token (JWT, 15min)
3. 生成 refresh token (JWT, 30d) + `user_session` 记录
4. Set-Cookie: memos_refresh=<refresh_token>; HttpOnly; Secure; SameSite=Lax
5. 返回 { accessToken, user }

请求认证:
1. Authorization: Bearer <token> → 验证 access token 或 PAT
2. 无 Bearer → 检查 Cookie → refresh token 轮转

Token 轮转:
1. 验证 refresh token 签名
2. 对 token 明文做 SHA-256 哈希，查询 `user_session.refresh_token_hash`
3. 检查 session 未撤销、未过期、用户未归档
4. 将旧 session 标记为 `REVOKED`
5. 生成新 refresh token 和新 session
6. 返回新 access token + 设置新 Cookie

PAT 验证:
1. Bearer token 以 `memos_pat_` 前缀识别
2. 先用短前缀定位候选记录，再比较 token 哈希
3. 检查 token 未撤销、未过期、用户未归档
4. 更新 `last_used_ts`，并以对应用户身份执行请求
```

### 密码策略
- 使用 `crypto.subtle` 的 `PBKDF2` + `SHA-256`
- 每个用户独立 16 字节随机 salt
- 默认迭代次数: 310000
- 存储格式: `pbkdf2_sha256$310000$<salt_base64>$<hash_base64>`
- 登录时支持根据存储格式升级哈希参数；升级只在密码验证成功后发生
- 不使用 bcrypt，避免 WASM 依赖和 Workers bundle 体积增加

### Cookie 与密钥

- `memos_refresh` 使用 `HttpOnly; Secure; SameSite=Lax; Path=/api/v1/auth`
- 生产环境的 `SERVER_SECRET` 必须通过 `wrangler secret put SERVER_SECRET` 配置，不写入 `wrangler.toml`
- 本地开发使用 `.dev.vars`，不得提交真实密钥
- JWT 签名算法使用 `HS256`；如果后续引入多服务信任边界，再升级到非对称签名

---

## 7. CEL 过滤引擎设计

原版使用 Go 的 `cel-go` 库。TypeScript 需要自研解析器。

### 支持的字段

| 字段 | 类型 | 存储 |
|------|------|------|
| `content` | string | memo.content |
| `creator` | string | JOIN user.username |
| `creator_id` | int | memo.creator_id |
| `created_ts` | timestamp | memo.created_ts |
| `updated_ts` | timestamp | memo.updated_ts |
| `pinned` | bool | memo.pinned |
| `visibility` | string | memo.visibility |
| `tag` / `tags` | string[] | memo.payload.tags (JSON) |
| `has_task_list` | bool | memo.payload.property.hasTaskList (JSON) |
| `has_link` | bool | memo.payload.property.hasLink (JSON) |
| `has_code` | bool | memo.payload.property.hasCode (JSON) |
| `has_incomplete_tasks` | bool | memo.payload.property.hasIncompleteTasks (JSON) |

### 实现方案

```typescript
// 三阶段管线: Parse → Normalize → Render SQL

class FilterEngine {
  // 1. Parse: 字符串 → AST
  parse(expression: string): ASTNode;
  
  // 2. Normalize: AST → IR (中间表示)
  normalize(ast: ASTNode): IR;
  
  // 3. Render: IR → SQL WHERE 子句
  render(ir: IR): { sql: string: params: unknown[] };
}

// 示例
const engine = new FilterEngine();
const result = engine.parse('has_task_list && visibility == "PUBLIC"');
// → { sql: "(json_extract(memo.payload, '$.property.hasTaskList') = 1) AND (memo.visibility = ?)", params: ["PUBLIC"] }
```

### 实现优先级
- Phase 3 先实现基础运算符: `==`, `!=`, `<`, `<=`, `>`, `>=`, `&&`, `||`, `!`, `in`
- 后续补充: `.contains()`, `.startsWith()`, `.endsWith()`, `now()`, `size()`

---

## 8. SSE 实时推送设计

原版使用进程内 `SSEHub`。Workers 无状态，需要 Durable Objects。

第一版明确采用 **HTTP SSE + EventSource**，不把浏览器端改成 WebSocket。原因是原版前端改动更小，并且 `/api/v1/sse` 的兼容性更好。Durable Object 内部维护 SSE stream writer；WebSocket 可作为后续优化。

### Durable Object 设计

```typescript
// sse-hub.ts
export class SSEHub {
  private storage: DurableObjectStorage;
  private sessions: Map<string, {
    userId: number;
    role: "ADMIN" | "USER";
    writer: WritableStreamDefaultWriter<Uint8Array>;
  }>;
  
  async fetch(request: Request): Promise<Response> {
    // 认证
    const token = new URL(request.url).searchParams.get("token");
    const claims = await this.validateToken(token);
    
    const sessionId = crypto.randomUUID();
    const { readable, writable } = new TransformStream();
    const writer = writable.getWriter();
    const encoder = new TextEncoder();
    
    await writer.write(encoder.encode(`event: ready\ndata: {}\n\n`));
    this.sessions.set(sessionId, {
      userId: Number(claims.sub),
      role: claims.role,
      writer,
    });

    const heartbeat = setInterval(() => {
      writer.write(encoder.encode(`: heartbeat\n\n`)).catch(() => {
        this.sessions.delete(sessionId);
        clearInterval(heartbeat);
      });
    }, 30000);
    
    return new Response(readable, {
      headers: {
        "Content-Type": "text/event-stream; charset=utf-8",
        "Cache-Control": "no-cache, no-transform",
        "Connection": "keep-alive",
      },
    });
  }
  
  // 广播事件 (从 API Worker 调用)
  async broadcast(event: SSEEvent): Promise<void> {
    const encoder = new TextEncoder();
    const chunk = encoder.encode(`event: ${event.type}\ndata: ${JSON.stringify(event)}\n\n`);

    for (const [sessionId, session] of this.sessions) {
      if (this.canReceive(event, session.userId, session.role)) {
        await session.writer.write(chunk).catch(() => {
          this.sessions.delete(sessionId);
        });
      }
    }
  }
}
```

### API Worker 代理

```typescript
app.get("/api/v1/sse", authMiddleware, async (c) => {
  const id = c.env.SSE_HUB.idFromName("global");
  const stub = c.env.SSE_HUB.get(id);
  const token = await createShortLivedSseToken(c.get("user"));
  return stub.fetch(new Request(`https://sse-hub/connect?token=${token}`));
});
```

### 事件类型

```typescript
type SSEEventType = 
  | "memo.created"
  | "memo.updated"
  | "memo.deleted"
  | "memo.comment.created"
  | "reaction.upserted"
  | "reaction.deleted";

interface SSEEvent {
  type: SSEEventType;
  name: string;        // e.g., "memos/abc123"
  visibility: "PUBLIC" | "PROTECTED" | "PRIVATE";
  creatorId: number;
}
```

### 可见性过滤

```typescript
function canReceive(event: SSEEvent, userId: number, userRole: string): boolean {
  if (event.visibility !== "PRIVATE") return true;
  if (userRole === "ADMIN") return true;
  return event.creatorId === userId;
}
```

### 断线与降级

- 前端使用 `EventSource("/api/v1/sse")`，由浏览器自动重连。
- 服务端事件必须带 `id`，后续可支持 `Last-Event-ID` 补偿。
- 如果 DO 不可用，核心 CRUD 不受影响，只降级为手动刷新列表。

---

## 9. 文件存储设计

### R2 存储

R2 bucket 默认不公开。附件下载统一走 `/file/attachments/:uid/:filename`，由 Worker 根据 memo 可见性、分享 token、当前用户身份做权限判断，再从 R2 读取对象并流式返回。

```typescript
// attachment-service.ts
class AttachmentService {
  async upload(file: File, userId: number): Promise<Attachment> {
    await assertAllowedFile(file);

    const uid = generateUID();
    const safeName = sanitizeFilename(file.name);
    const key = `attachments/${userId}/${uid}/${safeName}`;
    
    await env.MEMOS_BUCKET.put(key, file.stream(), {
      httpMetadata: {
        contentType: file.type || "application/octet-stream",
        contentDisposition: `inline; filename="${safeName}"`,
      },
      customMetadata: {
        creatorId: String(userId),
        originalFilename: file.name,
      },
    });
    
    const attachment = await this.d1
      .insert(attachmentTable)
      .values({
        uid,
        creatorId: userId,
        filename: safeName,
        type: file.type,
        size: file.size,
        storageType: "S3",
        reference: key,
      })
      .returning();
    
    return attachment;
  }
  
  async download(attachment: Attachment, viewer: Viewer): Promise<Response> {
    await assertCanReadAttachment(attachment, viewer);

    const object = await env.MEMOS_BUCKET.get(attachment.reference);
    if (!object) return new Response("Not found", { status: 404 });

    return new Response(object.body, {
      headers: {
        "Content-Type": attachment.type || "application/octet-stream",
        "Cache-Control": attachmentIsPublic(attachment) ? "public, max-age=3600" : "private, no-store",
      },
    });
  }
}
```

### 上传限制

- 单文件大小第一版限制为 25MB，避免 Worker 请求体和 CPU 预算风险。
- 图片、音频、PDF、纯文本可预览；未知类型按下载处理。
- 文件名只作为展示字段，R2 key 使用 `userId/uid/safeName`，避免路径穿越和重名覆盖。
- 公开 memo 的附件也不直接暴露 R2 公网地址，仍由 Worker 代理，便于之后收紧权限。

### 缩略图

缩略图第一版只做原图返回和浏览器端缩放。后续再接 Cloudflare Images 或 Image Resizing，避免 MVP 依赖额外服务。

```typescript
// 使用 Cloudflare Image Resizing
const resizeUrl = `https://imagedelivery.net/${accountId}/${imageId}/w=600`;
```

---

## 10. 前端适配

### 路由调整

原版使用 React Router，基本保持不变，只需调整 API base URL：

```typescript
// 原版: 相对路径 /api/v1/...
// Workers: 同域部署，继续使用相对路径
const API_BASE = "/api/v1";
```

### SSE 连接调整

```typescript
// 原版: EventSource 直接连 /api/v1/sse
// Workers: 继续暴露同一路径，API Worker 内部代理到 Durable Object
// 生产环境优先依赖 HttpOnly access cookie；本地开发可带 access_token 查询参数。
const eventSource = new EventSource("/api/v1/sse?access_token=" + encodeURIComponent(accessToken));
```

### 构建配置

```typescript
// vite.config.ts
export default defineConfig({
  base: "./",  // 相对路径
  build: {
    outDir: "../dist",  // 输出到 Worker 的静态资源目录
  },
});
```

---

## 11. Wrangler 配置

```toml
# wrangler.toml
name = "memos-worker"
main = "src/index.ts"
compatibility_date = "2026-05-20"
# 默认不启用 nodejs_compat；依赖必须优先选择 Web API / Workers 原生实现。
# compatibility_flags = ["nodejs_compat"]

# D1
[[d1_databases]]
binding = "DB"
database_name = "memos-db"
database_id = "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"

# KV
[[kv_namespaces]]
binding = "CACHE"
id = "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"

# R2
[[r2_buckets]]
binding = "MEMOS_BUCKET"
bucket_name = "memos-storage"

# Durable Objects
[durable_objects]
bindings = [
  { name = "SSE_HUB", class_name = "SSEHub" },
]

[[migrations]]
tag = "v1"
new_sqlite_classes = ["SSEHub"]

# 环境变量
[vars]
ENVIRONMENT = "development"

# 生产环境
[env.production]
vars = { ENVIRONMENT = "production" }
```

生产密钥通过 Wrangler secrets 注入：

```bash
wrangler secret put SERVER_SECRET
```

---

## 12. 项目结构

```
memos-worker/
├── docs/
│   └── superpowers/specs/
│       └── 2026-05-20-memos-worker-design.md
├── src/                          # Worker 源码
│   └── index.ts                  # Worker 入口、API、SSE DO、最小前端
├── test/
│   └── core.test.ts              # 密码、payload、文件名清理测试
├── migrations/                   # D1 migration 文件
│   └── 0001_initial.sql
├── wrangler.toml
├── package.json
├── tsconfig.json
└── README.md
```

---

## 13. 阶段实施计划

### MVP 验收标准

- 首次启动时可创建管理员账号。
- 管理员可登录、登出、刷新会话、修改个人资料。
- 可创建、编辑、归档、删除、置顶 memo。
- 列表支持分页、按创建时间倒序、按可见性过滤、按标签过滤。
- 可上传附件，并且私有 memo 的附件不能被未授权用户下载。
- 可从原 Memos 导入 memo、附件元数据和创建时间；可导出为 JSON。
- 前端可在同域 Worker 上打开，不依赖额外后端服务。
- `npm test` 和本地 `wrangler dev` 基础冒烟通过。

### Phase 0: 项目搭建 + Schema
- [ ] 初始化 Worker 项目 (Wrangler + Hono + Drizzle)
- [ ] 配置 D1 + KV + R2 bindings
- [ ] 迁移数据库 schema
- [ ] 初始化管理员创建流程
- [ ] 搭建 CI/CD (GitHub Actions → Wrangler deploy)

### Phase 1: 认证
- [ ] JWT 生成/验证
- [ ] 登录/登出/刷新 token
- [ ] 密码哈希 (PBKDF2)
- [ ] PAT 管理
- [ ] 认证中间件

### Phase 2: Memo CRUD
- [ ] Memo 创建/读取/更新/删除
- [ ] Markdown 渲染 (前端 marked)
- [ ] Payload 解析 (tags, property, location)
- [ ] 权限检查 (visibility)
- [ ] 导入/导出基础数据

### Phase 3: 过滤 + 分页
- [ ] CEL 过滤引擎 (基础运算符)
- [ ] 分页 (cursor-based)
- [ ] 排序 (AIP-132)

### Phase 4: 附件
- [ ] 附件上传 (R2)
- [ ] 附件下载/预览
- [ ] 缩略图
- [ ] 附件关联 memo

### Phase 5: 社交功能
- [ ] 评论
- [ ] Reaction (emoji)
- [ ] Memo 关联

### Phase 6: 实时推送
- [ ] SSEHub Durable Object
- [ ] EventSource/SSE 连接管理
- [ ] 事件广播 + 可见性过滤

### Phase 7: 高级功能
- [ ] 分享链接
- [ ] RSS feed
- [ ] Webhook
- [ ] 通知收件箱

### Phase 8: 前端 + 部署
- [ ] 前端 API 适配
- [ ] 构建优化
- [ ] 生产环境部署
- [ ] 自定义域名 + HTTPS

---

## 14. 风险与缓解

| 风险 | 影响 | 缓解 |
|------|------|------|
| D1 单线程限制 | 高并发写入排队 | 批量操作 + 队列 |
| CEL 引擎复杂度高 | 开发周期长 | 先实现基础功能，逐步扩展 |
| Worker 128MB 内存限制 | 大文件处理受限 | 流式处理 + R2 直传 |
| Worker 10ms CPU (Free) | 复杂查询超时 | 优化查询 + Paid plan |
| Durable Object 冷启动 | SSE 连接延迟 | 保持最小活跃实例 |
| 附件权限绕过 | 私有内容泄露 | R2 bucket 默认私有，统一 Worker 鉴权代理 |
| Refresh token 泄露 | 账号长期被盗用 | DB 只存哈希，轮转即撤销旧 session |
| 前端 bundle 大小 | 超过 3MB/10MB 限制 | 代码分割 + lazy loading |

---

## 15. 成本估算

以下为个人使用量级的粗估，实际部署前需要以 Cloudflare 当前价格和账号套餐为准。

| 资源 | Free Plan | Paid Plan |
|------|-----------|-----------|
| Workers 请求 | 100K/天 | $5/月 (10M 包含) |
| D1 | 500MB, 5M 行读/天 | $5/月起 |
| KV | 100K 读/天 | $5/月起 |
| R2 | 10GB 存储 | $0.015/GB/月 |
| Durable Objects | 包含 | 按请求计费 |
| **总计 (个人使用)** | **$0/月** | **~$10-15/月** |

---

## 附录

### A. 原版 Go → TypeScript 映射

| Go 包 | TypeScript 等价 |
|-------|----------------|
| `echo/v5` | Web Standard API Router；后续可切 Hono |
| `modernc.org/sqlite` | D1 prepared statements；后续可加 Drizzle |
| `google/cel-go` | 自研 filter-engine |
| `golang-jwt/jose` | Workers Crypto API + HS256 JWT |
| `goldmark` | marked |
| `gorilla/feeds` | 自研 RSS |
| `aws-sdk-go-v2/s3` | R2 (S3 兼容 API) |
| `sync.Map` (内存缓存) | Workers KV |

### B. 兼容性说明

- API 路径与原版保持一致，前端代码改动最小化
- 数据库核心业务字段尽量兼容原版 SQLite；认证会话和 R2 存储字段按 Workers 重新设计
- JWT claim 语义与原版保持接近，但签发、轮转、撤销按本项目实现
- CEL 表达式第一版只兼容常用基础语法，复杂函数分阶段补齐
