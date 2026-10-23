// plugged.host: Admin › Abuse: a finding with its evidence.
import { ModalProps } from '@mantine/core';
import Button from '@/elements/buttons/Button.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { Modal, ModalFooter } from '@/elements/modals/Modal.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import Code from '@/elements/typography/Code.tsx';
import Text from '@/elements/typography/Text.tsx';
import { AdminAbuseFinding } from '@/lib/schemas/admin/abuse.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

function Detail({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <Group gap='xs' wrap='nowrap' align='flex-start'>
      <Text size='sm' c='dimmed' w={140} className='shrink-0'>
        {label}
      </Text>
      <Text size='sm' component='div'>
        {children}
      </Text>
    </Group>
  );
}

export default function AbuseFindingModal({ finding, ...props }: ModalProps & { finding: AdminAbuseFinding }) {
  const { t } = useTranslations();

  return (
    <Modal title={finding.categoryLabel} size='lg' {...props}>
      <Stack>
        <Text size='sm'>{finding.summary}</Text>

        <Stack gap={4}>
          {finding.server && (
            <>
              <Detail label={t('common.table.columns.server', {})}>{finding.server.name}</Detail>
              {finding.server.ownerUsername && (
                <Detail label={t('common.table.columns.owner', {})}>{finding.server.ownerUsername}</Detail>
              )}
              {finding.server.eggName && (
                <Detail label={t('pages.admin.abuse.page.detail.egg', {})}>{finding.server.eggName}</Detail>
              )}
            </>
          )}
          {finding.node && <Detail label={t('common.table.columns.node', {})}>{finding.node.name}</Detail>}
          <Detail label={t('pages.admin.abuse.page.detail.rule', {})}>
            <Code>{finding.rule}</Code>
          </Detail>
          <Detail label={t('pages.admin.abuse.page.detail.hits', {})}>{finding.hits}</Detail>
          <Detail label={t('pages.admin.abuse.page.detail.firstSeen', {})}>
            <FormattedTimestamp timestamp={finding.firstSeen} />
          </Detail>
          <Detail label={t('pages.admin.abuse.page.column.lastSeen', {})}>
            <FormattedTimestamp timestamp={finding.lastSeen} />
          </Detail>
          {finding.action && finding.actioned && (
            <Detail label={t('pages.admin.abuse.page.detail.action', {})}>
              {finding.action} (<FormattedTimestamp timestamp={finding.actioned} />)
            </Detail>
          )}
        </Stack>

        <Stack gap={4}>
          <Text size='sm' c='dimmed'>
            {t('pages.admin.abuse.page.detail.evidence', {})}
          </Text>
          <Code block className='whitespace-pre-wrap break-all'>
            {JSON.stringify(finding.evidence, null, 2)}
          </Code>
        </Stack>

        <ModalFooter>
          <Button variant='default' onClick={props.onClose}>
            {t('common.button.close', {})}
          </Button>
        </ModalFooter>
      </Stack>
    </Modal>
  );
}
