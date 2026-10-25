import ExtensionSlot from '@/elements/ExtensionSlot.tsx';
// plugged.host: one page per item of the Pelican panel's jump bar; this one is its "Server
// Information". Rename, Debug Information, Reinstall and the behaviour cards moved to the other
// settings pages (docs/features/server-settings.md, docs/upstream.md)
import ServerInformationContainer from './ServerInformationContainer.tsx'; // plugged.host
import ServerSettingsShell from './ServerSettingsShell.tsx'; // plugged.host

export default function ServerSettings() {
  return (
    // plugged.host: the settings frame (title, Delete Server, jump bar) instead of the card grid
    <ServerSettingsShell registry={window.extensionContext.extensionRegistry.pages.server.settings.container}>
      <ExtensionSlot
        components={
          window.extensionContext.extensionRegistry.pages.server.settings.settingContainers.prependedComponents
        }
        name='settings-settingContainer-prepended'
      />

      {/* plugged.host */}
      <ServerInformationContainer />

      <ExtensionSlot
        components={
          window.extensionContext.extensionRegistry.pages.server.settings.settingContainers.appendedComponents
        }
        name='settings-settingContainer-appended'
      />
    </ServerSettingsShell>
  );
}
