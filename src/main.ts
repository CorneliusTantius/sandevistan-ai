import { mount } from "svelte";
import App from "./App.svelte";
import "./styles.css";

function showCrash(error: unknown) {
  const target = document.getElementById("app");
  if (!target) return;
  target.innerHTML = `<pre style="margin:0;padding:12px;color:#E84545;background:#111820;height:100vh;overflow:auto">${String(error)}</pre>`;
}

window.addEventListener("error", (event) => showCrash(event.error ?? event.message));
window.addEventListener("unhandledrejection", (event) => showCrash(event.reason));

try {
  const target = document.getElementById("app")!;
  target.textContent = "";
  mount(App, {
    target,
  });
} catch (error) {
  showCrash(error);
}
