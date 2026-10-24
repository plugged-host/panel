// plugged.host: a currency's recent history as full-width rows, the Pelican panel's ledger: what
// happened and when at the start, the amount at the end, no table to scroll sideways on a phone
// (docs/features/economy.md).
import classNames from 'classnames';
import { z } from 'zod';
import { typeKey } from '@/elements/economy/TransactionsTable.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import { economyTransactionSchema } from '@/lib/schemas/user/economy.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type Transaction = z.infer<typeof economyTransactionSchema>;

const strings = 'elements.economyTransactions';

/** What a row says happened: the ledger's type in words, with the stored detail where it adds any. */
function useDescribe() {
  const { t } = useTranslations();

  return (transaction: Transaction): string => {
    const key = typeKey(transaction.type);

    if (key === null) {
      return transaction.description;
    }
    if (key === 'earn') {
      return t(`${strings}.earnedWith`, { provider: transaction.description });
    }

    const label = t(`${strings}.type.${key}`, {});

    // these descriptions are the label again, or already say it all (the subdomain's name)
    if (key === 'starting' || key === 'daily') {
      return label;
    }
    if (key === 'subdomain' || key === 'subdomainRefund') {
      return transaction.description || label;
    }

    return transaction.description ? t(`${strings}.detail`, { type: label, detail: transaction.description }) : label;
  };
}

/**
 * The credit history (`ledger`: what, when, the balance after, a signed amount) or the Tokens page's
 * recent activity (`activity`: what and when). The caller draws the box around the rows.
 */
export default function TransactionRows({
  transactions,
  variant,
}: {
  transactions: Transaction[];
  variant: 'ledger' | 'activity';
}) {
  const { t, language } = useTranslations();
  const describe = useDescribe();
  const number = new Intl.NumberFormat(language);
  const ledger = variant === 'ledger';

  return (
    <>
      {transactions.map((transaction, index) => (
        <div
          key={`${transaction.created?.toISOString() ?? ''}-${index}`}
          className={ledger ? 'theme-eco-tx' : 'theme-earn-tx'}
        >
          <div className='flex min-w-0 flex-col'>
            <span className={ledger ? 'theme-eco-text' : 'theme-earn-text truncate'}>{describe(transaction)}</span>
            <span className={ledger ? 'theme-eco-small tabular-nums' : 'theme-earn-small'}>
              {transaction.created && <FormattedTimestamp timestamp={transaction.created} />}
              {ledger && transaction.created && ' · '}
              {ledger && t(`${strings}.balanceAfter`, { balance: number.format(transaction.balanceAfter) })}
            </span>
          </div>

          <span
            className={classNames(
              'theme-eco-amount',
              transaction.amount > 0 ? 'theme-eco-amount-in' : 'theme-eco-amount-out',
            )}
          >
            {transaction.amount > 0 ? '+' : transaction.amount < 0 ? '−' : ''}
            {number.format(Math.abs(transaction.amount))}
          </span>
        </div>
      ))}
    </>
  );
}
