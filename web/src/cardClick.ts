const INTERACTIVE_SELECTOR = [
  "a",
  "button",
  "input",
  "textarea",
  "select",
  "label",
  "[role='button']",
  "[data-card-interactive]"
].join(",");

export function shouldOpenMemoDetailFromCardClick(target: EventTarget | null, editing: boolean): boolean {
  if (editing || !target || !("closest" in target)) return false;
  const element = target as Element;
  return !element.closest(INTERACTIVE_SELECTOR);
}
