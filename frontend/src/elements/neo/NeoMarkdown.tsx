// plugged.host: an answer of neo's as markdown (GitHub's flavour), styled like the Pelican panel's
// chat (.theme-neo-answer in app.css). Raw HTML stays text and images stay links, so an answer
// cannot load anything from elsewhere.
import { ReactNode } from 'react';
import Markdown, { Components } from 'react-markdown';
import remarkGfm from 'remark-gfm';

// links leave the panel in a new tab; react-markdown already drops unsafe schemes
function Link({ href, children }: { href?: string; children?: ReactNode }) {
  if (!href) {
    return <span>{children}</span>;
  }

  return (
    <a href={href} target='_blank' rel='noopener noreferrer'>
      {children}
    </a>
  );
}

const components: Components = {
  a: ({ href, children }) => <Link href={href}>{children}</Link>,
  img: ({ src, alt }) => {
    const href = typeof src === 'string' ? src : undefined;

    return <Link href={href}>{alt || href}</Link>;
  },
};

export default function NeoMarkdown({ text }: { text: string }) {
  return (
    <div className='theme-neo-answer'>
      <Markdown remarkPlugins={[remarkGfm]} components={components}>
        {text}
      </Markdown>
    </div>
  );
}
