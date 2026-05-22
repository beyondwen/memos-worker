import type {
  BackupItem,
  BackupPreview,
  MigrationPreview,
  MigrationProgress,
  MigrationResult,
  TagItem,
} from "./settingsModel";
import { AiSettingsSection } from "./AiSettingsSection";
import { DataMaintenanceSection } from "./DataMaintenanceSection";
import { MigrationSection } from "./MigrationSection";
import { TagManagementSection } from "./TagManagementSection";

interface DataSettingsTabProps {
  backups: BackupItem[];
  backupCreating: boolean;
  backupPreview: BackupPreview | null;
  aiBaseUrl: string;
  aiModel: string;
  aiApiKey: string;
  aiConfigured: boolean;
  aiSaving: boolean;
  aiTesting: boolean;
  migrationBaseUrl: string;
  migrationToken: string;
  migrationIncludeArchived: boolean;
  migrationPreview: MigrationPreview | null;
  migrationResult: MigrationResult | null;
  migrationProgress: MigrationProgress | null;
  migrationPreviewing: boolean;
  migrationImporting: boolean;
  tags: TagItem[];
  tagFrom: string;
  tagTo: string;
  tagSaving: boolean;
  migrationProgressVisible: boolean;
  migrationKnownTotal: number;
  migrationProgressPercent: number | null;
  migrationProgressTitle: string;
  migrationProgressDetail: string;
  onExport: () => void;
  onImport: (event: Event) => void;
  onCreateBackup: () => void;
  onPreviewBackup: (backup: BackupItem) => void;
  onRestoreBackup: () => void;
  onAiBaseUrlChange: (value: string) => void;
  onAiModelChange: (value: string) => void;
  onAiApiKeyChange: (value: string) => void;
  onTestAiSettings: () => void;
  onSaveAiSettings: () => void;
  onMigrationBaseUrlChange: (value: string) => void;
  onMigrationTokenChange: (value: string) => void;
  onMigrationIncludeArchivedChange: (value: boolean) => void;
  onPreviewMigration: () => void;
  onRunMigration: () => void;
  onTagFromChange: (value: string) => void;
  onTagToChange: (value: string) => void;
  onRenameTag: (event: Event) => void;
}

export function DataSettingsTab({
  backups,
  backupCreating,
  backupPreview,
  aiBaseUrl,
  aiModel,
  aiApiKey,
  aiConfigured,
  aiSaving,
  aiTesting,
  migrationBaseUrl,
  migrationToken,
  migrationIncludeArchived,
  migrationPreview,
  migrationResult,
  migrationProgress,
  migrationPreviewing,
  migrationImporting,
  tags,
  tagFrom,
  tagTo,
  tagSaving,
  migrationProgressVisible,
  migrationKnownTotal,
  migrationProgressPercent,
  migrationProgressTitle,
  migrationProgressDetail,
  onExport,
  onImport,
  onCreateBackup,
  onPreviewBackup,
  onRestoreBackup,
  onAiBaseUrlChange,
  onAiModelChange,
  onAiApiKeyChange,
  onTestAiSettings,
  onSaveAiSettings,
  onMigrationBaseUrlChange,
  onMigrationTokenChange,
  onMigrationIncludeArchivedChange,
  onPreviewMigration,
  onRunMigration,
  onTagFromChange,
  onTagToChange,
  onRenameTag,
}: DataSettingsTabProps) {
  return (
    <>
      <DataMaintenanceSection
        backups={backups}
        backupCreating={backupCreating}
        backupPreview={backupPreview}
        onExport={onExport}
        onImport={onImport}
        onCreateBackup={onCreateBackup}
        onPreviewBackup={onPreviewBackup}
        onRestoreBackup={onRestoreBackup}
      />

      <AiSettingsSection
        aiBaseUrl={aiBaseUrl}
        aiModel={aiModel}
        aiApiKey={aiApiKey}
        aiConfigured={aiConfigured}
        aiSaving={aiSaving}
        aiTesting={aiTesting}
        onAiBaseUrlChange={onAiBaseUrlChange}
        onAiModelChange={onAiModelChange}
        onAiApiKeyChange={onAiApiKeyChange}
        onTestAiSettings={onTestAiSettings}
        onSaveAiSettings={onSaveAiSettings}
      />

      <MigrationSection
        migrationBaseUrl={migrationBaseUrl}
        migrationToken={migrationToken}
        migrationIncludeArchived={migrationIncludeArchived}
        migrationPreview={migrationPreview}
        migrationResult={migrationResult}
        migrationProgress={migrationProgress}
        migrationPreviewing={migrationPreviewing}
        migrationImporting={migrationImporting}
        migrationProgressVisible={migrationProgressVisible}
        migrationKnownTotal={migrationKnownTotal}
        migrationProgressPercent={migrationProgressPercent}
        migrationProgressTitle={migrationProgressTitle}
        migrationProgressDetail={migrationProgressDetail}
        onMigrationBaseUrlChange={onMigrationBaseUrlChange}
        onMigrationTokenChange={onMigrationTokenChange}
        onMigrationIncludeArchivedChange={onMigrationIncludeArchivedChange}
        onPreviewMigration={onPreviewMigration}
        onRunMigration={onRunMigration}
      />

      <TagManagementSection
        tags={tags}
        tagFrom={tagFrom}
        tagTo={tagTo}
        tagSaving={tagSaving}
        onTagFromChange={onTagFromChange}
        onTagToChange={onTagToChange}
        onRenameTag={onRenameTag}
      />
    </>
  );
}
