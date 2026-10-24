// plugged.host: a world archive goes up through the file manager's uploads, then is imported
// (docs/features/minecraft.md)
import importWorld from '@/api/server/minecraft/importWorld.ts';
import prepareWorldUpload from '@/api/server/minecraft/prepareWorldUpload.ts';
import { registerUploadRefresh, uploadFiles } from '@/lib/files/uploadManager.ts';
import { UploadDestination, uploadScopeKey } from '@/stores/uploads.ts';

/**
 * Starts uploading the archive with the file manager's uploader (in parts, resumable, shown in the
 * uploads card) and asks the panel to import it once the upload is done. The import is started
 * from here, so the tab has to stay open until the upload has finished.
 */
export async function uploadWorld(
  server: { uuid: string; uuidShort: string; name: string },
  file: File,
  keep: boolean,
  onImport: (error: unknown) => void,
): Promise<void> {
  const directory = await prepareWorldUpload(server.uuid);
  const destination: UploadDestination = {
    type: 'server',
    serverUuid: server.uuid,
    serverName: server.name,
    routeId: server.uuidShort,
    directory,
  };

  const unregister = registerUploadRefresh(uploadScopeKey(destination), () => {
    unregister();
    importWorld(server.uuid, file.name, keep)
      .then(() => onImport(null))
      .catch(onImport);
  });

  // Not awaited: the upload runs on in the uploads card while the page moves on.
  uploadFiles(destination, [file]).catch((error) => console.error(error));
}
