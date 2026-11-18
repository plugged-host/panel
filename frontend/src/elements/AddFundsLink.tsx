// plugged.host: where an account short of noprofit.space Funds adds them, and euro amounts as the panel shows them (docs/features/dedicated.md)
import { intlLocale } from '@/lib/languages.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';

/** Euro cents as the visitor's language writes euros. */
export function formatEuro(cents: number, language: string): string {
  return new Intl.NumberFormat(intlLocale(language), { style: 'currency', currency: 'EUR' }).format(cents / 100);
}

/** noprofit.space's Funds page, or the same words without a link when its address is not known. */
export default function AddFundsLink() {
  const { t } = useTranslations();
  const accountsUrl = useGlobalStore((state) => state.settings.noprofit?.accountsUrl);
  const label = t('pages.account.dedicated.addFunds', {});

  if (!accountsUrl) {
    return <span className='text-sm'>{label}</span>;
  }

  return (
    <a
      href={`${accountsUrl.replace(/\/+$/, '')}/funds`}
      target='_blank'
      rel='noopener noreferrer'
      className='theme-eco-link text-sm'
    >
      {label}
    </a>
  );
}
