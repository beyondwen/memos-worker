# Memos → Cloudflare Workers 迁移设计文档

> 日期: 2026-05-20
> 状态: Draft
> 目标: 将 usememos/memos (Go) 完整迁移到 TypeScript + Cloudflare Workers

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
│  │  │  - 维护 WebSocket/SSE 长连接                  │   │   │
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
| **Web 框架** | Hono v4 | 轻量 (~14KB)、Workers 原生、内置 JWT/CORS 中间件 |
| **ORM** | Drizzle ORM | 类型安全、D1 支持好、查询构建灵活 |
| **认证** | jose v5 | JWS/JWT 纯 JS 实现，Workers 兼容 |
| **密码** | bcryptjs (WASM) 或 Workers Crypto API | bcrypt 用 WASM，scrypt 用原生 crypto |
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
  content_id INTEGER NOT NULL,
  reaction_type TEXT NOT NULL,
  UNIQUE (creator_id, content_id, reaction_type),
  FOREIGN KEY (creator_id) REFERENCES "user"(id) ON DELETE CASCADE
);

CREATE INDEX idx_reaction_content_id ON reaction(content_id);

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
1. 验证密码 (bcrypt) 或 SSO
2. 生成 access token (JWT, 15min)
3. 生成 refresh token (JWT, 30d) + 存 DB
4. Set-Cookie: memos_refresh=<refresh_token>; HttpOnly; Secure; SameSite=Lax
5. 返回 { accessToken, user }

请求认证:
1. Authorization: Bearer <token> → 验证 access token 或 PAT
2. 无 Bearer → 检查 Cookie → refresh token 轮转

Token 轮转:
1. 验证 refresh token 签名 + DB 检查未撤销
2. 删除旧 refresh token
3. 生成新 refresh token 对
4. 返回新 access token + 设置新 Cookie
```

### 密码策略
- 使用 `crypto.subtle` 的 PBKDF2 或 scrypt (Workers 原生支持)
- 不使用 bcrypt (需要 WASM, 增加 bundle 大小)

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

### Durable Object 设计

```typescript
// sse-hub.ts
export class SSEHub {
  private storage: DurableObjectStorage;
  private sessions: Map<string, WebSocket>;
  
  async fetch(request: Request): Promise<Response> {
    // WebSocket 升级
    const [client, server] = Object.values(new WebSocketPair());
    
    // 认证
    const token = new URL(request.url).searchParams.get("token");
    const claims = await this.validateToken(token);
    
    // 存储连接
    this.sessions.set(claims.sub, server);
    
    // 心跳
    const heartbeat = setInterval(() => {
      server.send(": heartbeat\n\n");
    }, 30000);
    
    server.addEventListener("close", () => {
      clearInterval(heartbeat);
      this.sessions.delete(claims.sub);
    });
    
    return new Response(null, { status: 101, webSocket: client });
  }
  
  // 广播事件 (从 API Worker 调用)
  async broadcast(event: SSEEvent): Promise<void> {
    for (const [userId, ws] of this.sessions) {
      if (this.canReceive(event, userId)) {
        ws.send(`data: ${JSON.stringify(event)}\n\n`);
      }
    }
  }
}
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

---

## 9. 文件存储设计

### R2 存储

```typescript
// attachment-service.ts
class AttachmentService {
  async upload(file: File, userId: number): Promise<Attachment> {
    // 生成唯一 key
    const key = `attachments/${Date.now()}_${crypto.randomUUID()}_${file.name}`;
    
    // 上传到 R2
    await env.MEMOS_BUCKET.put(key, file.stream(), {
      httpMetadata: { contentType: file.type },
    });
    
    // 存元数据到 D1
    const attachment = await this.d1
      .insert(attachmentTable)
      .values({
        uid: generateUID(),
        creatorId: userId,
        filename: file.name,
        type: file.type,
        size: file.size,
        storageType: "S3",
        reference: key,
      })
      .returning();
    
    return attachment;
  }
  
  async getPresignedUrl(attachment: Attachment): Promise<string> {
    // R2 公开 bucket 直接返回 URL
    return `${env.R2_PUBLIC_URL}/${attachment.reference}`;
  }
}
```

### 缩略图

使用 Cloudflare Images 或在 Worker 中用 `images` API 处理：

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
// Workers: 通过 Durable Objects WebSocket

// 方案 A: 继续使用 EventSource (DO 支持 HTTP SSE)
const eventSource = new EventSource("/api/v1/sse?token=" + accessToken);

// 方案 B: 改用 WebSocket
const ws = new WebSocket(`wss://sse-hub.${workerUrl}/connect?token=${accessToken}`);
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
compatibility_flags = ["nodejs_compat"]

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

# 环境变量
[vars]
SERVER_SECRET = "change-me-in-production"

# 生产环境
[env.production]
vars = { ENVIRONMENT = "production" }
```

---

## 12. 项目结构

```
memos-worker/
├── docs/
│   └── superpowers/specs/
│       └── 2026-05-20-memos-worker-design.md
├── src/                          # Worker 源码
│   ├── index.ts                  # 入口
│   ├── router.ts                 # 路由注册
│   ├── middleware/
│   │   ├── auth.ts               # 认证中间件
│   │   ├── cors.ts               # CORS
│   │   └── error.ts              # 错误处理
│   ├── services/
│   │   ├── auth-service.ts       # 认证逻辑
│   │   ├── memo-service.ts       # Memo CRUD
│   │   ├── user-service.ts       # 用户管理
│   │   ├── attachment-service.ts # 附件管理
│   │   ├── filter-engine.ts      # CEL 过滤
│   │   ├── markdown-service.ts   # Markdown 渲染
│   │   ├── rss-service.ts        # RSS 生成
│   │   └── webhook-service.ts    # Webhook
│   ├── models/
│   │   ├── user.ts
│   │   ├── memo.ts
│   │   ├── attachment.ts
│   │   └── ...
│   ├── db/
│   │   ├── schema.ts             # Drizzle schema
│   │   └── migrate.ts            # 迁移脚本
│   ├── durable-objects/
│   │   └── sse-hub.ts            # SSE DO
│   └── utils/
│       ├── jwt.ts                # JWT 工具
│       ├── password.ts           # 密码哈希
│       └── uid.ts                # UID 生成
├── web/                          # 前端 (从原版 memos/web 适配)
│   ├── src/
│   ├── package.json
│   └── vite.config.ts
├── migrations/                   # D1 migration 文件
│   └── 0001_initial.sql
├── wrangler.toml
├── package.json
├── tsconfig.json
└── README.md
```

---

## 13. 阶段实施计划

### Phase 0: 项目搭建 + Schema
- [ ] 初始化 Worker 项目 (Wrangler + Hono + Drizzle)
- [ ] 配置 D1 + KV + R2 bindings
- [ ] 迁移数据库 schema
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
- [ ] WebSocket 连接管理
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
| 前端 bundle 大小 | 超过 3MB/10MB 限制 | 代码分割 + lazy loading |

---

## 15. 成本估算

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
| `echo/v5` | Hono |
| `modernc.org/sqlite` | D1 (via Drizzle) |
| `google/cel-go` | 自研 filter-engine |
| `golang-jwt/jose` | jose |
| `goldmark` | marked |
| `gorilla/feeds` | 自研 RSS |
| `aws-sdk-go-v2/s3` | R2 (S3 兼容 API) |
| `sync.Map` (内存缓存) | Workers KV |

### B. 兼容性说明

- API 路径与原版保持一致，前端代码改动最小化
- 数据库 schema 与原版 SQLite schema 兼容
- JWT token 格式与原版兼容
- CEL 表达式语法与原版兼容
