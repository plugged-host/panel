import EditorWorker from 'monaco-editor/editor/editor.worker.js?worker';
import CssWorker from 'monaco-editor/language/css/css.worker.js?worker';
import HtmlWorker from 'monaco-editor/language/html/html.worker.js?worker';
import JsonWorker from 'monaco-editor/language/json/json.worker.js?worker';
import TsWorker from 'monaco-editor/language/typescript/ts.worker.js?worker';

export type Monaco = typeof import('monaco-editor');
export type MonacoCodeEditor = import('monaco-editor').editor.IStandaloneCodeEditor;
export type MonacoDiffEditor = import('monaco-editor').editor.IStandaloneDiffEditor;

export type BeforeMount = (monaco: Monaco) => void;
export type OnMount = (editor: MonacoCodeEditor, monaco: Monaco) => void;
export type DiffBeforeMount = (monaco: Monaco) => void;
export type DiffOnMount = (editor: MonacoDiffEditor, monaco: Monaco) => void;
export type OnChange = (
  value: string | undefined,
  ev: import('monaco-editor').editor.IModelContentChangedEvent,
) => void;
export type OnValidate = (markers: import('monaco-editor').editor.IMarker[]) => void;

function getWorker(_workerId: string, label: string): Worker {
  switch (label) {
    case 'json':
      return new JsonWorker();
    case 'css':
    case 'scss':
    case 'less':
      return new CssWorker();
    case 'html':
    case 'handlebars':
    case 'razor':
      return new HtmlWorker();
    case 'typescript':
    case 'javascript':
      return new TsWorker();
    default:
      return new EditorWorker();
  }
}

let monacoInstance: Monaco | null = null;
let monacoPromise: Promise<Monaco> | null = null;

/**
 * Lazily loads the bundled monaco-editor. The import is deferred until an editor first mounts
 * so the active translation (`_VSCODE_NLS_MESSAGES`) is in place before monaco evaluates its strings.
 */
export function loadMonaco(): Promise<Monaco> {
  if (!monacoPromise) {
    // globalAPI keeps `window.monaco` available, as the previous AMD loader exposed it to extensions
    globalThis.MonacoEnvironment = { ...globalThis.MonacoEnvironment, globalAPI: true, getWorker };

    monacoPromise = import('monaco-editor').then(
      (monaco) => (monacoInstance = monaco),
      (error) => {
        monacoPromise = null;
        throw error;
      },
    );
  }

  return monacoPromise;
}

export function getLoadedMonacoOrNull(): Monaco | null {
  return monacoInstance;
}

export function getLoadedMonaco(): Monaco {
  if (!monacoInstance) {
    throw new Error('monaco-editor accessed before it was loaded');
  }

  return monacoInstance;
}
