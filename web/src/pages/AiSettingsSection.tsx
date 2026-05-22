interface AiSettingsSectionProps {
  aiBaseUrl: string;
  aiModel: string;
  aiApiKey: string;
  aiConfigured: boolean;
  aiSaving: boolean;
  aiTesting: boolean;
  onAiBaseUrlChange: (value: string) => void;
  onAiModelChange: (value: string) => void;
  onAiApiKeyChange: (value: string) => void;
  onTestAiSettings: () => void;
  onSaveAiSettings: () => void;
}

export function AiSettingsSection({
  aiBaseUrl,
  aiModel,
  aiApiKey,
  aiConfigured,
  aiSaving,
  aiTesting,
  onAiBaseUrlChange,
  onAiModelChange,
  onAiApiKeyChange,
  onTestAiSettings,
  onSaveAiSettings,
}: AiSettingsSectionProps) {
  return (
    <div class="settings-section">
      <h2>AI 设置</h2>
      <div class="ai-settings-grid">
        <div class="form-group">
          <label class="form-label">API Base URL</label>
          <input
            class="form-input"
            type="url"
            value={aiBaseUrl}
            onInput={(e) => onAiBaseUrlChange((e.target as HTMLInputElement).value)}
            placeholder="https://api.openai.com/v1"
          />
        </div>
        <div class="form-group">
          <label class="form-label">模型</label>
          <input
            class="form-input"
            value={aiModel}
            onInput={(e) => onAiModelChange((e.target as HTMLInputElement).value)}
            placeholder="gpt-4o-mini"
          />
        </div>
        <div class="form-group ai-key-field">
          <label class="form-label">API Key</label>
          <input
            class="form-input"
            type="password"
            value={aiApiKey}
            onInput={(e) => onAiApiKeyChange((e.target as HTMLInputElement).value)}
            placeholder={aiConfigured ? "已配置，留空则不修改" : "请输入 API Key"}
            autoComplete="off"
          />
        </div>
      </div>
      <div class="settings-actions">
        <button class="btn btn-secondary" onClick={onTestAiSettings} disabled={aiTesting || !aiBaseUrl.trim() || !aiModel.trim()}>
          {aiTesting ? "测试中..." : "测试连接"}
        </button>
        <button class="btn btn-primary" onClick={onSaveAiSettings} disabled={aiSaving || !aiBaseUrl.trim() || !aiModel.trim()}>
          {aiSaving ? "保存中..." : "保存 AI 设置"}
        </button>
      </div>
      <div class="migration-summary">
        <span>{aiConfigured ? "API Key 已配置" : "API Key 未配置"}</span>
        <span>{aiModel || "未设置模型"}</span>
      </div>
      <div class="muted-line">
        API Key 不会回显；留空保存时会保留已有 Key。
      </div>
    </div>
  );
}
