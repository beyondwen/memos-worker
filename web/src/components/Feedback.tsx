import { createContext } from "preact";
import type { ComponentChildren } from "preact";
import { useCallback, useContext, useRef, useState } from "preact/hooks";

type ToastKind = "success" | "error" | "info";

interface ToastState {
  id: number;
  message: string;
  kind: ToastKind;
}

interface ConfirmState {
  title: string;
  message?: string;
  confirmText: string;
  danger: boolean;
  resolve: (value: boolean) => void;
}

interface FeedbackApi {
  notify: (message: string, kind?: ToastKind) => void;
  confirm: (options: {
    title: string;
    message?: string;
    confirmText?: string;
    danger?: boolean;
  }) => Promise<boolean>;
}

const FeedbackContext = createContext<FeedbackApi | null>(null);

export function FeedbackProvider({ children }: { children: ComponentChildren }) {
  const [toast, setToast] = useState<ToastState | null>(null);
  const [confirmState, setConfirmState] = useState<ConfirmState | null>(null);
  const toastTimer = useRef<number | null>(null);

  const notify = useCallback((message: string, kind: ToastKind = "info") => {
    if (toastTimer.current) window.clearTimeout(toastTimer.current);
    setToast({ id: Date.now(), message, kind });
    toastTimer.current = window.setTimeout(() => setToast(null), 3200);
  }, []);

  const confirm = useCallback<FeedbackApi["confirm"]>((options) => {
    return new Promise((resolve) => {
      setConfirmState({
        title: options.title,
        message: options.message,
        confirmText: options.confirmText ?? "确认",
        danger: !!options.danger,
        resolve,
      });
    });
  }, []);

  const closeConfirm = (value: boolean) => {
    const current = confirmState;
    setConfirmState(null);
    current?.resolve(value);
  };

  return (
    <FeedbackContext.Provider value={{ notify, confirm }}>
      {children}

      {toast && (
        <div class={`toast toast-${toast.kind}`} role="status" aria-live="polite">
          {toast.message}
        </div>
      )}

      {confirmState && (
        <div class="confirm-backdrop" role="presentation" onClick={() => closeConfirm(false)}>
          <div
            class="confirm-dialog"
            role="dialog"
            aria-modal="true"
            aria-labelledby="confirm-title"
            onClick={(e) => e.stopPropagation()}
          >
            <h2 id="confirm-title">{confirmState.title}</h2>
            {confirmState.message && <p>{confirmState.message}</p>}
            <div class="confirm-actions">
              <button class="btn btn-ghost" onClick={() => closeConfirm(false)}>
                取消
              </button>
              <button
                class={`btn ${confirmState.danger ? "btn-danger" : "btn-primary"}`}
                onClick={() => closeConfirm(true)}
              >
                {confirmState.confirmText}
              </button>
            </div>
          </div>
        </div>
      )}
    </FeedbackContext.Provider>
  );
}

export function useFeedback() {
  const ctx = useContext(FeedbackContext);
  if (!ctx) throw new Error("useFeedback must be used inside FeedbackProvider");
  return ctx;
}
