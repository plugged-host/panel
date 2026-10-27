// plugged.host: one section of the server's Settings page, grouped as v3's own pages group theirs (Transfer,
// Limbo, the account's Discord and Partners pages): the heading and its note on the page, the section's one
// box under them (".theme-eco-card" for fields, ".theme-eco-list" for rows). `id` is the section's anchor,
// kept clear of the phone's menu button when the page opens at it (docs/features/server-settings.md)
import { ReactNode, useId } from 'react';
import { SettingsSectionId } from './useSettingsSections.ts';

// a field's helper text under its input, so fields side by side keep their inputs on one line
export const HELPER_UNDER: ('label' | 'input' | 'description' | 'error')[] = ['label', 'input', 'description', 'error'];

export default function SettingsSection({
  id,
  title,
  description,
  children,
}: {
  id: SettingsSectionId;
  title: string;
  description?: string;
  children: ReactNode;
}) {
  const headingId = useId();

  return (
    <section id={id} aria-labelledby={headingId} className='flex scroll-mt-20 flex-col gap-3 lg:scroll-mt-8'>
      <div className='flex flex-col gap-1'>
        <h2 id={headingId} className='theme-eco-heading'>
          {title}
        </h2>
        {description && <p className='theme-eco-muted max-w-2xl'>{description}</p>}
      </div>
      {children}
    </section>
  );
}
