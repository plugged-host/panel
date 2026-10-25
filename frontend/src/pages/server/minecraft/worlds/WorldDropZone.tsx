// plugged.host: the loaded world's upload drop zone, the Pelican panel's: drop or choose an archive,
// then "Upload and replace" (docs/features/minecraft.md)
import { useQueryClient } from '@tanstack/react-query';
import { DragEvent, useEffect, useRef, useState } from 'react';
import { httpErrorToHuman } from '@/api/axios.ts';
import Button from '@/elements/buttons/Button.tsx';
import Checkbox from '@/elements/input/Checkbox.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import { useUploadsStore } from '@/stores/uploads.ts';
import { ARCHIVES, isWorldArchive, isWorldUpload, uploadWorld } from './worldUpload.ts';

interface Props {
  /** Nothing can be uploaded now (the server runs, or a world is being imported), and why. */
  locked: string | null;
}

export default function WorldDropZone({ locked }: Props) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const server = useServerStore((state) => state.server);
  const base = 'pages.server.minecraft.worlds';

  const inputRef = useRef<HTMLInputElement>(null);
  const [file, setFile] = useState<File | null>(null);
  const [keep, setKeep] = useState(true);
  const [over, setOver] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [uploading, setUploading] = useState(false);
  // The folder the archive goes into, which finds its entry in the uploads store.
  const [directory, setDirectory] = useState<string | null>(null);
  const [lastPercent, setLastPercent] = useState(0);

  const progress = useUploadsStore((state) => {
    if (!directory) return null;

    for (const item of state.uploads.values()) {
      if (isWorldUpload(item, server.uuid, directory)) return item.progress;
    }
    return null;
  });
  // A finished upload leaves the store while the import is asked for: keep showing where it got.
  useEffect(() => {
    if (progress !== null) setLastPercent(progress);
  }, [progress]);
  const percent = Math.round(progress ?? lastPercent);

  const blocked = !!locked || uploading;

  const choose = (chosen: File | undefined) => {
    setError(null);
    if (!chosen || blocked) return;

    if (isWorldArchive(chosen.name)) {
      setFile(chosen);
    } else {
      setFile(null);
      setError(t(`${base}.form.archiveInvalid`, {}));
    }
  };

  const submit = () => {
    if (!file || blocked) return;

    let done = false;
    setLastPercent(0);
    setError(null);
    setUploading(true);

    uploadWorld(server, file, keep, (outcome) => {
      done = true;
      setUploading(false);
      setDirectory(null);

      if (outcome.kind === 'importing') {
        setFile(null);
        addToast(t(`${base}.toast.importStarted`, {}), 'success');
      } else if (outcome.kind === 'uploadFailed') {
        setError(t(`${base}.upload.failed`, {}));
      } else {
        setError(httpErrorToHuman(outcome.error));
      }

      queryClient.invalidateQueries({ queryKey: queryKeys.server(server.uuid).minecraft.worlds() });
    })
      .then((target) => {
        if (!done) setDirectory(target);
      })
      .catch((err) => {
        setUploading(false);
        setError(httpErrorToHuman(err));
      });
  };

  const onDragOver = (event: DragEvent) => {
    event.preventDefault();
    setOver(!blocked);
  };

  const onDragLeave = (event: DragEvent) => {
    event.preventDefault();
    setOver(false);
  };

  const onDrop = (event: DragEvent) => {
    event.preventDefault();
    setOver(false);
    choose(event.dataTransfer.files[0]);
  };

  return (
    <div className='theme-world-upload'>
      <button
        type='button'
        className='theme-world-drop'
        data-over={over || undefined}
        data-locked={blocked || undefined}
        aria-disabled={blocked || undefined}
        onClick={() => !blocked && inputRef.current?.click()}
        onDragOver={onDragOver}
        onDragLeave={onDragLeave}
        onDrop={onDrop}
      >
        <span className='theme-world-drop-text'>
          <strong>{file?.name ?? t(`${base}.upload.title`, {})}</strong>
          <span>{locked ?? t(`${base}.upload.hint`, {})}</span>
        </span>
      </button>
      <input
        ref={inputRef}
        type='file'
        accept={ARCHIVES.join(',')}
        hidden
        onChange={(event) => {
          choose(event.currentTarget.files?.[0]);
          event.currentTarget.value = '';
        }}
      />

      {uploading && (
        <div
          className='theme-world-bar'
          role='progressbar'
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={percent}
        >
          <span style={{ inlineSize: `${percent}%` }} />
        </div>
      )}
      {error && <p className='theme-world-error'>{error}</p>}

      {file && (
        <div className='theme-world-upload-foot'>
          <Checkbox
            size='xs'
            label={t(`${base}.form.keep`, {})}
            checked={keep}
            disabled={uploading}
            onChange={(event) => setKeep(event.currentTarget.checked)}
          />
          <div className='flex flex-wrap items-center gap-2'>
            <span className='theme-eco-muted'>{t(`${base}.deletesCurrent`, {})}</span>
            <Button size='sm' disabled={blocked} onClick={submit}>
              {uploading ? `${percent}%` : t(`${base}.button.uploadSubmit`, {})}
            </Button>
          </div>
        </div>
      )}
    </div>
  );
}
