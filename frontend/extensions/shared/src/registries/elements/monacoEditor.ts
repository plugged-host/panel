import { Registry } from 'shared';
import type { DiffOnMount, OnMount } from '@/lib/editor/monacoLoader.ts';

export class MonacoEditorRegistry implements Registry {
  public mergeFrom(other: this): this {
    this.onMountHandlers.push(...other.onMountHandlers);
    this.diffOnMountHandlers.push(...other.diffOnMountHandlers);

    return this;
  }

  public onMountHandlers: OnMount[] = [];
  public diffOnMountHandlers: DiffOnMount[] = [];

  public addOnMountHandler(handler: OnMount): this {
    this.onMountHandlers.push(handler);
    return this;
  }

  public addDiffOnMountHandler(handler: DiffOnMount): this {
    this.diffOnMountHandlers.push(handler);
    return this;
  }
}
