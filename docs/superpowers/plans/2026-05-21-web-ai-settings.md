# Web AI 设置实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 管理员可以在 Web 设置页配置 AI Base URL、Model、API Key，并让知识关联识别优先使用 Web 配置。

**架构：** 后端新增 `src/services/aiSettings.ts` 管理 `system_setting` 中的 AI 配置，API Key 不回显明文；`aiRelations.ts` 从 Web 配置读取运行时配置，未配置时回落到 Worker env；设置页数据页签新增 AI 设置卡片。

**技术栈：** Cloudflare Worker、D1、Preact、Vitest、TypeScript。

---

### 任务 1：AI 设置 helper

**文件：**
- 创建：`src/services/aiSettings.ts`
- 修改：`test/core.test.ts`

- [ ] 写失败测试：验证脱敏输出、空 API Key 更新时保留旧配置。
- [ ] 运行 `rtk npm test -- test/core.test.ts` 看到失败。
- [ ] 实现 `sanitizeAiSettingsForClient` 和 `mergeAiSettingsUpdate`。
- [ ] 再运行测试看到通过。

### 任务 2：后端 API

**文件：**
- 修改：`src/router.ts`
- 修改：`src/services/aiSettings.ts`
- 修改：`src/services/aiRelations.ts`

- [ ] 新增 `GET /api/v1/ai/settings`。
- [ ] 新增 `PATCH /api/v1/ai/settings`。
- [ ] 新增 `POST /api/v1/ai/settings/test`。
- [ ] `aiRelations.ts` 优先读取 Web 设置。

### 任务 3：前端设置

**文件：**
- 修改：`web/src/pages/SettingsPage.tsx`
- 修改：`web/src/style.css`

- [ ] 数据页签增加 AI 设置卡片。
- [ ] 展示 Base URL、Model、API Key 状态。
- [ ] 支持保存和测试连接。

### 任务 4：验证部署提交

**命令：**
- `rtk npm test`
- `rtk npm run typecheck`
- `rtk npm run build`
- `rtk npm run deploy`
- `rtk git add ...`
- `rtk git commit -m "feat(AI): 支持 Web 配置模型参数"`
- `rtk git push origin master`
