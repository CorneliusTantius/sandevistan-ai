import type { Action } from "svelte/action";

export const bindElement: Action<HTMLElement, (element: HTMLElement) => void> = (node, setter) => {
  setter(node);
  return {};
};
