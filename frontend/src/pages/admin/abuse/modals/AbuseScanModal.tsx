// plugged.host: Admin › Abuse: the full check of one server now, without suspending.
import { ModalProps } from '@mantine/core';
import { useQueryClient } from '@tanstack/react-query';
import { useEffect, useState } from 'react';
import { z } from 'zod';
import scanServer from '@/api/admin/abuse/scanServer.ts';
import getServers from '@/api/admin/servers/getServers.ts';
import { httpErrorToHuman } from '@/api/axios.ts';
import Button from '@/elements/buttons/Button.tsx';
import Badge from '@/elements/data-display/Badge.tsx';
import Alert from '@/elements/feedback/Alert.tsx';
import ServerSelect from '@/elements/input/ServerSelect.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { Modal, ModalFooter } from '@/elements/modals/Modal.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { AdminAbuseScanResult } from '@/lib/schemas/admin/abuse.ts';
import { adminServerSchema } from '@/lib/schemas/admin/servers.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { severityColorMapping, severityLabelMapping } from '../abuseLabels.ts';

export default function AbuseScanModal(props: ModalProps) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();

  const [serverUuid, setServerUuid] = useState<string | null>(null);
  const [scanning, setScanning] = useState(false);
  const [result, setResult] = useState<AdminAbuseScanResult | null>(null);

  useEffect(() => {
    if (!props.opened) {
      setResult(null);
    }
  }, [props.opened]);

  const doScan = () => {
    if (!serverUuid) {
      return;
    }

    setScanning(true);
    setResult(null);

    scanServer(serverUuid)
      .then((result) => {
        setResult(result);
        queryClient.invalidateQueries({ queryKey: queryKeys.admin.abuse.findings() });
      })
      .catch((msg) => addToast(httpErrorToHuman(msg), 'error'))
      .finally(() => setScanning(false));
  };

  return (
    <Modal title={t('pages.admin.abuse.page.modal.scan.title', {})} {...props}>
      <Stack>
        {t('pages.admin.abuse.page.modal.scan.content', {}).md()}

        <ServerSelect<z.infer<typeof adminServerSchema>>
          withAsterisk
          label={t('common.form.server', {})}
          placeholder={t('common.form.server', {})}
          queryKey={queryKeys.admin.servers.all()}
          fetcher={(search) => getServers(1, search)}
          value={serverUuid}
          onChange={(uuid) => {
            setServerUuid(uuid);
            setResult(null);
          }}
        />

        {result &&
          (!result.finished ? (
            <Alert color='blue'>{t('pages.admin.abuse.page.modal.scan.running', {})}</Alert>
          ) : result.findings.length === 0 ? (
            <Alert color={result.reachable ? 'green' : 'yellow'}>
              {result.reachable
                ? t('pages.admin.abuse.page.modal.scan.clean', {})
                : t('pages.admin.abuse.page.modal.scan.unreachable', {})}
            </Alert>
          ) : (
            <Stack gap='xs'>
              {!result.reachable && (
                <Alert color='yellow'>{t('pages.admin.abuse.page.modal.scan.unreachable', {})}</Alert>
              )}
              {result.findings.map((finding) => (
                <Group key={finding.rule} gap='xs' wrap='nowrap' align='flex-start'>
                  <Badge color={severityColorMapping[finding.severity]} variant='light'>
                    {severityLabelMapping[finding.severity]()}
                  </Badge>
                  <Text size='sm'>{finding.summary}</Text>
                </Group>
              ))}
            </Stack>
          ))}

        <ModalFooter>
          <Button onClick={doScan} loading={scanning} disabled={!serverUuid}>
            {t('pages.admin.abuse.page.modal.scan.run', {})}
          </Button>
          <Button variant='default' onClick={props.onClose}>
            {t('common.button.close', {})}
          </Button>
        </ModalFooter>
      </Stack>
    </Modal>
  );
}
