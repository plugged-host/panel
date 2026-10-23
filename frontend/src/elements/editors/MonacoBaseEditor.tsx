import {
  type CSSProperties,
  type DependencyList,
  type EffectCallback,
  ReactNode,
  RefObject,
  useEffect,
  useRef,
  useState,
} from 'react';
import {
  type BeforeMount,
  type DiffBeforeMount,
  type DiffOnMount,
  getLoadedMonacoOrNull,
  loadMonaco,
  type Monaco,
  type MonacoCodeEditor,
  type MonacoDiffEditor,
  type OnChange,
  type OnMount,
  type OnValidate,
} from '@/lib/editor/monacoLoader.ts';

type ViewState = import('monaco-editor').editor.ICodeEditorViewState;
type EditorOptions = import('monaco-editor').editor.IStandaloneEditorConstructionOptions;
type DiffEditorOptions = import('monaco-editor').editor.IDiffEditorConstructionOptions;
type OverrideServices = import('monaco-editor').editor.IEditorOverrideServices;

interface ContainerProps {
  width?: number | string;
  height?: number | string;
  loading?: ReactNode;
  className?: string;
  wrapperProps?: object;
}

export interface EditorProps extends ContainerProps {
  defaultValue?: string;
  defaultLanguage?: string;
  defaultPath?: string;
  value?: string;
  language?: string;
  path?: string;
  theme?: string;
  line?: number;
  options?: EditorOptions;
  overrideServices?: OverrideServices;
  saveViewState?: boolean;
  keepCurrentModel?: boolean;
  beforeMount?: BeforeMount;
  onMount?: OnMount;
  onChange?: OnChange;
  onValidate?: OnValidate;
}

export interface DiffEditorProps extends ContainerProps {
  original?: string;
  modified?: string;
  language?: string;
  originalLanguage?: string;
  modifiedLanguage?: string;
  originalModelPath?: string;
  modifiedModelPath?: string;
  keepCurrentOriginalModel?: boolean;
  keepCurrentModifiedModel?: boolean;
  theme?: string;
  options?: DiffEditorOptions;
  beforeMount?: DiffBeforeMount;
  onMount?: DiffOnMount;
}

const wrapperStyle: CSSProperties = { display: 'flex', position: 'relative', textAlign: 'initial' };
const loadingStyle: CSSProperties = {
  display: 'flex',
  height: '100%',
  width: '100%',
  justifyContent: 'center',
  alignItems: 'center',
};

const viewStates = new Map<string | undefined, ViewState | null>();

function getOrCreateModel(monaco: Monaco, value: string, language: string, path: string) {
  const uri = path ? monaco.Uri.parse(path) : undefined;
  return (uri && monaco.editor.getModel(uri)) || monaco.editor.createModel(value, language, uri);
}

function useMonacoInstance() {
  const [monaco, setMonaco] = useState<Monaco | null>(getLoadedMonacoOrNull);

  useEffect(() => {
    if (monaco) return;

    let cancelled = false;

    loadMonaco()
      .then((instance) => !cancelled && setMonaco(instance))
      .catch((error) => console.error('Monaco initialization: error:', error));

    return () => {
      cancelled = true;
    };
  }, [monaco]);

  return monaco;
}

function useUpdate(effect: EffectCallback, deps: DependencyList, ready: boolean) {
  const initialRef = useRef(true);

  useEffect(() => {
    if (initialRef.current || !ready) {
      initialRef.current = false;
      return;
    }

    return effect();
  }, deps);
}

function MonacoContainer({
  width,
  height,
  loading,
  className,
  wrapperProps,
  ready,
  containerRef,
}: ContainerProps & { ready: boolean; containerRef: RefObject<HTMLDivElement | null> }) {
  return (
    <section style={{ ...wrapperStyle, width, height }} {...wrapperProps}>
      {!ready && <div style={loadingStyle}>{loading}</div>}
      <div ref={containerRef} style={{ width: '100%', ...(!ready && { display: 'none' }) }} className={className} />
    </section>
  );
}

export function BaseEditor({
  defaultValue,
  defaultLanguage,
  defaultPath,
  value,
  language,
  path,
  theme = 'vs',
  line,
  loading = 'Loading...',
  options = {},
  overrideServices = {},
  saveViewState = true,
  keepCurrentModel = false,
  width = '100%',
  height = '100%',
  className,
  wrapperProps = {},
  beforeMount,
  onMount,
  onChange,
  onValidate,
}: EditorProps) {
  const monaco = useMonacoInstance();
  const [editor, setEditor] = useState<MonacoCodeEditor | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const onMountRef = useRef(onMount);
  const beforeMountRef = useRef(beforeMount);
  const previousPathRef = useRef(path);
  const preventChangeEventRef = useRef(false);
  const disposeRef = useRef<(editor: MonacoCodeEditor) => void>(() => undefined);

  useEffect(() => {
    disposeRef.current = (editor) => {
      if (keepCurrentModel) {
        if (saveViewState) viewStates.set(path, editor.saveViewState());
      } else {
        editor.getModel()?.dispose();
      }

      editor.dispose();
    };
  });

  useEffect(() => {
    if (!monaco || !containerRef.current) return;

    beforeMountRef.current?.(monaco);
    const modelPath = path || defaultPath;

    const created = monaco.editor.create(
      containerRef.current,
      {
        model: getOrCreateModel(
          monaco,
          value || defaultValue || '',
          defaultLanguage || language || '',
          modelPath || '',
        ),
        automaticLayout: true,
        ...options,
      },
      overrideServices,
    );

    if (saveViewState) created.restoreViewState(viewStates.get(modelPath) ?? null);
    monaco.editor.setTheme(theme);
    if (line !== undefined) created.revealLine(line);

    setEditor(created);

    return () => {
      setEditor(null);
      disposeRef.current(created);
    };
  }, [monaco]);

  useEffect(() => {
    if (editor && monaco) onMountRef.current?.(editor, monaco);
  }, [editor]);

  const ready = editor !== null && monaco !== null;

  useUpdate(
    () => {
      if (!editor || !monaco) return;

      const model = getOrCreateModel(
        monaco,
        defaultValue || value || '',
        defaultLanguage || language || '',
        path || defaultPath || '',
      );

      if (model !== editor.getModel()) {
        if (saveViewState) viewStates.set(previousPathRef.current, editor.saveViewState());
        editor.setModel(model);
        if (saveViewState) editor.restoreViewState(viewStates.get(path) ?? null);
      }

      previousPathRef.current = path;
    },
    [path],
    ready,
  );

  useUpdate(() => editor?.updateOptions(options), [options], ready);

  useUpdate(
    () => {
      if (!editor || !monaco || value === undefined) return;

      if (editor.getOption(monaco.editor.EditorOption.readOnly)) {
        editor.setValue(value);
      } else if (value !== editor.getValue()) {
        preventChangeEventRef.current = true;
        editor.executeEdits('', [
          { range: editor.getModel()!.getFullModelRange(), text: value, forceMoveMarkers: true },
        ]);
        editor.pushUndoStop();
        preventChangeEventRef.current = false;
      }
    },
    [value],
    ready,
  );

  useUpdate(
    () => {
      const model = editor?.getModel();
      if (model && language) monaco?.editor.setModelLanguage(model, language);
    },
    [language],
    ready,
  );

  useUpdate(
    () => {
      if (line !== undefined) editor?.revealLine(line);
    },
    [line],
    ready,
  );

  useUpdate(() => monaco?.editor.setTheme(theme), [theme], ready);

  useEffect(() => {
    if (!editor || !onChange) return;

    const subscription = editor.onDidChangeModelContent((event) => {
      if (!preventChangeEventRef.current) onChange(editor.getValue(), event);
    });

    return () => subscription.dispose();
  }, [editor, onChange]);

  useEffect(() => {
    if (!editor || !monaco || !onValidate) return;

    const subscription = monaco.editor.onDidChangeMarkers((uris) => {
      const editorUri = editor.getModel()?.uri;
      if (editorUri && uris.some((uri) => uri.path === editorUri.path)) {
        onValidate(monaco.editor.getModelMarkers({ resource: editorUri }));
      }
    });

    return () => subscription.dispose();
  }, [editor, monaco, onValidate]);

  return (
    <MonacoContainer
      width={width}
      height={height}
      loading={loading}
      className={className}
      wrapperProps={wrapperProps}
      ready={ready}
      containerRef={containerRef}
    />
  );
}

export function BaseDiffEditor({
  original,
  modified,
  language,
  originalLanguage,
  modifiedLanguage,
  originalModelPath,
  modifiedModelPath,
  keepCurrentOriginalModel = false,
  keepCurrentModifiedModel = false,
  theme = 'vs',
  loading = 'Loading...',
  options = {},
  height = '100%',
  width = '100%',
  className,
  wrapperProps = {},
  beforeMount,
  onMount,
}: DiffEditorProps) {
  const monaco = useMonacoInstance();
  const [editor, setEditor] = useState<MonacoDiffEditor | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const onMountRef = useRef(onMount);
  const beforeMountRef = useRef(beforeMount);
  const disposeRef = useRef<(editor: MonacoDiffEditor) => void>(() => undefined);

  useEffect(() => {
    disposeRef.current = (editor) => {
      const models = editor.getModel();
      if (!keepCurrentOriginalModel) models?.original?.dispose();
      if (!keepCurrentModifiedModel) models?.modified?.dispose();

      editor.dispose();
    };
  });

  useEffect(() => {
    if (!monaco || !containerRef.current) return;

    const created = monaco.editor.createDiffEditor(containerRef.current, { automaticLayout: true, ...options });

    beforeMountRef.current?.(monaco);
    created.setModel({
      original: getOrCreateModel(
        monaco,
        original || '',
        originalLanguage || language || 'text',
        originalModelPath || '',
      ),
      modified: getOrCreateModel(
        monaco,
        modified || '',
        modifiedLanguage || language || 'text',
        modifiedModelPath || '',
      ),
    });
    monaco.editor.setTheme(theme);

    setEditor(created);

    return () => {
      setEditor(null);
      disposeRef.current(created);
    };
  }, [monaco]);

  useEffect(() => {
    if (editor && monaco) onMountRef.current?.(editor, monaco);
  }, [editor]);

  const ready = editor !== null && monaco !== null;

  useUpdate(
    () => {
      if (!editor || !monaco) return;

      const originalEditor = editor.getOriginalEditor();
      const model = getOrCreateModel(
        monaco,
        original || '',
        originalLanguage || language || 'text',
        originalModelPath || '',
      );
      if (model !== originalEditor.getModel()) originalEditor.setModel(model);
    },
    [originalModelPath],
    ready,
  );

  useUpdate(
    () => {
      if (!editor || !monaco) return;

      const modifiedEditor = editor.getModifiedEditor();
      const model = getOrCreateModel(
        monaco,
        modified || '',
        modifiedLanguage || language || 'text',
        modifiedModelPath || '',
      );
      if (model !== modifiedEditor.getModel()) modifiedEditor.setModel(model);
    },
    [modifiedModelPath],
    ready,
  );

  useUpdate(
    () => {
      if (!editor || !monaco) return;

      const modifiedEditor = editor.getModifiedEditor();
      if (modifiedEditor.getOption(monaco.editor.EditorOption.readOnly)) {
        modifiedEditor.setValue(modified || '');
      } else if (modified !== modifiedEditor.getValue()) {
        modifiedEditor.executeEdits('', [
          { range: modifiedEditor.getModel()!.getFullModelRange(), text: modified || '', forceMoveMarkers: true },
        ]);
        modifiedEditor.pushUndoStop();
      }
    },
    [modified],
    ready,
  );

  useUpdate(() => editor?.getModel()?.original.setValue(original || ''), [original], ready);

  useUpdate(
    () => {
      const models = editor?.getModel();
      if (!models || !monaco) return;

      monaco.editor.setModelLanguage(models.original, originalLanguage || language || 'text');
      monaco.editor.setModelLanguage(models.modified, modifiedLanguage || language || 'text');
    },
    [language, originalLanguage, modifiedLanguage],
    ready,
  );

  useUpdate(() => monaco?.editor.setTheme(theme), [theme], ready);

  useUpdate(() => editor?.updateOptions(options), [options], ready);

  return (
    <MonacoContainer
      width={width}
      height={height}
      loading={loading}
      className={className}
      wrapperProps={wrapperProps}
      ready={ready}
      containerRef={containerRef}
    />
  );
}
