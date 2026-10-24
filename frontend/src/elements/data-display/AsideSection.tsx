// plugged.host: Filament's aside section (`->aside()`): heading and description beside a card of fields (app.css .theme-aside)
import { ReactNode } from 'react';

export default function AsideSection({
  title,
  description,
  children,
}: {
  title: string;
  description: string;
  children: ReactNode;
}) {
  return (
    <section className='theme-aside'>
      <div className='theme-aside-head'>
        <h3 className='theme-aside-heading'>{title}</h3>
        <p className='theme-aside-description'>{description}</p>
      </div>
      <div className='theme-aside-body'>{children}</div>
    </section>
  );
}
