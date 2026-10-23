import { useQueryClient } from '@tanstack/react-query';
import { ChangeEvent, useEffect, useMemo, useRef, useState } from 'react';
import getAdminExtensions, { AdminExtensionList } from '@/api/admin/extensions/getAdminExtensions.ts';
import addExtension from '@/api/admin/extensions/manage/addExtension.ts';
import cancelExtensionRebuild from '@/api/admin/extensions/manage/cancelExtensionRebuild.ts';
import getExtensionStatus, { ExtensionStatus } from '@/api/admin/extensions/manage/getExtensionStatus.ts';
import rebuildExtensions from '@/api/admin/extensions/manage/rebuildExtensions.ts';
import removeExtension from '@/api/admin/extensions/manage/removeExtension.ts';
import restartPanel from '@/api/admin/extensions/manage/restartPanel.ts';
import setExtensionEnabled from '@/api/admin/extensions/setExtensionEnabled.ts';
import { httpErrorToHuman } from '@/api/axios.ts';
import { computePendingRestart, getBuildPhase, removeByPackageName, upsertByPackageName } from '@/lib/extensions.ts';
import { isCancelledError } from '@/lib/files/uploadManager.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { AdminBackendExtension } from '@/lib/schemas/admin/backendExtension.ts';
import { useImportDragAndDrop } from '@/plugins/import/useImportDragAndDrop.ts';
import { usePollingResource } from '@/plugins/resource/usePollingResource.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useMutateWithToast } from '@/plugins/toast/useMutateWithToast.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type PendingLicense = {
  file: File;
  extension: Awaited<ReturnType<typeof addExtension>>['extension'];
};

export type ExtensionUpload = {
  file: File;
  loaded: number;
  controller: AbortController;
};

const PROGRESS_FLUSH_MS = 100;

export function useExtensionManagement() {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const mutate = useMutateWithToast();

  const { data: adminExtensions, refetch: refetchExtensions } = useResource({
    queryKey: queryKeys.admin.extensions.all(),
    queryFn: getAdminExtensions,
  });
  const { data: extensionStatus, refetch: refetchStatus } = usePollingResource({
    queryKey: queryKeys.admin.extensions.status(),
    queryFn: getExtensionStatus,
    interval: 5000,
    pollInBackground: true,
    silent: true,
    retryOnError: 60,
  });

  const [removalExtension, setRemovalExtension] = useState<AdminBackendExtension | null>(null);
  const [pendingLicense, setPendingLicense] = useState<PendingLicense | null>(null);
  const [openModal, setOpenModal] = useState<'logs' | null>(null);
  const [cancellingBuild, setCancellingBuild] = useState<number | null>(null);
  const watchedBuildRef = useRef<number | null>(null);
  const [uploads, setUploads] = useState<Map<number, ExtensionUpload>>(new Map());
  const uploadIdRef = useRef(0);
  const [pendingActions, setPendingActions] = useState<ReadonlySet<string>>(new Set());

  const isPending = (key: string) => pendingActions.has(key);

  const track = <T>(key: string, promise: Promise<T>) => {
    setPendingActions((prev) => new Set(prev).add(key));

    return promise.finally(() =>
      setPendingActions((prev) => {
        const next = new Set(prev);
        next.delete(key);
        return next;
      }),
    );
  };

  const supervisor = extensionStatus?.supervisor ?? null;
  const supervisorState = supervisor?.state ?? null;
  const buildId = supervisor?.buildId ?? null;
  const buildFailed = supervisorState?.type === 'failed';
  const failureReason = supervisor?.failureReason ?? null;
  const failedExtensions = buildFailed ? (supervisor?.failedExtensions ?? []) : [];
  const isBuilding = extensionStatus?.isBuilding ?? false;
  const phase = supervisorState ? getBuildPhase(t, supervisorState) : null;
  const pendingRestart = useMemo(() => computePendingRestart(adminExtensions), [adminExtensions]);

  const setExtensionStatus = (updater: (prev: ExtensionStatus | undefined) => ExtensionStatus | undefined) => {
    queryClient.setQueryData<ExtensionStatus>(queryKeys.admin.extensions.status(), (prev) => updater(prev));
  };

  useEffect(() => {
    if (!supervisorState) return;

    if (supervisorState.type === 'queued' || supervisorState.type === 'building') {
      watchedBuildRef.current = buildId;
      return;
    }

    setCancellingBuild(null);

    if (watchedBuildRef.current === null || watchedBuildRef.current !== buildId) return;
    watchedBuildRef.current = null;

    if (supervisorState.type === 'failed') {
      addToast(
        failureReason
          ? t('pages.admin.extensions.toast.buildFailed', { reason: failureReason })
          : t('pages.admin.extensions.alert.buildFailed.title', {}),
        'error',
      );
      return;
    }

    refetchExtensions();
    addToast(t('pages.admin.extensions.toast.buildCompleted', {}), 'success');
    setOpenModal(null);
  }, [supervisorState?.type, buildId]);

  const handleRebuild = (force: boolean) => {
    mutate(track('rebuild', rebuildExtensions(force)), (rebuild) => {
      watchedBuildRef.current = rebuild.buildId;
      addToast(t('pages.admin.extensions.toast.buildStarted', {}), 'success');
      setExtensionStatus((prev) => prev && { ...prev, isBuilding: true });
      refetchStatus();
      setOpenModal('logs');
    });
  };

  const handleCancelBuild = () => {
    mutate(
      track('cancel', cancelExtensionRebuild(buildId)),
      (cancel) => {
        setCancellingBuild(cancel.buildId);
        addToast(t('pages.admin.extensions.toast.cancelRequested', {}), 'success');
      },
      () => refetchStatus(),
    );
  };

  const handleRemove = (backendExtension: AdminBackendExtension, removeMigrations: boolean) => {
    const packageName = backendExtension.metadataToml.packageName;

    mutate(track(`remove:${packageName}`, removeExtension(packageName, removeMigrations)), () => {
      setExtensionStatus((prev) =>
        prev
          ? {
              ...prev,
              pendingExtensions: removeByPackageName(prev.pendingExtensions, packageName),
              removedExtensions: upsertByPackageName(prev.removedExtensions, backendExtension),
            }
          : prev,
      );
      addToast(t('pages.admin.extensions.toast.removed', { packageName }).md(), 'success');
      setRemovalExtension(null);
    });
  };

  const applyExtension = (extension: PendingLicense['extension']) => {
    setExtensionStatus((prev) => {
      if (!prev) return prev;

      const appliedMatch = adminExtensions?.extensions.find(
        (e) => e.metadataToml.packageName === extension.metadataToml.packageName && e.version === extension.version,
      );

      return {
        ...prev,
        pendingExtensions: appliedMatch
          ? removeByPackageName(prev.pendingExtensions, extension.metadataToml.packageName)
          : upsertByPackageName(prev.pendingExtensions, extension),
        removedExtensions: removeByPackageName(prev.removedExtensions, extension.metadataToml.packageName),
      };
    });
    addToast(
      t('pages.admin.extensions.toast.added', { packageName: extension.metadataToml.packageName }).md(),
      'success',
    );
  };

  const handleToggle = (packageName: string, enabled: boolean) => {
    mutate(track(`toggle:${packageName}`, setExtensionEnabled(packageName, enabled)), () => {
      queryClient.setQueryData<AdminExtensionList>(queryKeys.admin.extensions.all(), (prev) =>
        prev
          ? {
              ...prev,
              pendingDisabled: enabled
                ? prev.pendingDisabled.filter((e) => e !== packageName)
                : [...prev.pendingDisabled, packageName],
            }
          : prev,
      );
      addToast(
        enabled
          ? t('pages.admin.extensions.toast.enabled', { packageName }).md()
          : t('pages.admin.extensions.toast.disabled', { packageName }).md(),
        'success',
      );
    });
  };

  const handleRestart = () => {
    mutate(track('restart', restartPanel()), () => {
      addToast(t('pages.admin.extensions.toast.restarting', {}), 'success');
    });
  };

  const setUploadLoaded = (id: number, loaded: number) => {
    setUploads((prev) => {
      const entry = prev.get(id);
      if (!entry || entry.loaded === loaded) return prev;
      return new Map(prev).set(id, { ...entry, loaded });
    });
  };

  const handleAdd = (file: File, acceptLicense = false) => {
    const id = ++uploadIdRef.current;
    const controller = new AbortController();
    let lastFlush = 0;
    setUploads((prev) => new Map(prev).set(id, { file, loaded: 0, controller }));

    addExtension(file, acceptLicense, controller.signal, (event) => {
      const now = Date.now();
      if (event.loaded < file.size && now - lastFlush < PROGRESS_FLUSH_MS) return;
      lastFlush = now;
      setUploadLoaded(id, Math.min(event.loaded, file.size));
    })
      .then(({ extension, needsLicenseAcceptance }) => {
        if (needsLicenseAcceptance) {
          setPendingLicense({ file, extension });
          return;
        }
        applyExtension(extension);
      })
      .catch((error) => {
        if (!isCancelledError(error)) addToast(httpErrorToHuman(error), 'error');
      })
      .finally(() => {
        setUploads((prev) => {
          const next = new Map(prev);
          next.delete(id);
          return next;
        });
      });
  };

  const handleCancelUpload = (id: number) => {
    uploads.get(id)?.controller.abort();
  };

  const handleLicenseAccept = () => {
    if (!pendingLicense) return;
    setPendingLicense(null);
    handleAdd(pendingLicense.file, true);
  };

  const { isDragging } = useImportDragAndDrop({
    onDrop: (files) => Promise.all(files.map((file) => handleAdd(file))),
    enabled: extensionStatus ? !extensionStatus.isBuilding : false,
    filterFile: (file) => file.name.toLowerCase().endsWith('.zip'),
  });

  const handleFileUpload = (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;

    event.target.value = '';
    handleAdd(file);
  };

  return {
    adminExtensions,
    extensionStatus,
    supervisor,
    buildId,
    buildFailed,
    failureReason,
    failedExtensions,
    isBuilding,
    phase,
    pendingRestart,
    cancellingBuild,
    removalExtension,
    setRemovalExtension,
    pendingLicense,
    setPendingLicense,
    openModal,
    setOpenModal,
    handleRebuild,
    handleCancelBuild,
    handleRemove,
    handleToggle,
    handleRestart,
    isPending,
    handleAdd,
    handleLicenseAccept,
    handleFileUpload,
    uploads,
    handleCancelUpload,
    isDragging,
  };
}
