import { ModalProps } from '@mantine/core';
import { z } from 'zod';
import getDatabaseAgentHostSystemOverview from '@/api/admin/database-agent-hosts/getDatabaseAgentHostSystemOverview.ts';
import upgradeDatabaseAgentHost from '@/api/admin/database-agent-hosts/upgradeDatabaseAgentHost.ts';
import Button from '@/elements/buttons/Button.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import FormModal from '@/elements/modals/FormModal.tsx';
import { ModalFooter } from '@/elements/modals/Modal.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { adminDatabaseAgentHostSchema } from '@/lib/schemas/admin/databaseAgentHosts.ts';
import { adminUpgradeSchema } from '@/lib/schemas/admin/system.ts';
import { useDatabaseAgentHostUpgradeStatus } from '@/plugins/database-agent-hosts/useDatabaseAgentHostVersion.ts';
import { useModalForm } from '@/plugins/form/useModalForm.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

export default function DatabaseAgentHostUpgradeModal({
  databaseAgentHost,
  onScheduled,
  ...props
}: ModalProps & {
  databaseAgentHost: z.infer<typeof adminDatabaseAgentHostSchema>;
  onScheduled: (previousVersion: string | null) => void;
}) {
  const { t } = useTranslations();
  const { addToast } = useToast();

  const { data: overview } = useResource({
    queryKey: queryKeys.admin.databaseAgentHosts.systemOverview(databaseAgentHost.uuid),
    queryFn: () => getDatabaseAgentHostSystemOverview(databaseAgentHost.uuid),
    enabled: props.opened,
    silent: true,
  });

  const { status } = useDatabaseAgentHostUpgradeStatus(databaseAgentHost.uuid, { enabled: props.opened });

  const { form, handleClose, handleSubmit, loading, isDirty } = useModalForm<z.infer<typeof adminUpgradeSchema>>({
    initialValues: { version: 'latest' },
    hydrate: () =>
      status?.source === 'daemon' && status.latestVersion ? { version: status.latestVersion } : undefined,
    schema: adminUpgradeSchema,
    opened: props.opened,
    onClose: props.onClose,
    onSubmit: async (values) => {
      const result = await upgradeDatabaseAgentHost(databaseAgentHost.uuid, values);

      if (!result.applied) {
        addToast(
          t('pages.admin.databaseAgentHosts.modal.upgrade.toast.ignored', { name: databaseAgentHost.name }),
          'warning',
        );
        return;
      }

      addToast(
        t('pages.admin.databaseAgentHosts.modal.upgrade.toast.scheduled', {
          name: databaseAgentHost.name,
          version: result.version ?? values.version,
        }),
        'success',
      );
      onScheduled(overview?.version ?? null);
    },
  });

  return (
    <FormModal
      title={t('pages.admin.databaseAgentHosts.modal.upgrade.title', { name: databaseAgentHost.name })}
      isDirty={isDirty}
      loading={loading}
      {...props}
      onClose={handleClose}
      onSubmit={handleSubmit}
    >
      <Stack>
        {overview?.upgrade && (
          <Text size='sm'>
            {t('pages.admin.databaseAgentHosts.modal.upgrade.content', {
              repository: overview.upgrade.repository,
              provider: overview.upgrade.provider,
            }).md()}
          </Text>
        )}

        <TextInput
          withAsterisk
          label={t('pages.admin.databaseAgentHosts.modal.upgrade.form.version', {})}
          description={t('pages.admin.databaseAgentHosts.modal.upgrade.form.versionDescription', {})}
          placeholder='latest'
          key={form.key('version')}
          {...form.getInputProps('version')}
        />

        <ModalFooter>
          <Button type='submit' loading={loading} disabled={!form.isValid()}>
            {t('common.button.upgrade', {})}
          </Button>
          <Button variant='default' onClick={handleClose}>
            {t('common.button.close', {})}
          </Button>
        </ModalFooter>
      </Stack>
    </FormModal>
  );
}
