import type {
  BackupItem,
  BackupPreview,
  MigrationPreview,
  MigrationProgress,
  MigrationResult,
  OriginalBackupResult,
  RelationRebuildProgress,
  SystemHealth,
  TagItem,
} from "./settingsModel";
import { AiSettingsSection } from "./AiSettingsSection";
import { DataMaintenanceSection } from "./DataMaintenanceSection";
import { MigrationSection } from "./MigrationSection";
import { SystemHealthSection } from "./SystemHealthSection";
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
  originalBackupResult: OriginalBackupResult | null;
  migrationPreviewing: boolean;
  migrationImporting: boolean;
  originalBackuping: boolean;
  tags: TagItem[];
  tagFrom: string;
  tagTo: string;
  tagSaving: boolean;
  systemHealth: SystemHealth | null;
  healthLoading: boolean;
  rebuildingIndex: boolean;
  rebuildingRelations: boolean;
  relationRebuildProgress: RelationRebuildProgress | null;
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
  onRefreshHealth: () => void;
  onRebuildMemoIndex: () => void;
  onRebuildRelations: () => void;
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
  onBackupToOriginal: () => void;
  onRunMutualBackup: () => void;
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
  originalBackupResult,
  migrationPreviewing,
  migrationImporting,
  originalBackuping,
  tags,
  tagFrom,
  tagTo,
  tagSaving,
  systemHealth,
  healthLoading,
  rebuildingIndex,
  rebuildingRelations,
  relationRebuildProgress,
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
  onRefreshHealth,
  onRebuildMemoIndex,
  onRebuildRelations,
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
  onBackupToOriginal,
  onRunMutualBackup,
  onTagFromChange,
  onTagToChange,
  onRenameTag,
}: DataSettingsTabProps) {
  return (
    <>
      <SystemHealthSection
        health={systemHealth}
        healthLoading={healthLoading}
        rebuildingIndex={rebuildingIndex}
        rebuildingRelations={rebuildingRelations}
        relationRebuildProgress={relationRebuildProgress}
        onRefreshHealth={onRefreshHealth}
        onRebuildMemoIndex={onRebuildMemoIndex}
        onRebuildRelations={onRebuildRelations}
      />

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
        originalBackupResult={originalBackupResult}
        migrationPreviewing={migrationPreviewing}
        migrationImporting={migrationImporting}
        originalBackuping={originalBackuping}
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
        onBackupToOriginal={onBackupToOriginal}
        onRunMutualBackup={onRunMutualBackup}
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
