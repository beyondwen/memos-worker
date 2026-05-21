# AI 关联识别设计

## 目标

在备忘录详情页的知识关联区域加入 AI 自动识别能力。系统先用本地规则从大量备忘录中筛出小候选集，再让 AI 判断哪些候选真正相关，最后由用户确认保存到现有 `memo_relation` 表。

## 范围

- 第一版不全量扫描所有备忘录给 AI。
- 第一版不自动静默写入关系，用户点击保存后才写入。
- 第一版不新增向量库或 embedding 表。
- AI 失败时返回本地候选建议，功能仍可用。
- Token/API Key 只从 Worker secret/环境变量读取，不进入前端。

## 配置

- `AI_API_KEY`：必填，未配置时使用本地候选降级。
- `AI_BASE_URL`：可选，默认 `https://api.openai.com/v1`。
- `AI_MODEL`：可选，默认 `gpt-4o-mini`。

## 候选筛选

候选来源限定为当前用户可读备忘录，最多取 80 条最近正常备忘录，再按以下规则打分：

- 标签相同：高权重。
- 正文关键词重合：中权重。
- 最近更新：低权重。

最终最多发送 30 条候选给 AI。

## API

`POST /api/v1/memos/:uid/relations/suggest`

返回：

```json
{
  "suggestions": [
    {
      "memo": "memos/m_xxx",
      "content": "候选摘要",
      "reason": "都在讨论 Memos 迁移",
      "confidence": 0.82,
      "source": "ai"
    }
  ]
}
```

## UI

详情页“引用关系”区域调整为“知识关联”，增加“AI 识别关联”按钮。结果以推荐卡片展示，用户可以一键应用推荐，把推荐 memo 写入现有引用输入框并保存。
