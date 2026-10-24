// plugged.host: a chat image: one of neo's images route (`uuid`), or one on its way (`src`, a data
// URL). The session cookie authorizes a plain <img>; while an admin impersonates someone, the image
// is fetched with the impersonation header instead.
import { useEffect, useState } from 'react';
import { axiosInstance, getImpersonatedUser } from '@/api/axios.ts';
import { neoImageUrl } from '@/lib/neo.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

export default function NeoImage({ uuid, src: given }: { uuid?: string; src?: string }) {
  const { t } = useTranslations();
  const impersonating = !!uuid && !!getImpersonatedUser();
  const [fetched, setFetched] = useState<string | null>(null);

  useEffect(() => {
    if (!uuid || !impersonating) return;

    let url: string | null = null;
    let cancelled = false;
    axiosInstance
      .get(neoImageUrl(uuid), { responseType: 'blob' })
      .then(({ data }) => {
        if (cancelled) return;
        url = URL.createObjectURL(data as Blob);
        setFetched(url);
      })
      .catch(() => undefined);

    return () => {
      cancelled = true;
      if (url) URL.revokeObjectURL(url);
    };
  }, [uuid, impersonating]);

  const src = given ?? (uuid ? (impersonating ? fetched : neoImageUrl(uuid)) : null);
  if (!src) {
    return <span className='theme-neo-image' />;
  }

  const image = <img src={src} alt={t('elements.neo.message.image', {})} loading='lazy' className='theme-neo-image' />;

  // a data URL cannot be opened in a tab of its own
  return given ? (
    image
  ) : (
    <a href={src} target='_blank' rel='noopener noreferrer'>
      {image}
    </a>
  );
}
