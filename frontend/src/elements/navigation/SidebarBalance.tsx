// plugged.host: Theme v2's usage card and balances at the foot of the sidebar (docs/features/economy.md)
import { useQuery } from '@tanstack/react-query';
import classNames from 'classnames';
import { ReactNode } from 'react';
import { Link } from 'react-router';
import getBalance from '@/api/me/economy/getBalance.ts';
import { useCloseMobileMenu } from '@/elements/navigation/Sidebar.tsx';
import { bytesToString, mbToBytes } from '@/lib/format/size.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';

interface UsageRow {
  key: 'cpu' | 'memory' | 'disk';
  used: string;
  limit: string;
  percent: number;
  full: boolean;
}

// "200 / 500%", "512 MiB / 16 GiB": a limit of 0 is unlimited, and such a row is left out.
function usageRow(key: UsageRow['key'], used: number, limit: number): UsageRow | null {
  if (limit <= 0) {
    return null;
  }

  const format = key === 'cpu' ? (value: number) => `${value}%` : (value: number) => bytesToString(mbToBytes(value), 1);

  return {
    key,
    used: key === 'cpu' ? String(used) : used > 0 ? format(used) : '0',
    limit: format(limit),
    percent: Math.min(100, Math.round((used / limit) * 100)),
    full: used >= limit,
  };
}

// A part leads to where it is topped up, or is plain text when that page is off.
function Part({
  to,
  title,
  className,
  children,
}: {
  to: string | null;
  title: string;
  className: string;
  children: ReactNode;
}) {
  const closeMobileMenu = useCloseMobileMenu();

  return to ? (
    <Link
      to={to}
      title={title}
      className={classNames(className, 'theme-balance-linked')}
      onClick={() => closeMobileMenu?.()}
    >
      {children}
    </Link>
  ) : (
    <div className={className}>{children}</div>
  );
}

export default function SidebarBalance() {
  const { t, language } = useTranslations();
  const economy = useGlobalStore((state) => state.settings.economy);

  const { data: balance } = useQuery({
    queryKey: queryKeys.user.balance(),
    queryFn: getBalance,
    refetchInterval: 60_000,
  });

  if (!balance) {
    return null;
  }

  const { limit, used } = balance.allowance;
  const rows = [
    usageRow('cpu', used.cpu, limit.cpu),
    usageRow('memory', used.memory, limit.memory),
    usageRow('disk', used.disk, limit.disk),
  ].filter((row) => row !== null);

  const storeUrl = economy?.creditsEnabled ? '/account/store' : null;
  const pills = [
    { key: 'tokens' as const, value: balance.tokens, to: economy?.tokensEnabled ? '/account/tokens' : null },
    { key: 'credits' as const, value: balance.credits, to: storeUrl },
  ].filter((pill) => pill.value !== null);

  if (rows.length === 0 && pills.length === 0) {
    return null;
  }

  const number = new Intl.NumberFormat(language);

  return (
    <div className='theme-balance'>
      {rows.length > 0 && (
        <Part to={storeUrl} title={t('elements.sidebar.balance.openStore', {})} className='theme-balance-usage'>
          {rows.map((row) => (
            <div key={row.key} className='theme-balance-row'>
              <div className='theme-balance-row-info'>
                <span className='theme-balance-row-name'>{t(`elements.sidebar.balance.${row.key}`, {})}</span>
                <span className='theme-balance-row-metrics'>
                  <span className={classNames('theme-balance-row-value', row.full && 'theme-balance-full')}>
                    {row.used}
                  </span>
                  <span className='theme-balance-row-limit'>/ {row.limit}</span>
                </span>
              </div>
              <div className='theme-balance-track'>
                <div
                  className={classNames('theme-balance-bar', row.full && 'theme-balance-bar-full')}
                  style={{ width: `${row.percent}%` }}
                />
              </div>
            </div>
          ))}
        </Part>
      )}

      {pills.length > 0 && (
        <div className='theme-balance-pills'>
          {pills.map((pill) => (
            <Part
              key={pill.key}
              to={pill.to}
              title={t(
                pill.key === 'tokens' ? 'elements.sidebar.balance.openTokens' : 'elements.sidebar.balance.openStore',
                {},
              )}
              className='theme-balance-item'
            >
              <span className='theme-balance-value'>{number.format(pill.value ?? 0)}</span>
              <span className='theme-balance-label'>{t(`elements.sidebar.balance.${pill.key}`, {})}</span>
            </Part>
          ))}
        </div>
      )}
    </div>
  );
}
