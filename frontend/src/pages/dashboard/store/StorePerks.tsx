// plugged.host: the bar over the Resource Store with the upgrades the account has not claimed yet,
// the Pelican panel's perk discovery: one closed bar with the count in its heading, opened rows
// with a gray button each, remembered open or closed, and gone once nothing is left
// (docs/features/economy.md#the-resource-store).
import { faChevronDown } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { Button as MantineButton, UnstyledButton } from '@mantine/core';
import { useId, useState } from 'react';
import { z } from 'zod';
import Button from '@/elements/buttons/Button.tsx';
import Card from '@/elements/data-display/Card.tsx';
import Collapse from '@/elements/layout/Collapse.tsx';
import { bytesToString, mbToBytes } from '@/lib/format/size.ts';
import { storePerkSchema } from '@/lib/schemas/user/economy.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type Perk = z.infer<typeof storePerkSchema>;

const page = 'pages.account.store.perks';
const storageKey = 'store-perks:open';

function storedOpen(): boolean {
  try {
    return window.localStorage.getItem(storageKey) === '1';
  } catch {
    return false;
  }
}

export default function StorePerks({
  perks,
  credits,
  onOpenStore,
}: {
  perks: Perk[];
  credits: number;
  /** The store perk's button: the page is the store, so it goes to the resources. */
  onOpenStore: () => void;
}) {
  const { t, language } = useTranslations();
  const contentId = useId();
  const [open, setOpen] = useState(storedOpen);

  if (perks.length === 0) {
    return null;
  }

  const number = new Intl.NumberFormat(language);
  const size = (mb: number) => bytesToString(mbToBytes(mb), 1);

  // "+3 GiB RAM", "+50% CPU and +3 GiB RAM", "+50% CPU, +3 GiB RAM and +10 GiB disk space"
  const gain = ({ cpu, memory, disk }: { cpu: number; memory: number; disk: number }) => {
    const parts = [
      memory > 0 ? t(`${page}.gain.memory`, { amount: size(memory) }) : null,
      cpu > 0 ? t(`${page}.gain.cpu`, { amount: `${cpu}%` }) : null,
      disk > 0 ? t(`${page}.gain.disk`, { amount: size(disk) }) : null,
    ].filter((part) => part !== null);
    const last = parts.pop() ?? '';

    return parts.length > 0 ? `${parts.join(', ')} ${t(`${page}.gain.and`, {})} ${last}` : last;
  };

  const rows = perks.map((perk) => {
    switch (perk.id) {
      case 'discord_link':
        return {
          id: perk.id,
          title: t(`${page}.discordLink.title`, { gain: gain(perk.gain) }),
          body: t(`${page}.discordLink.body`, {}),
          cta: t(`${page}.discordLink.cta`, {}),
          url: perk.url,
        };
      case 'discord_join':
        return {
          id: perk.id,
          title: t(`${page}.discordJoin.title`, { gain: gain(perk.gain) }),
          body: t(`${page}.discordJoin.body`, {}),
          cta: t(`${page}.discordJoin.cta`, {}),
          url: perk.url,
        };
      case 'discord_earn':
        return {
          id: perk.id,
          title: t(`${page}.discordEarn.title`, { credits: number.format(perk.perInvite) }),
          body:
            perk.perBoost > 0
              ? t(`${page}.discordEarn.bodyBoost`, { credits: number.format(perk.perBoost) })
              : t(`${page}.discordEarn.body`, {}),
          cta: t(`${page}.discordEarn.cta`, {}),
          url: perk.url,
        };
      case 'store':
        return {
          id: perk.id,
          title: t(`${page}.store.title`, { price: number.format(perk.price), amount: size(perk.amount) }),
          body: t(`${page}.store.body`, { balance: number.format(credits) }),
          cta: t(`${page}.store.cta`, {}),
          url: null,
        };
    }
  });

  const toggle = () => {
    const next = !open;
    setOpen(next);
    try {
      window.localStorage.setItem(storageKey, next ? '1' : '0');
    } catch {
      // nothing is kept; the bar still opens and closes
    }
  };

  return (
    <Card p={0}>
      <UnstyledButton className='theme-collapse-toggle' aria-expanded={open} aria-controls={contentId} onClick={toggle}>
        <span className='theme-eco-heading'>{t(`${page}.heading`, { count: perks.length })}</span>
        <FontAwesomeIcon icon={faChevronDown} className='theme-collapse-arrow' />
      </UnstyledButton>

      <Collapse expanded={open} id={contentId}>
        <div className='theme-collapse-body'>
          <div className='theme-rows'>
            {rows.map((row) => (
              <div key={row.id} className='theme-row'>
                <div className='theme-row-text'>
                  <h4 className='theme-row-title'>{row.title}</h4>
                  <p className='theme-row-body'>{row.body}</p>
                </div>

                <div className='theme-row-actions'>
                  {row.url ? (
                    <MantineButton
                      component='a'
                      href={row.url}
                      target='_blank'
                      rel='noopener noreferrer'
                      variant='default'
                      size='sm'
                      fw={600}
                    >
                      {row.cta}
                    </MantineButton>
                  ) : (
                    <Button variant='default' size='sm' onClick={onOpenStore}>
                      {row.cta}
                    </Button>
                  )}
                </div>
              </div>
            ))}
          </div>
        </div>
      </Collapse>
    </Card>
  );
}
