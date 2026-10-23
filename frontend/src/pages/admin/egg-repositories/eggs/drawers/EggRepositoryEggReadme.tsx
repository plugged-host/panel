import Markdown from 'react-markdown';
import rehypeRaw from 'rehype-raw';
import rehypeSanitize from 'rehype-sanitize';
import remarkGfm from 'remark-gfm';
import { markdownComponents } from '@/providers/TranslationProvider.tsx';

export default function EggRepositoryEggReadme({ readme }: { readme: string }) {
  return (
    <div className='text-sm wrap-break-word'>
      <Markdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeRaw, rehypeSanitize]} components={markdownComponents}>
        {readme}
      </Markdown>
    </div>
  );
}
