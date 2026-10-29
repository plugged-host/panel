// plugged.host: Theme v2's usage card and balances at the foot of the sidebar (docs/features/economy.md)
import { useQuery } from '@tanstack/react-query';
import classNames from 'classnames';
import { ReactNode } from 'react';
import { Link } from 'react-router';
import { z } from 'zod';
import getBalance from '@/api/me/economy/getBalance.ts';
import { useCloseMobileMenu } from '@/elements/navigation/Sidebar.tsx';
import { bytesToString, mbToBytes } from '@/lib/format/size.ts';
import { intlLocale } from '@/lib/languages.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { userBalanceSchema } from '@/lib/schemas/user/economy.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';

/** Whether the card shows anything: a usage row (a limit above 0) or a balance (the welcome tour asks too). */
export function balanceCardShown(balance: z.infer<typeof userBalanceSchema>): boolean {
  const { limit } = balance.allowance;

  return limit.cpu > 0 || limit.memory > 0 || limit.disk > 0 || balance.tokens !== null || balance.credits !== null;
}

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

// The plural item ("{count} tokens", in the language's word order and plural form) with its number
// in the value style and the words around it in the label style.
function BalanceAmount({ text, value }: { text: string; value: string }) {
  const at = text.indexOf(value);
  if (at < 0) {
    return <span className='theme-balance-label'>{text}</span>;
  }

  const before = text.slice(0, at).trim();
  const after = text.slice(at + value.length).trim();

  return (
    <>
      {before && <span className='theme-balance-label'>{before}</span>}
      <span className='theme-balance-value'>{value}</span>
      {after && <span className='theme-balance-label'>{after}</span>}
    </>
  );
}

export default function SidebarBalance() {
  const { t, tItem, language } = useTranslations();
  const economy = useGlobalStore((state) => state.settings.economy);

  const { data: balance } = useQuery({
    queryKey: queryKeys.user.balance(),
    queryFn: getBalance,
    refetchInterval: 60_000,
  });

  if (!balance || !balanceCardShown(balance)) {
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
    { key: 'token' as const, value: balance.tokens, to: economy?.tokensEnabled ? '/account/tokens' : null },
    { key: 'credit' as const, value: balance.credits, to: storeUrl },
  ].filter((pill) => pill.value !== null);

  const number = new Intl.NumberFormat(intlLocale(language));

  // plugged.host: tokens toward the cheapest membership the account does not have (none with the
  // second membership, or with memberships off)
  const goal =
    balance.tokens !== null && balance.membershipGoal && economy?.membershipEnabled
      ? { ...balance.membershipGoal, tokens: balance.tokens }
      : null;

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
        <div className={classNames('theme-balance-pills', goal && 'theme-balance-pills-goal')}>
          {pills.map((pill) => (
            <Part
              key={pill.key}
              to={pill.to}
              title={t(
                pill.key === 'token' ? 'elements.sidebar.balance.openTokens' : 'elements.sidebar.balance.openStore',
                {},
              )}
              className='theme-balance-item'
            >
              <BalanceAmount text={tItem(pill.key, pill.value ?? 0)} value={number.format(pill.value ?? 0)} />
              {/* plugged.host: today's streak tokens wait on the Tokens page (docs/features/economy.md#daily-streak) */}
              {pill.key === 'token' && balance.streakClaimable && (
                <span className='theme-balance-dot' role='img' aria-label={t('elements.sidebar.balance.streak', {})} />
              )}
            </Part>
          ))}

          {goal && (
            <Part
              to='/account/membership'
              title={t('elements.sidebar.balance.openMembership', {})}
              className='theme-balance-goal'
            >
              <div className='theme-balance-goal-text'>
                {goal.tokens >= goal.cost
                  ? t('elements.sidebar.balance.goalReady', { name: goal.name })
                  : t('elements.sidebar.balance.goal', {
                      tokens: number.format(goal.tokens),
                      cost: number.format(goal.cost),
                      name: goal.name,
                    })}
              </div>
              <div className='theme-balance-track' aria-hidden='true'>
                <div
                  className='theme-balance-bar'
                  style={{ width: `${Math.min(100, Math.round((goal.tokens / goal.cost) * 100))}%` }}
                />
              </div>
            </Part>
          )}
        </div>
      )}
    </div>
  );
}
