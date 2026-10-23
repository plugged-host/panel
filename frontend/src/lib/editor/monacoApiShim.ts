import { getLoadedMonaco, type Monaco } from '@/lib/editor/monacoLoader.ts';

// y-monaco statically imports monaco's editor API; this alias defers it to the lazily loaded instance
// so pulling in the collab binding doesn't drag the whole editor into the importing chunk.

export class Range {
  constructor(...args: ConstructorParameters<Monaco['Range']>) {
    return new (getLoadedMonaco().Range)(...args) as Range;
  }
}

export class Selection {
  constructor(...args: ConstructorParameters<Monaco['Selection']>) {
    return new (getLoadedMonaco().Selection)(...args) as Selection;
  }
}

export const SelectionDirection = new Proxy(
  {},
  {
    get: (_target, prop) =>
      (getLoadedMonaco().SelectionDirection as unknown as Record<string, unknown>)[prop as string],
  },
);
