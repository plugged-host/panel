// plugged.host: a credit or token history (docs/features/economy.md)
import { z } from 'zod';
import Table, { TableData, TableRow } from '@/elements/data-display/Table.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import Text from '@/elements/typography/Text.tsx';
import { economyTransactionSchema } from '@/lib/schemas/user/economy.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

const typeKeys = {
  starting: 'starting',
  daily: 'daily',
  resource_purchase: 'store',
  plus: 'plus',
  plusplus: 'plusplus',
  admin: 'admin',
  earn_reversal: 'earnReversal', // a survey the provider took back (docs/features/economy.md)
  limbo: 'limbo', // plugged.host: docs/features/limbo.md
  limbo_refund: 'limboRefund',
  subdomain: 'subdomain', // docs/features/subdomains.md
  subdomain_refund: 'subdomainRefund',
  discord_invite: 'discordInvite', // docs/features/discord.md
  discord_invite_clawback: 'discordInviteClawback',
  discord_invite_debt: 'discordInviteDebt',
  discord_boost: 'discordBoost',
  partner: 'partner', // plugged.host: docs/features/partners.md
} as const;

export type TypeKey = (typeof typeKeys)[keyof typeof typeKeys] | 'earn';

export function typeKey(type: string): TypeKey | null {
  if (type.startsWith('earn:')) return 'earn';

  return type in typeKeys ? typeKeys[type as keyof typeof typeKeys] : null;
}

export default function TransactionsTable({
  transactions,
}: {
  transactions: z.infer<typeof economyTransactionSchema>[];
}) {
  const { t } = useTranslations();
  const page = 'elements.economyTransactions';

  if (transactions.length === 0) {
    return (
      <Text size='sm' c='dimmed'>
        {t(`${page}.empty`, {})}
      </Text>
    );
  }

  return (
    <Table
      columns={[
        t(`${page}.columns.what`, {}),
        t(`${page}.columns.amount`, {}),
        t(`${page}.columns.balance`, {}),
        t(`${page}.columns.when`, {}),
      ]}
    >
      {transactions.map((transaction, index) => {
        const key = typeKey(transaction.type);

        return (
          <TableRow key={`${transaction.created?.toISOString() ?? ''}-${index}`}>
            <TableData>
              {key ? t(`${page}.type.${key}`, {}) : transaction.description}
              {key && transaction.description && (
                <Text size='xs' c='dimmed'>
                  {transaction.description}
                </Text>
              )}
            </TableData>
            <TableData>
              {transaction.amount > 0 ? `+${transaction.amount.toLocaleString()}` : transaction.amount.toLocaleString()}
            </TableData>
            <TableData>{transaction.balanceAfter.toLocaleString()}</TableData>
            <TableData>
              {transaction.created ? <FormattedTimestamp timestamp={transaction.created} /> : t('common.na', {})}
            </TableData>
          </TableRow>
        );
      })}
    </Table>
  );
}
