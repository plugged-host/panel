// plugged.host: a world archive goes up through the file manager's uploads, then is imported
// (docs/features/minecraft.md)
import importWorld from '@/api/server/minecraft/importWorld.ts';
import prepareWorldUpload from '@/api/server/minecraft/prepareWorldUpload.ts';
import { registerUploadRefresh, uploadFiles } from '@/lib/files/uploadManager.ts';
import { UploadDestination, UploadItem, uploadScopeKey, useUploadsStore } from '@/stores/uploads.ts';

/** The archives Wings unpacks that a world may come in. */
export const ARCHIVES = ['.zip', '.tar', '.tar.gz', '.tgz', '.tar.xz', '.tar.zst'];

/** How an upload ended: the import started, the upload failed, or the panel refused the import. */
export type WorldUploadOutcome =
  | { kind: 'importing' }
  | { kind: 'uploadFailed' }
  | { kind: 'importFailed'; error: unknown };

export function isWorldArchive(name: string): boolean {
  const lower = name.toLowerCase();
  return !name.startsWith('.') && ARCHIVES.some((extension) => lower.endsWith(extension));
}

/** Whether an entry of the uploads store goes into this server's world upload folder. */
export function isWorldUpload(item: UploadItem, serverUuid: string, directory: string): boolean {
  return (
    item.destination.type === 'server' &&
    item.destination.serverUuid === serverUuid &&
    item.destination.directory === directory
  );
}

/**
 * Starts uploading the archive with the file manager's uploader (in parts, resumable, shown in the
 * uploads card) and asks the panel to import it once the upload is done. Returns the folder it
 * goes into, which tells its entry in the uploads store apart. The import is started from here,
 * so the tab has to stay open until the upload has finished.
 */
export async function uploadWorld(
  server: { uuid: string; uuidShort: string; name: string },
  file: File,
  keep: boolean,
  onDone: (outcome: WorldUploadOutcome) => void,
): Promise<string> {
  const directory = await prepareWorldUpload(server.uuid);
  const destination: UploadDestination = {
    type: 'server',
    serverUuid: server.uuid,
    serverName: server.name,
    routeId: server.uuidShort,
    directory,
  };

  const unregister = registerUploadRefresh(uploadScopeKey(destination), () => {
    // Called whenever an upload to this server settles, which may be another one first: a
    // finished upload has left the store by now, a failed one stays in it.
    const ours = [...useUploadsStore.getState().uploads.values()].filter((item) =>
      isWorldUpload(item, server.uuid, directory),
    );
    if (ours.some((item) => item.status !== 'error')) return;

    unregister();
    if (ours.length > 0) {
      onDone({ kind: 'uploadFailed' });
      return;
    }

    importWorld(server.uuid, file.name, keep)
      .then(() => onDone({ kind: 'importing' }))
      .catch((error) => onDone({ kind: 'importFailed', error }));
  });

  // Not awaited: the upload runs on in the uploads card while the page moves on.
  uploadFiles(destination, [file]).catch((error) => console.error(error));

  return directory;
}
