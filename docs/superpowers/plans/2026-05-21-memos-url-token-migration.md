# 原版 Memos 在线迁移实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 管理员可以通过原版 Memos URL + Access Token 在线迁移备忘录。

**架构：** 新增 `src/services/migration.ts` 负责远端分页、字段映射、预检统计和导入写库；`src/router.ts` 暴露预检与导入 API；`web/src/pages/SettingsPage.tsx` 在数据页签提供表单入口。

**技术栈：** Cloudflare Worker、D1、Preact、Vitest、TypeScript。

---

### 任务 1：迁移映射测试

**文件：**
- 修改：`test/core.test.ts`
- 创建：`src/services/migration.ts`

- [ ] **步骤 1：编写失败的测试**

```ts
expect(normalizeMemosBaseUrl("https://demo.usememos.com/")).toBe("https://demo.usememos.com");
expect(mapOriginalMemoToImport({ name: "memos/1", content: "hi", createTime: "2026-05-21T00:00:00Z" }, 1).content).toBe("hi");
```

- [ ] **步骤 2：运行测试验证失败**

运行：`rtk npm test -- test/core.test.ts`
预期：FAIL，提示迁移函数未导出或行为未实现。

- [ ] **步骤 3：实现最少映射代码**

实现 URL 清理、时间解析、状态/可见性规范化、payload 元信息保留。

- [ ] **步骤 4：运行测试验证通过**

运行：`rtk npm test -- test/core.test.ts`
预期：PASS。

### 任务 2：后端迁移 API

**文件：**
- 修改：`src/router.ts`
- 修改：`src/services/migration.ts`

- [ ] **步骤 1：实现 `previewOriginalMemosMigration`**

校验管理员权限，分页请求原版 `/api/v1/memos?pageSize=1000`，统计 memo、附件、引用、归档数量。

- [ ] **步骤 2：实现 `importOriginalMemos`**

复用分页结果，按 `payload.source.originalName` 去重，插入当前账号的 memo。

- [ ] **步骤 3：接入路由**

新增 `/api/v1/migration/memos/preview` 和 `/api/v1/migration/memos/import`。

### 任务 3：设置页表单

**文件：**
- 修改：`web/src/pages/SettingsPage.tsx`
- 修改：`web/src/style.css`

- [ ] **步骤 1：增加状态和事件处理**

增加 URL、Token、includeArchived、preview/result、loading 状态。

- [ ] **步骤 2：增加数据页 UI**

在“数据维护”下方添加“从原版 Memos 迁移”卡片。

- [ ] **步骤 3：补充样式**

复用 settings-section 样式，新增轻量结果摘要样式。

### 任务 4：验证、部署、提交

**命令：**
- `rtk npm test`
- `rtk npm run typecheck`
- `rtk npm run build`
- `rtk npm run deploy`
- `rtk git add ...`
- `rtk git commit -m "feat(迁移): 支持原版 Memos 在线导入"`
- `rtk git push origin master`
