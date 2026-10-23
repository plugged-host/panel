import { faDownload } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { DrawerProps } from '@mantine/core';
import { lazy, useEffect, useState } from 'react';
import { z } from 'zod';
import Button from '@/elements/buttons/Button.tsx';
import { AdminCan } from '@/elements/Can.tsx';
import Spinner from '@/elements/feedback/Spinner.tsx';
import ScrollArea from '@/elements/layout/ScrollArea.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import Drawer from '@/elements/overlays/Drawer.tsx';
import { adminEggRepositoryEggSchema, adminEggRepositorySchema } from '@/lib/schemas/admin/eggRepositories.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import EggRepositoryEggInstallModal from '../modals/EggRepositoryEggInstallModal.tsx';

const EggRepositoryEggReadme = lazy(() => import('./EggRepositoryEggReadme.tsx'));

export default function EggRepositoryEggDrawer({
  eggRepository,
  egg,
  opened,
  onClose,
  ...props
}: DrawerProps & {
  eggRepository: z.infer<typeof adminEggRepositorySchema> | null;
  egg: z.infer<typeof adminEggRepositoryEggSchema> | null;
}) {
  const { t } = useTranslations();
  const [installOpen, setInstallOpen] = useState(false);
  const [displayEgg, setDisplayEgg] = useState(egg);

  useEffect(() => {
    if (egg) {
      setDisplayEgg(egg);
    }
  }, [egg]);

  return (
    <Drawer
      position='right'
      offset={8}
      radius='md'
      opened={opened}
      onClose={onClose}
      title={displayEgg?.exportedEgg.name}
      size='lg'
      {...props}
    >
      {displayEgg && (
        <>
          {eggRepository && (
            <AdminCan action={['eggs.create', 'nests.read']}>
              <EggRepositoryEggInstallModal
                eggRepository={eggRepository}
                eggs={[displayEgg]}
                opened={installOpen}
                onClose={() => setInstallOpen(false)}
              />
            </AdminCan>
          )}

          <Stack gap='md' className='h-full'>
            {displayEgg.banner && (
              <img
                src={displayEgg.banner}
                alt={displayEgg.exportedEgg.name}
                className='mx-auto max-h-60 max-w-full rounded-md'
              />
            )}

            {eggRepository && (
              <AdminCan action={['eggs.create', 'nests.read']}>
                <Button leftSection={<FontAwesomeIcon icon={faDownload} />} onClick={() => setInstallOpen(true)}>
                  {t('common.button.install', {})}
                </Button>
              </AdminCan>
            )}

            <ScrollArea className='flex-1' offsetScrollbars>
              {displayEgg.readme ? (
                <Spinner.Suspense className='py-12'>
                  <EggRepositoryEggReadme readme={displayEgg.readme} />
                </Spinner.Suspense>
              ) : (
                <div className='flex items-center justify-center py-12 text-(--mantine-color-dimmed)'>
                  {t('pages.admin.eggRepositories.tabs.eggs.page.drawer.noReadme', {})}
                </div>
              )}
            </ScrollArea>
          </Stack>
        </>
      )}
    </Drawer>
  );
}
