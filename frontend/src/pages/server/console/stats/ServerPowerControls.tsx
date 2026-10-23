import { faPlay, faRotateRight, faSkull, faStop } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useEffect, useState } from 'react';
import { z } from 'zod';
import { useShallow } from 'zustand/react/shallow';
import Button from '@/elements/buttons/Button.tsx';
import { ServerCan } from '@/elements/Can.tsx';
import ExtensionSlot from '@/elements/ExtensionSlot.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import { serverPowerAction } from '@/lib/schemas/server/server.ts';
import { usePowerAction } from '@/plugins/server/usePowerAction.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';

export default function ServerPowerControls() {
  const { t } = useTranslations();
  const [open, setOpen] = useState(false);
  const { server, state, socketConnected } = useServerStore(
    useShallow((s) => ({
      server: s.server,
      state: s.state,
      socketConnected: s.socketConnected,
    })),
  );
  const powerAction = usePowerAction(); // plugged.host: over the API, through the start gates

  const killable = state === 'stopping';

  const onButtonClick = (action: z.infer<typeof serverPowerAction> | 'kill-confirmed') => {
    if (action === 'kill') {
      return setOpen(true);
    }

    setOpen(false);
    powerAction(action === 'kill-confirmed' ? 'kill' : action);
  };

  useEffect(() => {
    if (state === 'offline') {
      setOpen(false);
    }
  }, [state]);

  return (
    // plugged.host: the Pelican console's power actions, separate pills at the end of the title row
    // (Start in the accent, Restart gray, Stop and Kill red, each with its Tabler icon); on phones
    // only the icons, as Theme v2 has them.
    <div className='theme-power flex w-full md:w-fit gap-2'>
      <ConfirmationModal
        opened={open}
        onClose={() => setOpen(false)}
        title={t('pages.server.console.power.modal.forceStop.title', {})}
        confirm={t('common.button.continue', {})}
        onConfirmed={() => onButtonClick('kill-confirmed')}
      >
        {t('pages.server.console.power.modal.forceStop.content', {}).md()}
      </ConfirmationModal>

      <ExtensionSlot
        components={
          window.extensionContext.extensionRegistry.pages.server.console.powerButtonComponents.prependedComponents
        }
        name='console-powerbutton-prepended'
      />

      <ServerCan action='control.start'>
        <Button
          color='blue'
          disabled={
            !socketConnected || state !== 'offline' || !!server.status || server.isSuspended || server.isTransferring
          }
          loading={state === 'starting'}
          onClick={() => onButtonClick('start')}
          className='flex-1 min-w-fit'
          leftSection={<FontAwesomeIcon icon={faPlay} />}
          aria-label={t('common.enum.serverPowerAction.start', {})}
        >
          <span className='theme-power-label'>{t('common.enum.serverPowerAction.start', {})}</span>
        </Button>
      </ServerCan>
      <ServerCan action='control.restart'>
        <Button
          variant='default'
          disabled={!socketConnected || !state || !!server.status || server.isSuspended || server.isTransferring}
          onClick={() => onButtonClick('restart')}
          className='flex-1 min-w-fit'
          leftSection={<FontAwesomeIcon icon={faRotateRight} />}
          aria-label={t('common.enum.serverPowerAction.restart', {})}
        >
          <span className='theme-power-label'>{t('common.enum.serverPowerAction.restart', {})}</span>
        </Button>
      </ServerCan>
      <ServerCan action='control.stop'>
        <Button
          color='red'
          disabled={
            !socketConnected || state === 'offline' || !!server.status || server.isSuspended || server.isTransferring
          }
          onClick={() => onButtonClick(killable ? 'kill' : 'stop')}
          className='flex-1 min-w-fit'
          leftSection={<FontAwesomeIcon icon={killable ? faSkull : faStop} />}
          aria-label={
            killable ? t('common.enum.serverPowerAction.kill', {}) : t('common.enum.serverPowerAction.stop', {})
          }
        >
          <span className='theme-power-label'>
            {killable ? t('common.enum.serverPowerAction.kill', {}) : t('common.enum.serverPowerAction.stop', {})}
          </span>
        </Button>
      </ServerCan>

      <ExtensionSlot
        components={
          window.extensionContext.extensionRegistry.pages.server.console.powerButtonComponents.appendedComponents
        }
        name='console-powerButton-appended'
      />
    </div>
  );
}
