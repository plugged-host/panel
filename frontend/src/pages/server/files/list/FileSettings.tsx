import { useShallow } from 'zustand/react/shallow';
import Checkbox from '@/elements/input/Checkbox.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import SegmentedControl from '@/elements/layout/SegmentedControl.tsx';
import UserSettingScopeMenu from '@/elements/UserSettingScopeMenu.tsx';
import SettingsPopover from '@/pages/server/files/SettingsPopover.tsx';
import { useFileManager } from '@/providers/FileManagerProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { fileManagerSettingKey } from '@/stores/fileManager.ts';

export default function FileSettings({
  view,
  onViewChange,
}: {
  view: 'list' | 'tree';
  onViewChange: (view: string) => void;
}) {
  const { t } = useTranslations();
  const {
    clickOnce,
    preferPhysicalSize,
    vscodeUriScheme,
    editorPreviewTabs,
    alwaysOverwrite,
    setClickOnce,
    setPreferPhysicalSize,
    setVscodeUriScheme,
    setEditorPreviewTabs,
    setAlwaysOverwrite,
  } = useFileManager(
    useShallow((state) => ({
      clickOnce: state.clickOnce,
      preferPhysicalSize: state.preferPhysicalSize,
      vscodeUriScheme: state.vscodeUriScheme,
      editorPreviewTabs: state.editorPreviewTabs,
      alwaysOverwrite: state.alwaysOverwrite,
      setClickOnce: state.setClickOnce,
      setPreferPhysicalSize: state.setPreferPhysicalSize,
      setVscodeUriScheme: state.setVscodeUriScheme,
      setEditorPreviewTabs: state.setEditorPreviewTabs,
      setAlwaysOverwrite: state.setAlwaysOverwrite,
    })),
  );

  return (
    <SettingsPopover
      tooltip={t('pages.server.files.tooltip.settings', {})}
      registry={window.extensionContext.extensionRegistry.pages.server.files.fileSettings}
      keyPrefix='files-settings'
    >
      <div className='flex flex-col gap-1'>
        <span className='theme-eco-small'>{t('pages.server.files.settings.view', {})}</span>
        <SegmentedControl
          value={view}
          onChange={onViewChange}
          fullWidth
          data={[
            { value: 'list', label: t('pages.server.files.view.list', {}) },
            { value: 'tree', label: t('pages.server.files.view.tree', {}) },
          ]}
        />
      </div>
      <Checkbox
        label={
          <span className='inline-flex items-center gap-1'>
            {t('pages.server.files.settings.clickOnce', {})}
            <UserSettingScopeMenu
              settingKey={fileManagerSettingKey('clickOnce')}
              value={clickOnce}
              withinPortal={false}
            />
          </span>
        }
        checked={clickOnce}
        onChange={(e) => setClickOnce(e.target.checked)}
      />
      <Checkbox
        label={
          <span className='inline-flex items-center gap-1'>
            {t('pages.server.files.settings.preferPhysicalSize', {})}
            <UserSettingScopeMenu
              settingKey={fileManagerSettingKey('preferPhysicalSize')}
              value={preferPhysicalSize}
              withinPortal={false}
            />
          </span>
        }
        checked={preferPhysicalSize}
        onChange={(e) => setPreferPhysicalSize(e.target.checked)}
      />
      <Checkbox
        label={
          <span className='inline-flex items-center gap-1'>
            {t('pages.server.files.settings.editorPreviewTabs', {})}
            <UserSettingScopeMenu
              settingKey={fileManagerSettingKey('editorPreviewTabs')}
              value={editorPreviewTabs}
              withinPortal={false}
            />
          </span>
        }
        description={t('pages.server.files.settings.editorPreviewTabsDescription', {})}
        checked={editorPreviewTabs}
        onChange={(e) => setEditorPreviewTabs(e.target.checked)}
      />
      <Checkbox
        label={
          <span className='inline-flex items-center gap-1'>
            {t('pages.server.files.settings.alwaysOverwrite', {})}
            <UserSettingScopeMenu
              settingKey={fileManagerSettingKey('alwaysOverwrite')}
              value={alwaysOverwrite}
              withinPortal={false}
            />
          </span>
        }
        description={t('pages.server.files.settings.alwaysOverwriteDescription', {})}
        checked={alwaysOverwrite}
        onChange={(e) => setAlwaysOverwrite(e.target.checked)}
      />
      <TextInput
        label={t('pages.server.files.settings.vscodeUriScheme', {})}
        value={vscodeUriScheme}
        onChange={(e) => setVscodeUriScheme(e.target.value)}
      />
    </SettingsPopover>
  );
}
