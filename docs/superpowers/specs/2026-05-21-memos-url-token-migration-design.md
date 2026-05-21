# 原版 Memos 在线迁移设计

## 目标

在设置页增加“从原版 Memos 迁移”入口，允许管理员输入原版 Memos 地址和 Access Token，从原版 `/api/v1/memos` 分页读取备忘录并导入当前 Worker。

## 范围

- Token 只用于本次请求，不入库、不写审计详情、不写日志。
- 支持预检和正式导入两个动作。
- 第一版迁移备忘录正文、创建时间、更新时间、状态、可见性、置顶、标签。
- 附件和引用关系先保留原始元信息到 payload，不下载附件二进制，也不建立跨 memo 关系。
- 导入按原版 memo `name` 做幂等去重，重复执行时跳过已导入记录。

## API

- `POST /api/v1/migration/memos/preview`
- `POST /api/v1/migration/memos/import`

请求体：

```json
{
  "baseUrl": "https://memos.example.com",
  "accessToken": "token",
  "includeArchived": false
}
```

返回预检：

```json
{
  "preview": {
    "memoCount": 12,
    "attachmentCount": 2,
    "relationCount": 1,
    "archivedCount": 0,
    "truncated": false
  }
}
```

返回导入：

```json
{
  "result": {
    "imported": 12,
    "skipped": 0,
    "attachmentCount": 2,
    "relationCount": 1,
    "archivedCount": 0,
    "truncated": false
  }
}
```

## UI

设置页“数据”页签增加独立卡片：

- 原版 Memos 地址输入框。
- Access Token 密码输入框。
- “包含归档内容”复选框。
- “预检”和“开始迁移”按钮。
- 展示预检或导入结果。

## 测试

- 单元测试覆盖 URL 规范化、原版 memo 字段映射、统计汇总。
- 现有 `npm test`、`npm run typecheck`、`npm run build` 必须通过。
