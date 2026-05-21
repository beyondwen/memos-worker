export interface MemoTemplate {
  id: "todo" | "meeting" | "study" | "bug" | "daily";
  label: string;
  content: string;
}

export const MEMO_TEMPLATES: MemoTemplate[] = [
  {
    id: "todo",
    label: "TODO",
    content: "## TODO\n- [ ] \n- [ ] \n\n备注：",
  },
  {
    id: "meeting",
    label: "会议纪要",
    content: "## 会议纪要\n时间：\n参与人：\n\n### 结论\n- \n\n### 待办\n- [ ] ",
  },
  {
    id: "study",
    label: "学习笔记",
    content: "## 学习笔记\n主题：\n\n### 核心概念\n- \n\n### 例子\n- \n\n### 下次复习\n- ",
  },
  {
    id: "bug",
    label: "Bug 记录",
    content: "## Bug 记录\n现象：\n\n复现步骤：\n1. \n\n原因：\n\n处理：\n- [ ] ",
  },
  {
    id: "daily",
    label: "日报",
    content: "## 日报\n### 今天完成\n- \n\n### 明天计划\n- \n\n### 风险\n- ",
  },
];

export function applyMemoTemplate(current: string, templateId: MemoTemplate["id"]): string {
  const template = MEMO_TEMPLATES.find((item) => item.id === templateId);
  if (!template) return current;
  const trimmed = current.trimEnd();
  return trimmed ? `${trimmed}\n\n${template.content}` : template.content;
}
