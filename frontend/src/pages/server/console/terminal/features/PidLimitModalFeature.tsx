// plugged.host: the Pelican panel's `pid_limit` egg feature: a notice when the server cannot start more threads (docs/features/minecraft.md#thread-limit-notice)
import { useEffect, useRef, useState } from 'react';
import Button from '@/elements/buttons/Button.tsx';
import { Modal, ModalFooter } from '@/elements/modals/Modal.tsx';
import Code from '@/elements/typography/Code.tsx';
import Text from '@/elements/typography/Text.tsx';
import { isAdmin } from '@/lib/auth/permissions.ts';
import useWebsocketEvent, { SocketEvent } from '@/plugins/websocket/useWebsocketEvent.ts';
import { useAuth } from '@/providers/AuthProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';

/** The console lines the Pelican panel's PIDLimitSchema listens for, lower case. */
const MATCHERS = [
  'pthread_create failed',
  'failed to create thread',
  'unable to create thread',
  'unable to create native thread',
  'unable to create new native thread',
  'exception in thread "craft async scheduler management thread"',
];

/** What a player passes on to the administrator, as the Pelican panel words it. */
const ERROR_CODE = 'pthread_create failed, Possibly out of memory or process/resource limits reached';

export default function PidLimitModal() {
  const { t } = useTranslations();
  const { user } = useAuth();
  const state = useServerStore((s) => s.state);

  const [opened, setOpened] = useState(false);
  // once per run of the server: a thread error usually floods the console
  const shown = useRef(false);

  useEffect(() => {
    if (state === 'starting') {
      shown.current = false;
    }
  }, [state]);

  useWebsocketEvent(SocketEvent.CONSOLE_OUTPUT, (line) => {
    if (shown.current || (state !== 'starting' && state !== 'running')) return;

    const normalized = line.toLowerCase();
    if (MATCHERS.some((matcher) => normalized.includes(matcher))) {
      shown.current = true;
      setOpened(true);
    }
  });

  const admin = isAdmin(user);

  return (
    <Modal
      title={t(
        admin ? 'pages.server.console.feature.pidLimit.titleAdmin' : 'pages.server.console.feature.pidLimit.titleUser',
        {},
      )}
      opened={opened}
      onClose={() => setOpened(false)}
    >
      {admin ? (
        <>
          <Text size='sm' mb='md'>
            {t('pages.server.console.feature.pidLimit.contentAdmin', {}).md()}
          </Text>
          <Text size='sm' mb='md'>
            {t('pages.server.console.feature.pidLimit.contentAdminRestart', {}).md()}
          </Text>
        </>
      ) : (
        <>
          <Text size='sm' mb='md'>
            {t('pages.server.console.feature.pidLimit.contentUser', {})}
          </Text>
          <Code block dir='ltr'>
            {ERROR_CODE}
          </Code>
        </>
      )}

      <ModalFooter>
        <Button variant='default' onClick={() => setOpened(false)}>
          {t('common.button.close', {})}
        </Button>
      </ModalFooter>
    </Modal>
  );
}
