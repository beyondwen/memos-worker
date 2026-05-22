# Memos Worker

Cloudflare Pages + Rust Worker + D1 + R2 版的轻量 Memos 实现。

## 当前功能

- 首次启动创建管理员
- 登录、登出、刷新会话、当前用户信息
- 修改密码，并撤销旧会话
- Memo 创建、列表、读取、编辑、归档、批量处理
- 标签解析和按标签筛选
- R2 附件上传、绑定 memo、鉴权下载
- 管理员 JSON 导入/导出
- 评论、引用关系
- Rust SSE 端点、D1 事件补偿、事件清理和迁移进度流
- 内置最小 Web UI

## 当前架构

- 后端入口是 `src/lib.rs`，已经切到 Rust Worker；历史 TypeScript Worker 后端不再作为运行路径保留。
- 前端入口在 `web/src`，构建产物由 Cloudflare Pages 托管。
- D1 负责业务数据和 SSE 补偿事件；R2 负责附件对象。
- `/api/v1/sse` 采用短连接事件流加 D1 补偿，客户端通过 `Last-Event-ID` 或 `since` 补拉事件。当前还没有 Durable Object 长连接广播。
- Memo、评论、引用关系和批量操作会写入 `memo_event`。
- 定时任务会先创建备份，再清理超过 7 天的 `memo_event`，避免补偿表无限增长。

## 本地运行

```bash
npm install
npm run db:migrate:local
npm run dev:api
```

后端运行在 Rust Worker 上，本地构建需要先安装 Rust 工具链：

```bash
rustup target add wasm32-unknown-unknown
cargo install worker-build
```

`scripts/build-worker.sh` 会自动尝试加载 `$HOME/.cargo/env`，常规 `npm run build` 和 `npm run deploy` 不需要手工 source Rust 环境。

本地开发需要 `.dev.vars`：

```bash
SERVER_SECRET=dev-only-change-me
```

另开一个终端启动前端：

```bash
npm run dev:web
```

打开 Vite 输出的本地地址，前端会把 `/api` 和 `/file` 代理到
`http://127.0.0.1:8787`。首次进入会创建管理员账号。

## 验证

```bash
npm run typecheck
npm test
npm run build
```

`npm run build` 会同时构建 Pages 静态资源和 Rust Worker dry-run，不会部署。
当前 Rust Worker 构建关闭了 `wasm-opt`，避免本地或 CI 在无法访问 GitHub binaryen release 时失败。

需要验证真实 HTTP 写链路时，可以先启动本地 Worker，再运行端到端冒烟脚本：

```bash
MEMOS_E2E_USERNAME=admin \
MEMOS_E2E_PASSWORD=your-password \
npm run test:e2e
```

脚本默认访问 `http://127.0.0.1:8787`，会创建临时 memo，覆盖评论、引用关系、批量操作和 SSE 事件，结束后自动清理。验证远端环境时设置 `MEMOS_E2E_BASE_URL`；如需保留测试数据，设置 `MEMOS_E2E_KEEP_DATA=1`。脚本会自动读取 `MEMOS_E2E_PROXY_URL`、`HTTPS_PROXY` 或 `HTTP_PROXY` 并通过代理访问远端环境。

需要让脚本先注册临时测试账号时，可以启用 `MEMOS_E2E_SIGNUP=1`。如果还希望脚本结束后删除该测试账号，需要提供管理员账号：

```bash
MEMOS_E2E_BASE_URL=https://memos-worker.example.workers.dev \
MEMOS_E2E_USERNAME=e2e_$(date +%s) \
MEMOS_E2E_PASSWORD=temporary-password \
MEMOS_E2E_SIGNUP=1 \
MEMOS_E2E_CLEANUP_USER=1 \
MEMOS_E2E_ADMIN_USERNAME=admin \
MEMOS_E2E_ADMIN_PASSWORD=admin-password \
npm run test:e2e
```

## 生产部署

1. 创建 D1 数据库并替换 `wrangler.toml` 中的 `database_id`。
2. 创建 R2 bucket，并确认绑定名为 `MEMOS_BUCKET`。
3. 设置生产密钥：

```bash
npm run pages:create
wrangler secret put SERVER_SECRET
```

4. 应用远端迁移并部署：

```bash
npm run db:migrate
npm run deploy
```

部署拆分为两部分：

- `npm run deploy:api`：部署 Rust Worker 后端，只承载 `/api/*`、`/file/*`、定时任务、D1 和 R2。
- `npm run deploy:web`：部署 `dist/static` 到 Cloudflare Pages，默认项目名为 `memos-worker-pages`。

自定义域名示例：

- `https://notes.example.com/*` 指向 Pages。
- DNS 需要一条 CNAME：`notes` -> `memos-worker-pages.pages.dev`，建议开启代理。
- Rust Worker Routes 负责 `notes.example.com/api/*` 和 `notes.example.com/file/*`。
- Pages 通过 `web/public/_redirects` 保留 SPA fallback；API 和文件路由由 Worker Routes 接管。

同域名部署时前端不需要额外配置。若临时使用不同域名，可在构建
Pages 前设置 `VITE_API_BASE_URL`，例如：

```bash
VITE_API_BASE_URL=https://memos-worker.example.workers.dev npm run build:web
```

## 文档状态

`docs/superpowers/` 下的设计稿和计划主要是历史迁移记录，其中早期文档可能仍描述 TypeScript Worker、Worker Assets 或 Durable Object 方案。当前实现以本 README 的 Pages + Rust Worker 架构为准。
