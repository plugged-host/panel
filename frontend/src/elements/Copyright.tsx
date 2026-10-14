import classNames from 'classnames';
import ExtensionSlot from '@/elements/ExtensionSlot.tsx';
import Anchor from '@/elements/typography/Anchor.tsx';
import { useGlobalStore } from '@/stores/global.ts';

export default function Copyright({ className }: { className?: string }) {
  const release = useGlobalStore((state) => state.settings.app?.release);

  return (
    <div className={classNames('flex flex-col text-xs transition-all text-(--mantine-color-dimmed)', className)}>
      <ExtensionSlot
        components={window.extensionContext.extensionRegistry.elements.copyright.prependedComponents}
        name='global-copyright-prepended'
      />

      <span className='flex flex-row flex-wrap gap-x-2'>
        <Anchor size='xs' href='https://plugged.host' target='_blank'>
          plugged.host
        </Anchor>
        {release && <span className='font-mono'>{release}</span>}
        <span>
          &middot; Powered by{' '}
          <Anchor size='xs' href='https://calagopus.com' target='_blank'>
            Calagopus
          </Anchor>
        </span>
      </span>

      <ExtensionSlot
        components={window.extensionContext.extensionRegistry.elements.copyright.appendedComponents}
        name='global-copyright-appended'
      />
    </div>
  );
}
