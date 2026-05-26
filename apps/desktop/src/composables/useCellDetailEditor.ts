import { shallowRef, onBeforeUnmount, type ShallowRef } from "vue";
import { EditorState, Compartment } from "@codemirror/state";
import {
  EditorView,
  keymap,
  drawSelection,
  dropCursor,
  highlightSpecialChars,
  highlightActiveLine,
} from "@codemirror/view";
import { json } from "@codemirror/lang-json";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { bracketMatching } from "@codemirror/language";
import { searchKeymap, search as cmSearch } from "@codemirror/search";
import { loadEditorTheme, editorFontTheme } from "@/lib/editorThemes";
import { isJsonColumnType } from "@/lib/cellDetailPresentation";
import type { EditorTheme } from "@/stores/settingsStore";
import type { AppThemeAppearance } from "@/lib/appTheme";

export interface UseCellDetailEditorOptions {
  onChange?: (value: string) => void;
  onEscape?: () => void;
  onBlur?: () => void;
  readOnly?: boolean;
  editorTheme: () => EditorTheme;
  appAppearance: () => AppThemeAppearance;
  fontSize: () => number;
  fontFamily: () => string;
}

export interface UseCellDetailEditorReturn {
  create: (parent: HTMLElement, initialValue: string, columnType?: string) => Promise<void>;
  setValue: (value: string, columnType?: string) => void;
  getValue: () => string;
  destroy: () => void;
  view: Readonly<ShallowRef<EditorView | null>>;
}

function looksLikeJsonString(text: string): boolean {
  const trimmed = text.trim();
  return trimmed.startsWith("{") || trimmed.startsWith("[");
}

function shouldUseJsonMode(columnType?: string, value?: string): boolean {
  if (isJsonColumnType(columnType)) return true;
  if (value && looksLikeJsonString(value)) return true;
  return false;
}

export function useCellDetailEditor(options: UseCellDetailEditorOptions): UseCellDetailEditorReturn {
  const view = shallowRef<EditorView | null>(null) as ShallowRef<EditorView | null>;
  const languageComp = new Compartment();
  const themeComp = new Compartment();
  const fontThemeComp = new Compartment();

  let destroyed = false;
  let currentIsJson = false;

  async function create(parent: HTMLElement, initialValue: string, columnType?: string): Promise<void> {
    if (destroyed) return;

    const doc = initialValue ?? "";
    currentIsJson = shouldUseJsonMode(columnType, doc);

    const theme = await loadEditorTheme(options.editorTheme(), options.appAppearance());
    const fontTheme = editorFontTheme(EditorView, options.fontSize(), options.fontFamily(), { scrollable: false });

    const state = EditorState.create({
      doc,
      extensions: [
        // Minimal setup without line numbers
        highlightSpecialChars(),
        history(),
        drawSelection(),
        dropCursor(),
        highlightActiveLine(),
        EditorView.theme({
          ".cm-activeLine": {
            backgroundColor: "color-mix(in oklch, var(--foreground) 4%, transparent)",
          },
        }),
        EditorState.allowMultipleSelections.of(true),
        bracketMatching(),
        keymap.of([...defaultKeymap, ...historyKeymap, ...searchKeymap]),
        // Feature extensions
        cmSearch({ top: true }),
        EditorView.lineWrapping,
        languageComp.of(currentIsJson ? json() : []),
        themeComp.of(theme),
        fontThemeComp.of(fontTheme),
        keymap.of([
          {
            key: "Escape",
            run: () => {
              options.onEscape?.();
              return true;
            },
          },
        ]),
        EditorView.updateListener.of((update) => {
          if (update.docChanged) {
            options.onChange?.(update.state.doc.toString());
          }
        }),
        EditorView.domEventHandlers({
          blur: () => {
            options.onBlur?.();
          },
        }),
        EditorState.readOnly.of(!!options.readOnly),
        EditorView.editable.of(!options.readOnly),
      ],
    });

    view.value = new EditorView({ state, parent });
  }

  function setValue(value: string, columnType?: string) {
    const editor = view.value;
    if (!editor || destroyed) return;

    const text = value ?? "";
    const newIsJson = shouldUseJsonMode(columnType, text);
    const effects: ReturnType<typeof Compartment.prototype.reconfigure>[] = [];

    if (newIsJson !== currentIsJson) {
      effects.push(languageComp.reconfigure(newIsJson ? json() : []));
      currentIsJson = newIsJson;
    }

    editor.dispatch({
      changes: { from: 0, to: editor.state.doc.length, insert: text },
      effects,
    });
  }

  function getValue(): string {
    return view.value?.state.doc.toString() ?? "";
  }

  function destroy() {
    if (destroyed) return;
    destroyed = true;
    view.value?.destroy();
    view.value = null;
  }

  onBeforeUnmount(() => {
    destroy();
  });

  return { create, setValue, getValue, destroy, view };
}
