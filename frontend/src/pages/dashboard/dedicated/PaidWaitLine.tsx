// plugged.host: the start queue's and the waiting room's line about Paid, whose servers never wait (docs/features/dedicated.md)
import usePaidOffer from './usePaidOffer.ts';

const PATH = '/account/dedicated';

/** `leave` takes the page away (the queue); without it the link opens in a new tab (the waiting room holds the page). */
export default function PaidWaitLine({ leave }: { leave?: (path: string) => void }) {
  const paid = usePaidOffer();
  const { t } = paid;

  if (!paid.offered) {
    return null;
  }

  return (
    <p className='theme-eco-small'>
      {paid.from
        ? t('pages.account.dedicated.upsell.wait', { name: paid.name, price: paid.from })
        : t('pages.account.dedicated.upsell.waitNoPrice', {})}{' '}
      <a
        className='theme-eco-link'
        href={PATH}
        {...(leave
          ? {
              onClick: (event) => {
                event.preventDefault();
                leave(PATH);
              },
            }
          : { target: '_blank', rel: 'noopener' })}
      >
        {t('pages.account.dedicated.upsell.seePlans', {})}
      </a>
    </p>
  );
}
