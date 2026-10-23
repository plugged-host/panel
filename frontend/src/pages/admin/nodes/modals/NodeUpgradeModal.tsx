import { ModalProps } from '@mantine/core';
import { z } from 'zod';
import getNodeSystemOverview from '@/api/admin/nodes/system/getNodeSystemOverview.ts';
import upgradeNode from '@/api/admin/nodes/upgradeNode.ts';
import Button from '@/elements/buttons/Button.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import FormModal from '@/elements/modals/FormModal.tsx';
import { ModalFooter } from '@/elements/modals/Modal.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { AdminNode } from '@/lib/schemas/admin/nodes.ts';
import { adminUpgradeSchema } from '@/lib/schemas/admin/system.ts';
import { useModalForm } from '@/plugins/form/useModalForm.ts';
import { useNodeUpgradeStatus } from '@/plugins/nodes/useNodeVersion.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

export default function NodeUpgradeModal({
  node,
  onScheduled,
  ...props
}: ModalProps & {
  node: AdminNode;
  onScheduled: (previousVersion: string | null) => void;
}) {
  const { t } = useTranslations();
  const { addToast } = useToast();

  const { data: overview } = useResource({
    queryKey: queryKeys.admin.nodes.systemOverview(node.uuid),
    queryFn: () => getNodeSystemOverview(node.uuid),
    enabled: props.opened,
    silent: true,
  });

  const { status } = useNodeUpgradeStatus(node.uuid, { enabled: props.opened });

  const { form, handleClose, handleSubmit, loading, isDirty } = useModalForm<z.infer<typeof adminUpgradeSchema>>({
    initialValues: { version: 'latest' },
    hydrate: () =>
      status?.source === 'daemon' && status.latestVersion ? { version: status.latestVersion } : undefined,
    schema: adminUpgradeSchema,
    opened: props.opened,
    onClose: props.onClose,
    onSubmit: async (values) => {
      const result = await upgradeNode(node.uuid, values);

      if (!result.applied) {
        addToast(t('pages.admin.nodes.modal.upgrade.toast.ignored', { name: node.name }), 'warning');
        return;
      }

      addToast(
        t('pages.admin.nodes.modal.upgrade.toast.scheduled', {
          name: node.name,
          version: result.version ?? values.version,
        }),
        'success',
      );
      onScheduled(overview?.version ?? null);
    },
  });

  return (
    <FormModal
      title={t('pages.admin.nodes.modal.upgrade.title', { name: node.name })}
      isDirty={isDirty}
      loading={loading}
      {...props}
      onClose={handleClose}
      onSubmit={handleSubmit}
    >
      <Stack>
        {overview?.upgrade && (
          <Text size='sm'>
            {t('pages.admin.nodes.modal.upgrade.content', {
              repository: overview.upgrade.repository,
              provider: overview.upgrade.provider,
            }).md()}
          </Text>
        )}

        <TextInput
          withAsterisk
          label={t('pages.admin.nodes.modal.upgrade.form.version', {})}
          description={t('pages.admin.nodes.modal.upgrade.form.versionDescription', {})}
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
