// plugged.host: Settings' "Upgrades": what the user buys with tokens for this server, one full-width row per
// purchase (what it does, until when it runs, one gray button to buy or add time) and per offer, as the Danger
// Zone's rows (docs/features/server-upgrades.md)
import { useState } from 'react';
import Button from '@/elements/buttons/Button.tsx';
import { formatDateTime } from '@/lib/format/time.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import CardColorModal, { colorName } from '../upgrades/CardColorModal.tsx';
import UpgradeConfirmModal from '../upgrades/UpgradeConfirmModal.tsx';
import {
  KIND_KEYS,
  type UpgradeKind,
  type UpgradeOffer,
  type UpgradeRequest,
  type Upgrades,
  upgradesBase,
  useServerUpgrades,
  useUpgradeDescription,
  useUpgradeDuration,
} from '../upgrades/useServerUpgrades.ts';
import SettingsSection from './SettingsSection.tsx';

// the card colour comes last, on a row of its own
const ORDER = ['keep_awake', 'boost', 'extra_allocation', 'extra_database', 'extra_backup'] as const;

const UNAVAILABLE = {
  max: 'max',
  node_busy: 'nodeBusy',
  node_unknown: 'nodeUnknown',
  unlimited: 'unlimited',
  parked: 'parked',
} as const;

interface Row {
  key: string;
  title: string;
  body: string;
  note?: string;
  action?: { label: string; disabled?: boolean; onClick: () => void };
  color?: string | null;
}

function useRows(upgrades: Upgrades, buy: (request: UpgradeRequest) => void, pickColor: () => void): Row[] {
  const { t, tItem } = useTranslations();
  const duration = useUpgradeDuration();
  const describe = useUpgradeDescription();

  const offerOf = (kind: UpgradeKind): UpgradeOffer | undefined =>
    upgrades.offers.find((candidate) => candidate.kind === kind);
  const add = (offer: UpgradeOffer) =>
    t(`${upgradesBase}.add`, { duration: duration(offer.hours), price: tItem('token', offer.tokens) });
  const runsUntil = (until: Date | null) =>
    until ? t(`${upgradesBase}.runsUntil`, { time: formatDateTime(until) }) : '';
  // a pass or a boost that runs gets longer; an extra slot is renewed on its own
  const single = (kind: UpgradeKind) => kind === 'keep_awake' || kind === 'boost';

  const rows: Row[] = [];
  for (const kind of ORDER) {
    const offer = offerOf(kind);
    const key = KIND_KEYS[kind];
    const title = t(`${upgradesBase}.kinds.${key}.title`, {});
    const running = upgrades.purchases.filter((purchase) => purchase.kind === kind);

    // what runs, each with its end and a button to add time while it is sold
    for (const purchase of running) {
      rows.push({
        key: purchase.uuid,
        title,
        body: t(`${upgradesBase}.kinds.${key}.running`, {}),
        note: [runsUntil(purchase.until), kind === 'boost' ? t(`${upgradesBase}.kinds.boost.restart`, {}) : '']
          .filter(Boolean)
          .join(' '),
        action: offer
          ? { label: add(offer), onClick: () => buy({ kind, renew: single(kind) ? null : purchase.uuid }) }
          : undefined,
      });
    }

    // a new one: not for a pass or a boost that runs, not past the most that may run, not where it adds nothing
    if (!offer || offer.unavailable === 'max' || offer.unavailable === 'unlimited') continue;
    if (single(kind) && running.length > 0) continue;

    rows.push({
      key: kind,
      title,
      body: describe(offer),
      note: [
        running.length > 0 ? t(`${upgradesBase}.count`, { count: running.length, max: offer.max }) : '',
        offer.unavailable ? t(`${upgradesBase}.unavailable.${UNAVAILABLE[offer.unavailable]}`, {}) : '',
      ]
        .filter(Boolean)
        .join(' '),
      action: {
        label: t(`${upgradesBase}.buy`, { price: tItem('token', offer.tokens) }),
        disabled: offer.unavailable !== null,
        onClick: () => buy({ kind }),
      },
    });
  }

  // the card colour: bought for good, changed for free
  const colorOffer = offerOf('card_color');
  const colorBought = upgrades.purchases.some((purchase) => purchase.kind === 'card_color');
  if (colorBought || colorOffer) {
    rows.push({
      key: 'card_color',
      title: t(`${upgradesBase}.kinds.cardColor.title`, {}),
      body: colorBought ? t(`${upgradesBase}.kinds.cardColor.running`, {}) : colorOffer ? describe(colorOffer) : '',
      color: colorBought ? upgrades.cardColor : null,
      action: colorBought
        ? { label: t(`${upgradesBase}.color.change`, {}), onClick: pickColor }
        : colorOffer
          ? { label: t(`${upgradesBase}.buy`, { price: tItem('token', colorOffer.tokens) }), onClick: pickColor }
          : undefined,
    });
  }

  return rows;
}

function UpgradeRows({ upgrades }: { upgrades: Upgrades }) {
  const { t, tItem } = useTranslations();
  const [request, setRequest] = useState<UpgradeRequest | null>(null);
  const [picking, setPicking] = useState(false);
  const rows = useRows(upgrades, setRequest, () => setPicking(true));

  return (
    <SettingsSection
      id='upgrades'
      title={t(`${upgradesBase}.title`, {})}
      description={t(`${upgradesBase}.note`, { balance: tItem('token', upgrades.balance) })}
    >
      {request && <UpgradeConfirmModal upgrades={upgrades} request={request} onClose={() => setRequest(null)} />}
      <CardColorModal opened={picking} upgrades={upgrades} onClose={() => setPicking(false)} />

      <div className='theme-eco-card'>
        <div className='theme-rows'>
          {rows.map((row) => (
            <div key={row.key} className='theme-row'>
              <div className='theme-row-text'>
                <h3 className='theme-row-title'>{row.title}</h3>
                <p className='theme-row-body'>
                  {row.color && (
                    <span
                      className='theme-card-swatch-dot'
                      data-card-color={row.color}
                      role='img'
                      aria-label={colorName(t, row.color)}
                    />
                  )}
                  {row.body}
                </p>
                {row.note && <p className='theme-row-body'>{row.note}</p>}
              </div>
              {row.action && (
                <div className='theme-row-actions'>
                  <Button variant='default' size='sm' disabled={row.action.disabled} onClick={row.action.onClick}>
                    {row.action.label}
                  </Button>
                </div>
              )}
            </div>
          ))}
        </div>
      </div>
    </SettingsSection>
  );
}

export default function UpgradesSection() {
  const { resource, canUpgrade } = useServerUpgrades();
  const upgrades = resource.data;

  if (!canUpgrade || !upgrades || (upgrades.offers.length === 0 && upgrades.purchases.length === 0)) {
    return null;
  }

  return <UpgradeRows upgrades={upgrades} />;
}
