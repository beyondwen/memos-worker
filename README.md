# Memos Worker

Cloudflare Workers + D1 + R2 版的轻量 Memos 实现。

## 当前功能

- 首次启动创建管理员
- 登录、登出、刷新会话、当前用户信息
- 修改密码，并撤销旧会话
- Memo 创建、列表、读取、编辑、归档
- 标签解析和按标签筛选
- R2 附件上传、绑定 memo、鉴权下载
- 管理员 JSON 导入/导出
- Durable Object SSE 推送
- 内置最小 Web UI

## 本地运行

```bash
npm install
npm run db:migrate:local
npm run dev
```

后端运行在 Rust Worker 上，本地构建需要先安装 Rust 工具链：

```bash
rustup target add wasm32-unknown-unknown
cargo install worker-build
```

本地开发需要 `.dev.vars`：

```bash
SERVER_SECRET=dev-only-change-me
```

打开 `http://127.0.0.1:8787`，首次进入会创建管理员账号。

## 验证

```bash
npm run typecheck
npm test
npm run build
```

`npm run build` 使用 `wrangler deploy --dry-run --outdir dist`，只验证打包，不会部署。
当前 Rust Worker 构建关闭了 `wasm-opt`，避免本地或 CI 在无法访问 GitHub binaryen release 时失败。

## 生产部署

1. 创建 D1 数据库并替换 `wrangler.toml` 中的 `database_id`。
2. 创建 R2 bucket，并确认绑定名为 `MEMOS_BUCKET`。
3. 设置生产密钥：

```bash
wrangler secret put SERVER_SECRET
```

4. 应用远端迁移并部署：

```bash
npm run db:migrate
wrangler deploy
```
