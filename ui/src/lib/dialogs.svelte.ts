// Promise-based confirmation dialog rendered by App.svelte.

export interface DialogChoice {
  id: string;
  label: string;
  kind?: 'primary' | 'danger' | 'default';
}

interface DialogState {
  message: string;
  choices: DialogChoice[];
  resolve: (choice: string | null) => void;
}

export const dialog = $state({ current: null as DialogState | null });

export function ask(message: string, choices: DialogChoice[]): Promise<string | null> {
  return new Promise((resolve) => {
    dialog.current = { message, choices, resolve };
  });
}

export async function confirm(message: string, okLabel: string, cancelLabel: string, danger = false): Promise<boolean> {
  const choice = await ask(message, [
    { id: 'cancel', label: cancelLabel },
    { id: 'ok', label: okLabel, kind: danger ? 'danger' : 'primary' },
  ]);
  return choice === 'ok';
}

export function closeDialog(choice: string | null) {
  const current = dialog.current;
  dialog.current = null;
  current?.resolve(choice);
}
