import { useCallback } from 'react';
import { FileOpenMode } from 'shared/src/registries/pages/server/files/index.ts';
import { fileOpenUrl } from '@/lib/files/files.ts';
import { openUrl } from '@/lib/network/url.ts';
import { FileManagerContextType } from '@/providers/contexts/fileManagerContext.ts';
import { useFileManagerApi } from '@/stores/fileManager.ts';
import { useServerStore } from '@/stores/server.ts';

export default function useOpenFileInNewTab() {
  const server = useServerStore((state) => state.server);
  const store = useFileManagerApi();

  return useCallback(
    (openMode: FileOpenMode, context?: Partial<FileManagerContextType>) => {
      const url = fileOpenUrl(openMode, server, { ...store.getState(), ...context });
      if (url) openUrl(url);
    },
    [server, store],
  );
}
