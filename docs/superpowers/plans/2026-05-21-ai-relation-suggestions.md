# AI 关联识别实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 在备忘录详情页加入 AI 自动推荐知识关联能力。

**架构：** `src/services/aiRelations.ts` 负责候选筛选、OpenAI-compatible 调用和结果解析；`src/router.ts` 暴露 suggest API；`RelationPanel` 调用 suggest API 并让用户确认保存到现有关系。

**技术栈：** Cloudflare Worker、D1、Preact、Vitest、TypeScript。

---

### 任务 1：候选筛选与解析

**文件：**
- 创建：`src/services/aiRelations.ts`
- 修改：`test/core.test.ts`

- [ ] 编写失败测试：验证候选数量限制、标签/关键词优先、AI JSON 解析。
- [ ] 运行 `rtk npm test -- test/core.test.ts` 看到失败。
- [ ] 实现 `rankRelationCandidates`、`parseAiRelationSuggestions`。
- [ ] 再运行同一测试看到通过。

### 任务 2：后端 API

**文件：**
- 修改：`src/types.ts`
- 修改：`src/router.ts`
- 修改：`src/services/aiRelations.ts`

- [ ] 增加 `AI_API_KEY`、`AI_BASE_URL`、`AI_MODEL` 环境字段。
- [ ] 实现 `suggestMemoRelations(request, env, viewer, uid)`。
- [ ] 在 memo relations 路由下增加 `/relations/suggest`。

### 任务 3：前端入口

**文件：**
- 修改：`web/src/components/RelationPanel.tsx`
- 修改：`web/src/relationView.ts`
- 修改：`web/src/style.css`

- [ ] 增加推荐类型和 API 调用。
- [ ] 展示“AI 识别关联”按钮、推荐列表和“应用推荐”按钮。
- [ ] 将标题从“引用关系”调整为“知识关联”。

### 任务 4：验证、部署、提交

**命令：**
- `rtk npm test`
- `rtk npm run typecheck`
- `rtk npm run build`
- `rtk npm run deploy`
- `rtk git add ...`
- `rtk git commit -m "feat(AI): 支持自动识别知识关联"`
- `rtk git push origin master`
